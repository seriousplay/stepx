"""Exercise storage ownership and actual descriptor-based deletion locally."""
import ast
import base64
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import stat
import tempfile
import time
import unittest

from desktop.tests.test_gui_native_task_host import record, final_result

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('native_host_storage', ROOT / 'scripts/gui-native-host-storage.py')
storage = importlib.util.module_from_spec(spec)
spec.loader.exec_module(storage)


class HostStorageTests(unittest.TestCase):
    def setup_tree(self, directory, legacy=False):
        value = record()
        value['code_cache_realpath'] = str(directory)
        value['start_proof'] = {'device_started_epoch': int(time.time()) - 2}
        value['collect_proof'] = dict(operation='task_host_collect', status='collected', host_exited=True,
                                     owned_files_removed=True, removed={'dex': True, 'result': True},
                                     worker_exit_code=0, device_started_epoch=int(time.time()) + 2,
                                     final=final_result(value))
        name = 'stepx-gui-task-host-12345678-1234-1234-1234-123456789abc'
        root = directory / name; root.mkdir(mode=0o700)
        owner = dict(pid=value['process']['pid'], boot_id=value['boot_id'], root=str(root),
                     nonce='12345678-1234-1234-1234-123456789abc')
        if not legacy:
            owner['start_ticks'] = value['process']['start_ticks']
        marker = root / 'owner.json'; marker.write_text(json.dumps(owner)); marker.chmod(0o600)
        (root / 'files').mkdir(mode=0o700)
        (root / 'files/item.bin').write_bytes(b'only owned data')
        (root / 'files/item.bin').chmod(0o600)
        (directory / 'unrelated').write_bytes(b'keep this')
        def checked(condition, message):
            if not condition:
                raise RuntimeError(message)
        namespace = dict(os=os, stat=stat, json=json, re=re, hashlib=hashlib, RECORD=value,
                         checked=checked, out={}, publish=lambda out: None)
        exec(storage.STORAGE_HELPERS, namespace)
        namespace['directory'] = os.open(directory, os.O_RDONLY | os.O_DIRECTORY)
        self.addCleanup(os.close, namespace['directory'])
        return namespace, root, owner, value

    def test_audit_and_cleanup_only_own_tree(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            namespace, root, owner, value = self.setup_tree(directory)
            candidate = namespace['audit_candidate'](root.name)
            self.assertEqual(candidate['ownership_binding'], 'pid_start_boot')
            self.assertEqual(candidate['owner_sha256'], hashlib.sha256((root / 'owner.json').read_bytes()).hexdigest())
            namespace['remove_pinned_tree'](candidate)
            self.assertFalse(root.exists())
            self.assertEqual((directory / 'unrelated').read_bytes(), b'keep this')

    def test_strict_root_mode_is_checked_only_after_owner_match_with_safe_diagnostics(self):
        with tempfile.TemporaryDirectory() as temporary:
            namespace, root, owner, value = self.setup_tree(Path(temporary))
            root.chmod(0o750)
            with self.assertRaisesRegex(RuntimeError, 'storage_root_invalid'):
                namespace['audit_candidate'](root.name)
            diagnostic = namespace['out']['storage_root_diagnostic']
            self.assertEqual(diagnostic, dict(mode=0o750, uid=1000, gid=1000,
                                             same_device=True, owner_process_matches=True))
            self.assertTrue((root / 'files/item.bin').exists())
            owner['pid'] += 1
            (root / 'owner.json').write_text(json.dumps(owner))
            namespace['out'].clear()
            self.assertIsNone(namespace['audit_candidate'](root.name))
            self.assertNotIn('storage_root_diagnostic', namespace['out'])

    def test_inherited_directory_setgid_is_pinned_and_can_be_removed(self):
        with tempfile.TemporaryDirectory() as temporary:
            namespace, root, owner, value = self.setup_tree(Path(temporary))
            root.chmod(0o2700)
            (root / 'files').chmod(0o2700)
            candidate = namespace['audit_candidate'](root.name)
            self.assertEqual(candidate['root_metadata']['mode'], 0o2700)
            self.assertEqual(candidate['entries']['files']['mode'], 0o2700)
            namespace['remove_pinned_tree'](candidate)
            self.assertFalse(root.exists())

    def test_setgid_does_not_allow_shared_writes_or_other_special_bits(self):
        for mode, is_file in ((0o2720, False), (0o4700, False), (0o1700, False), (0o2600, True)):
            with self.subTest(mode=oct(mode)), tempfile.TemporaryDirectory() as temporary:
                namespace, root, owner, value = self.setup_tree(Path(temporary))
                target = root / 'files/item.bin' if is_file else root / 'files'
                target.chmod(mode)
                with self.assertRaisesRegex(RuntimeError, 'storage_writable_by_other|storage_special_mode'):
                    namespace['audit_candidate'](root.name)
                self.assertTrue((root / 'files/item.bin').exists())

    def test_legacy_owner_requires_recorded_lifetime_and_exact_pid_boot(self):
        with tempfile.TemporaryDirectory() as temporary:
            namespace, root, owner, value = self.setup_tree(Path(temporary), legacy=True)
            self.assertEqual(namespace['audit_candidate'](root.name)['ownership_binding'], 'legacy_pid_boot_time_window')
            value['start_proof']['device_started_epoch'] -= 10000
            with self.assertRaisesRegex(RuntimeError, 'legacy_lifetime_missing'):
                namespace['audit_candidate'](root.name)
            self.assertTrue(root.exists())

    def test_start_tick_mismatch_and_replaced_marker_are_never_deleted(self):
        with tempfile.TemporaryDirectory() as temporary:
            namespace, root, owner, value = self.setup_tree(Path(temporary))
            pinned = namespace['audit_candidate'](root.name)
            owner['start_ticks'] = '9999'
            (root / 'owner.json').write_text(json.dumps(owner))
            with self.assertRaisesRegex(RuntimeError, 'owner_start_changed'):
                namespace['remove_pinned_tree'](pinned)
            self.assertTrue((root / 'files/item.bin').exists())

    def test_symlinks_hardlinks_and_other_tree_changes_fail_before_deletion(self):
        for kind in ('symlink', 'hardlink', 'changed'):
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary)
                namespace, root, owner, value = self.setup_tree(directory)
                pinned = namespace['audit_candidate'](root.name)
                if kind == 'symlink':
                    (root / 'files/link').symlink_to(directory / 'unrelated')
                elif kind == 'hardlink':
                    os.link(root / 'files/item.bin', root / 'files/hard')
                else:
                    (root / 'files/item.bin').write_bytes(b'replaced contents')
                with self.assertRaises(RuntimeError):
                    namespace['remove_pinned_tree'](pinned)
                self.assertTrue(root.exists())
                self.assertTrue((root / 'owner.json').exists())
                self.assertEqual((directory / 'unrelated').read_bytes(), b'keep this')

    def test_storage_workers_survive_real_transport_substitution(self):
        value = record()
        value['collect_proof'] = dict(operation='task_host_collect', status='collected', host_exited=True,
                                     owned_files_removed=True, removed={'dex': True, 'result': True},
                                     worker_exit_code=0, final=final_result(value))
        value['storage_audit_proof'] = dict(status='storage_audited', host_exited=True, storage_count=1,
                                           storage={'name': 'fake-but-not-executed'})
        permissions = storage.runner.module('storage_transport_test', 'companion-permissions.py')
        template = permissions.agentbus_worker_source((permissions.WORKFLOW / 'tools/root_worker.py').read_text())
        assignment = next(node for node in ast.parse(template).body if isinstance(node, ast.Assign)
                          and any(isinstance(target, ast.Name) and target.id == 'src' for target in node.targets))
        for cleanup in (False, True):
            code = storage.build_worker(value, cleanup)
            self.assertNotIn('RESULT', code)
            transferred = eval(compile(ast.Expression(assignment.value), '<storage-transport>', 'eval'),
                               dict(code=code, result='/tmp/storage-test.json', os=os))
            compile(transferred, '<storage-worker>', 'exec')


if __name__ == '__main__':
    unittest.main()

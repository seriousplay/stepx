"""Local execution of crash recovery's exact audit and fd-based deletion body."""
import ast
import base64
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import stat
import tempfile
import unittest

from desktop.tests.test_gui_native_task_host import record
import desktop.tests.test_gui_native_host_storage as storage_tests

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('native_crash', ROOT / 'scripts/gui-native-crash-recovery.py')
crash = importlib.util.module_from_spec(spec)
spec.loader.exec_module(crash)


def failed_record():
    value = record()
    value['start_proof'] = dict(operation='task_host_start', host_spawned=True, detached=True,
                               phase='host_detached', worker_exit_code=0, process=copy.deepcopy(value['process']),
                               files=copy.deepcopy(value['files']), code_cache_realpath=value['code_cache_realpath'])
    value['collect_proof'] = dict(operation='task_host_collect', host_exited=True, worker_exit_code=0,
                                 operation_error='RuntimeError', removed={})
    return value


class CrashRecoveryTests(unittest.TestCase):
    def setup_tree(self, directory):
        ns, root, owner, value = storage_tests.HostStorageTests.setup_tree(self, directory)
        payload = b'dex\nowned crash test artifact'
        for key, contents, mode in [('dex', payload, 0o400), ('result', b'', 0o600)]:
            path = directory / value[key + '_name']; path.write_bytes(contents); path.chmod(mode)
            st = path.stat()
            value['files'][key] = dict(dev=st.st_dev, ino=st.st_ino, uid=st.st_uid, gid=st.st_gid,
                                       nlink=st.st_nlink, mode=stat.S_IMODE(st.st_mode))
        value['dex_sha256'] = hashlib.sha256(payload).hexdigest()
        value['start_proof'] = dict(operation='task_host_start', host_spawned=True, detached=True,
                                   phase='host_detached', worker_exit_code=0, process=copy.deepcopy(value['process']),
                                   files=copy.deepcopy(value['files']), code_cache_realpath=value['code_cache_realpath'])
        value['collect_proof'] = dict(operation='task_host_collect', host_exited=True, worker_exit_code=0,
                                     operation_error='RuntimeError', removed={})
        return ns, root, value

    def execute(self, directory, value, *, cleanup=False, visible=None, external_dead=True):
        def checked(condition, code):
            if not condition:
                raise RuntimeError(code)
        ns = dict(os=os, stat=stat, re=re, json=json, hashlib=hashlib, RECORD=value,
                  checked=checked, out={}, publish=lambda proof: None)
        exec(crash.runner.WORKER_COMMON.split("checked(boot()==")[0], ns)
        exec(crash.storage.STORAGE_HELPERS, ns)
        ns.update(directory=os.open(directory, os.O_RDONLY | os.O_DIRECTORY),
                  observed=lambda pid: visible, DO_CLEANUP=cleanup, EXTERNALLY_EXITED=external_dead,
                  ALREADY_REMOVED=cleanup and value.get('crash_cleanup_proof', {}).get('owned_staged_removed') is True)
        exec(crash.CRASH_OPERATIONS, ns)
        ns['out']['worker_exit_code'] = 0
        return ns['out']

    def test_audit_then_cleanup_deletes_only_recorded_files_without_inventing_final(self):
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp); ns, root, value = self.setup_tree(directory)
            original = copy.deepcopy(value['collect_proof'])
            audit = self.execute(directory, value)
            self.assertEqual(audit['status'], 'crash_audited')
            self.assertEqual(audit['staged']['result']['size'], 0)
            value['crash_audit_proof'] = audit
            cleaned = self.execute(directory, value, cleanup=True)
            self.assertEqual(cleaned['status'], 'crash_files_removed')
            self.assertFalse(root.exists())
            self.assertFalse((directory / value['dex_name']).exists())
            self.assertFalse((directory / value['result_name']).exists())
            self.assertEqual((directory / 'unrelated').read_bytes(), b'keep this')
            self.assertEqual(value['collect_proof'], original)
            for key in ('final_event_available', 'execution_verified', 'no_task_started_verified', 'global_rollback_verified'):
                self.assertFalse(cleaned[key])
            self.assertNotIn('final', cleaned)

    def test_nonempty_result_and_changed_inode_never_authorize_deletion(self):
        for kind in ('nonempty', 'inode'):
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as temp:
                directory = Path(temp); ns, root, value = self.setup_tree(directory)
                path = directory / value['result_name']
                if kind == 'nonempty':
                    path.write_bytes(b'private non-final output')
                else:
                    replacement = directory / 'replacement'; replacement.write_bytes(b''); replacement.chmod(0o600)
                    os.replace(replacement, path)
                out = self.execute(directory, value)
                self.assertEqual(out['operation_error_code'], 'result_not_empty' if kind == 'nonempty' else 'staged_inode_changed')
                self.assertTrue(root.exists()); self.assertTrue(path.exists())
                self.assertNotIn('private non-final output', json.dumps(out))

    def test_live_pid_or_missing_independent_exit_proof_stops_before_file_work(self):
        for visible, external, reason in (({'pid': 24567}, True, 'crash_host_still_live'),
                                          (None, False, 'exit_not_independently_verified')):
            with self.subTest(reason=reason), tempfile.TemporaryDirectory() as temp:
                directory = Path(temp); ns, root, value = self.setup_tree(directory)
                out = self.execute(directory, value, visible=visible, external_dead=external)
                self.assertEqual(out['operation_error_code'], reason)
                self.assertNotIn('host_exited', out); self.assertTrue(root.exists())
                self.assertTrue((directory / value['dex_name']).exists())

    def test_cleanup_requires_completed_audit_and_original_start_proof(self):
        value = failed_record()
        with self.assertRaisesRegex(RuntimeError, 'crash audit'):
            crash.build_worker(value, True, cleanup=True)
        value['start_proof']['process']['start_ticks'] = 'other'
        with self.assertRaisesRegex(RuntimeError, 'detached-start proof'):
            crash.build_worker(value, True)

    def test_post_audit_change_is_rejected_before_deleting_storage(self):
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp); ns, root, value = self.setup_tree(directory)
            value['crash_audit_proof'] = self.execute(directory, value)
            (root / 'files/item.bin').write_bytes(b'changed since audit')
            out = self.execute(directory, value, cleanup=True)
            self.assertEqual(out['operation_error_code'], 'storage_changed_since_crash_audit')
            self.assertTrue((root / 'owner.json').exists())
            self.assertTrue((directory / value['dex_name']).exists())

    def test_completed_cleanup_rechecks_absence_without_attempting_another_deletion(self):
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp); ns, root, value = self.setup_tree(directory)
            value['crash_audit_proof'] = self.execute(directory, value)
            value['crash_cleanup_proof'] = self.execute(directory, value, cleanup=True)
            again = self.execute(directory, value, cleanup=True)
            self.assertTrue(again['verification_only'])
            self.assertEqual(again['status'], 'crash_files_removed')
            self.assertEqual((directory / 'unrelated').read_bytes(), b'keep this')
            (directory / value['result_name']).write_bytes(b'new unrelated file')
            refused = self.execute(directory, value, cleanup=True)
            self.assertEqual(refused['operation_error_code'], 'removed_crash_path_reappeared')
            self.assertEqual((directory / value['result_name']).read_bytes(), b'new unrelated file')

    def test_workers_survive_real_transport_substitution(self):
        value = failed_record()
        permissions = crash.runner.module('crash_transport_test', 'companion-permissions.py')
        template = permissions.agentbus_worker_source((permissions.WORKFLOW / 'tools/root_worker.py').read_text())
        assignment = next(node for node in ast.parse(template).body if isinstance(node, ast.Assign)
                          and any(isinstance(target, ast.Name) and target.id == 'src' for target in node.targets))
        for cleanup in (False, True):
            if cleanup:
                value['crash_audit_proof'] = dict(operation='task_host_crash_audit', status='crash_audited', host_exited=True,
                                                  storage_count=1, worker_exit_code=0, staged={'dex': {}, 'result': {}}, storage={})
            code = crash.build_worker(value, True, cleanup)
            transferred = eval(compile(ast.Expression(assignment.value), '<crash-transport>', 'eval'),
                               dict(code=code, result='/tmp/crash-test.json', os=os))
            compile(transferred, '<crash-worker>', 'exec')


if __name__ == '__main__':
    unittest.main()

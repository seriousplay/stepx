"""Exercise native injection guards locally; no ADB/device access."""
import ast
import hashlib
import importlib.util
import io
from pathlib import Path
import re
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('gui_native_patch', ROOT / 'scripts/gui-native-patch.py')
adapter = importlib.util.module_from_spec(spec)
spec.loader.exec_module(adapter)


class NativeGuiPatchGuardTests(unittest.TestCase):
    identity = dict(pid=7204, start_ticks='1721', boot_id='device-boot-123')

    def worker(self):
        return adapter.build_worker(b'dex\n035\0test', 'a' * 64, 10215, self.identity)

    def test_loader_targets_only_ability_and_keeps_detach(self):
        code = self.worker()
        compile(code, '<native-gui-worker>', 'exec')
        self.assertIn("cstr('stepx.NativeGuiAdapter')", code)
        self.assertIn("run','(Ljava/lang/String;I)Ljava/lang/String;'", code)
        self.assertIn('if int(n)==pid:tids.append(int(n))', code)
        self.assertIn("pt(17,t);out['detached']=True", code)
        for forbidden in ('STEPX_SYSTEM_SERVER_PID', 'stepx.GmsProbe', 'stepx.AgentBusLease', "'android.ui'"):
            self.assertNotIn(forbidden, code)

    def run_target_guards(self, changes=None):
        """Execute the actual emitted identity assertions before memory access."""
        code = self.worker()
        tree = ast.parse(code)
        checks = []
        for node in tree.body:
            if isinstance(node, ast.Assign) and any(isinstance(t, ast.Name) and t.id == 'maps'
                                                    for t in node.targets):
                break
            if isinstance(node, ast.Assert):
                checks.append(node)
        self.assertGreaterEqual(len(checks), 6)
        values = {
            '/proc/7204/cmdline': b'com.stepos.ability\0',
            '/proc/7204/stat': '7204 (worker name) ' + ' '.join(['S'] + ['0'] * 18 + ['1721']),
            '/proc/sys/kernel/random/boot_id': 'device-boot-123\n',
            '/proc/7204/attr/current': 'u:r:system_app:s0\0',
            '/proc/7204/status': 'Name:\tability\nUid:\t1000\t1000\t1000\t1000\n',
            adapter.ABILITY_APK: b'original apk',
        }
        values.update(changes or {})
        def opened(path, mode='r'):
            value = values[path]
            return io.BytesIO(value) if 'b' in mode else io.StringIO(value)
        namespace = dict(open=opened, re=re, hashlib=hashlib, pid=7204,
                         LEASE_START='1721', LEASE_BOOT='device-boot-123',
                         LEASE_APK=adapter.ABILITY_APK,
                         LEASE_APK_SHA256=hashlib.sha256(b'original apk').hexdigest())
        exec(compile(ast.Module(body=checks, type_ignores=[]), '<target-guards>', 'exec'), namespace)

    def test_generated_guard_accepts_exact_process(self):
        self.run_target_guards()

    def test_generated_guard_rejects_pid_reuse_reboot_uid_domain_and_apk_change(self):
        for changed in (
            {'/proc/7204/cmdline': b'unrelated.process\0'},
            {'/proc/7204/stat': '7204 (ability) ' + ' '.join(['S'] + ['0'] * 18 + ['9999'])},
            {'/proc/sys/kernel/random/boot_id': 'another-boot'},
            {'/proc/7204/attr/current': 'u:r:untrusted_app:s0'},
            {'/proc/7204/status': 'Uid:\t1000\t0\t1000\t1000\n'},
            {adapter.ABILITY_APK: b'updated apk'},
        ):
            with self.subTest(changed=next(iter(changed))):
                with self.assertRaises(AssertionError):
                    self.run_target_guards(changed)

    def test_modified_native_loader_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'module').mkdir()
            (root / 'module/inject.py').write_text('arbitrary code')
            with patch.object(adapter.permissions, 'WORKFLOW', root):
                with self.assertRaisesRegex(RuntimeError, 'Audited native loader changed'):
                    self.worker()

    def test_task_or_replaced_payload_cannot_enter_inspect_only_loader(self):
        self.assertEqual(hashlib.sha256(adapter.read_reviewed_probe()).hexdigest(), adapter.PROBE_DEX_SHA256)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            payload = root / 'desktop/assets/stepx-native-gui.dex'
            payload.parent.mkdir(parents=True)
            payload.write_bytes((ROOT / 'desktop/assets/stepx-native-gui-task.dex').read_bytes())
            with patch.object(adapter, 'ROOT', root):
                with self.assertRaisesRegex(RuntimeError, 'Only the reviewed inspect-only'):
                    adapter.read_reviewed_probe()

    def test_native_template_drift_is_rejected_even_after_hash_review(self):
        source = (adapter.permissions.WORKFLOW / 'module/inject.py').read_text()
        source = source.replace("cstr('stepx.GmsProbe')", "cstr('RenamedEntry')")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'module').mkdir()
            (root / 'module/inject.py').write_text(source)
            with patch.object(adapter.permissions, 'WORKFLOW', root), \
                    patch.object(adapter.permissions, 'LOADER_SHA256', hashlib.sha256(source.encode()).hexdigest()):
                with self.assertRaisesRegex(RuntimeError, 'Unsupported native adapter template'):
                    self.worker()

    def test_preflight_refuses_transport_pointing_at_another_physical_phone(self):
        with patch.object(adapter, 'inspect', return_value={}), \
                patch.object(adapter.permissions, 'guard'), \
                patch.object(adapter.permissions, 'shell', return_value='unrelated-phone'):
            with self.assertRaisesRegex(RuntimeError, 'Physical device identity mismatch'):
                adapter.preflight(Path('trusted.apk'))


if __name__ == '__main__':
    unittest.main()

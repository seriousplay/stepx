"""Guard regression tests for the optional, exact-firmware AgentBus adapter."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('amoo_background', ROOT / 'scripts/amoo-background.py')
adapter = importlib.util.module_from_spec(spec)
spec.loader.exec_module(adapter)


class AmooBackgroundGuardTests(unittest.TestCase):
    def test_payload_is_scoped_to_exact_native_process_and_abi(self):
        code = adapter.build_worker(b'dex\n035\0test', 'a' * 64, 10215,
                                    {'pid': 8444, 'start_ticks': '1931'})
        compile(code, '<lease-worker>', 'exec')
        self.assertIn('pid=LEASE_PID', code)
        self.assertIn("==b'com.stepos.agentbus'", code)
        self.assertIn('==LEASE_START', code)
        self.assertIn('==LEASE_APK_SHA256', code)
        self.assertIn('if int(n)==pid:tids.append(int(n))', code)
        self.assertIn("cstr('stepx.AgentBusLease')", code)
        self.assertNotIn('STEPX_SYSTEM_SERVER_PID', code)
        self.assertNotIn('stepx.GmsProbe', code)
        self.assertNotIn('ZGV4C', code.split('lib.setfsuid(0)', 1)[1])
        self.assertIn("pt(17,t);out['detached']=True", code)

    def test_changed_loader_fails_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'module').mkdir()
            (root / 'module/inject.py').write_text('unreviewed loader')
            with patch.object(adapter.permissions, 'WORKFLOW', root):
                with self.assertRaisesRegex(RuntimeError, 'Audited native loader changed'):
                    adapter.build_worker(b'dex\n', 'a' * 64, 10215, {'pid': 8444, 'start_ticks': '1931'})

    def test_changed_agentbus_version_refuses_even_inspection(self):
        def shell(*args):
            if args == ('pm', 'path', adapter.AGENTBUS):
                return 'package:' + adapter.AGENTBUS_APK
            if args == ('sha256sum', adapter.AGENTBUS_APK):
                return 'b' * 64 + '  ' + adapter.AGENTBUS_APK
            raise AssertionError('Should reject before further process access')
        with patch.object(adapter.permissions, 'inspect', return_value={'signature_verified': True, 'uid': 10215}), \
                patch.object(adapter.permissions, 'shell', side_effect=shell):
            with self.assertRaisesRegex(RuntimeError, 'Native AgentBus version changed'):
                adapter.inspect(Path('companion.apk'))

    def test_existing_endpoint_never_reinjects_or_starts_lease(self):
        with patch.object(adapter, 'preflight', return_value={'installed': True}), \
                patch.object(adapter, 'build_worker', side_effect=AssertionError('No reinjection')):
            result = adapter.apply(Path('companion.apk'))
            self.assertEqual(result['status'], 'already_present')
            self.assertFalse(result['injected'])


if __name__ == '__main__':
    unittest.main()

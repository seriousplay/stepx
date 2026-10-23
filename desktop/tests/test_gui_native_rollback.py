"""Validate that rollback can stop only its recorded, authenticated probe."""
import importlib.util
from pathlib import Path
import unittest
from unittest.mock import Mock, patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('gui_native_rollback', ROOT / 'scripts/gui-native-rollback.py')
rollback = importlib.util.module_from_spec(spec)
spec.loader.exec_module(rollback)


class NativeGuiRollbackTests(unittest.TestCase):
    def setUp(self):
        self.record = dict(pid=7204, start_ticks='1721', boot_id='same-boot', companion_uid=10215,
                           backup='/private/backup')
        self.process = dict(present=True, pid=7204, start_ticks='1721', boot_id='same-boot')
        self.proof = dict(current_process=self.process, system_server_unchanged=True,
                          target_process_replaced=False)
        self.status = dict(ok=True, version=1, pid=7204, uid=10215, target=rollback.recovery.PACKAGE,
                           socket=rollback.patcher.SOCKET, apk_sha256=rollback.recovery.APK_SHA256,
                           mode='inspect_only', execution_enabled=False)
        self.shutdown = dict(ok=True, version=1, running=False, execution_enabled=False,
                             rollback='process_adapter_closed')

    def common_mocks(self):
        for target, key, value in (
            (rollback, 'load_record', self.record),
            (rollback.patcher.permissions, 'inspect', dict(signature_verified=True, uid=10215)),
            (rollback.recovery, 'verify_backup', self.proof),
            (rollback.recovery, 'capture', dict(ability=self.process)),
            (rollback.patcher, 'save', None),
        ):
            mocked = patch.object(target, key, return_value=value)
            mocked.start()
            self.addCleanup(mocked.stop)

    def test_content_parser_extracts_json_without_treating_bundle_as_truth(self):
        adb = Mock()
        adb.shell.return_value = 'Result: Bundle[{result={"ok":true,"version":1,"running":false}, ok=true}]'
        self.assertEqual(rollback.provider(adb, 'shutdown')['running'], False)
        adb.shell.return_value = 'Result: Bundle[{ok=true, result={"ok":false,"version":1,"error":"rejected"}}]'
        with self.assertRaisesRegex(RuntimeError, 'rejected'):
            rollback.provider(adb, 'shutdown')

    def test_wrong_endpoint_or_execution_mode_cannot_be_stopped(self):
        for change in ({'pid': 999}, {'uid': 999}, {'target': 'unrelated'},
                       {'apk_sha256': 'bad'}, {'mode': 'execution'}, {'execution_enabled': True}):
            with self.subTest(change=change), self.assertRaises(RuntimeError):
                rollback.verify_probe_identity(dict(self.status, **change), self.record)

    def test_scoped_shutdown_leaves_precise_residual_memory_disclosure(self):
        self.common_mocks()
        with patch.object(rollback, 'socket_present', side_effect=[True, False]), \
                patch.object(rollback, 'provider', side_effect=[self.status, self.shutdown]) as provider:
            result = rollback.rollback(Mock(), Path('/private/record.json'), Path('trusted.apk'))
        self.assertEqual([call.args[1] for call in provider.call_args_list], ['status', 'shutdown'])
        self.assertTrue(result['endpoint_absent'])
        self.assertTrue(result['in_memory_dex_may_remain'])
        self.assertFalse(result['native_behavior_regression_verified'])

    def test_replacement_process_endpoint_is_never_stopped(self):
        self.common_mocks()
        self.proof['current_process'] = dict(self.process, start_ticks='9999')
        with patch.object(rollback, 'socket_present', return_value=True), \
                patch.object(rollback, 'provider', side_effect=AssertionError('Must not call replacement')):
            with self.assertRaisesRegex(RuntimeError, 'different process'):
                rollback.rollback(Mock(), Path('/private/record.json'), Path('trusted.apk'))

    def test_process_race_after_status_prevents_shutdown(self):
        self.common_mocks()
        with patch.object(rollback, 'socket_present', return_value=True), \
                patch.object(rollback, 'provider', return_value=self.status) as provider, \
                patch.object(rollback.recovery, 'capture', return_value={
                    'ability': dict(self.process, start_ticks='9999')}):
            with self.assertRaisesRegex(RuntimeError, 'Ability changed before shutdown'):
                rollback.rollback(Mock(), Path('/private/record.json'), Path('trusted.apk'))
        self.assertEqual([call.args[1] for call in provider.call_args_list], ['status'])

    def test_wrong_companion_uid_is_rejected_before_ipc(self):
        self.common_mocks()
        with patch.object(rollback.patcher.permissions, 'inspect', return_value=dict(signature_verified=True, uid=10216)), \
                patch.object(rollback, 'provider', side_effect=AssertionError('Must not contact endpoint')):
            with self.assertRaisesRegex(RuntimeError, 'Signed companion identity changed'):
                rollback.rollback(Mock(), Path('/private/record.json'), Path('trusted.apk'))


if __name__ == '__main__':
    unittest.main()

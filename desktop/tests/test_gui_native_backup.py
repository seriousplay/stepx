"""No-device regression tests for native GUI backup and rollback evidence."""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('gui_native_backup', ROOT / 'scripts/gui-native-backup.py')
backup = importlib.util.module_from_spec(spec)
spec.loader.exec_module(backup)


def snapshot():
    identity = {'ro.serialno': backup.SERIAL, 'ro.build.fingerprint': backup.FINGERPRINT,
                'ro.build.version.sdk': '36', 'ro.product.cpu.abi': 'arm64-v8a', 'selinux': 'Enforcing'}
    process = dict(present=True, name=backup.PACKAGE, pid=1234, start_ticks='321',
                   boot_id='01234567-0123-0123-0123-0123456789ab', uids=[1000] * 4,
                   gids=[1000] * 4, selinux='u:r:system_app:s0')
    return dict(identity=identity,
                files={path: dict(sha256=sha, mode='644', uid=0, gid=0, bytes=20,
                                  mtime=1, selinux='u:object_r:system_file:s0')
                       for path, sha in backup.PINNED_FILES.items()},
                startup_properties={key: '' for key in backup.STARTUP_PROPERTIES},
                ability=process,
                system_server=dict(process, name='system_server', pid=100, start_ticks='50',
                                   selinux='u:r:system_server:s0'))


class FakeAdb:
    transport = backup.SERIAL

    def pull(self, remote, local):
        assert remote == backup.APK
        Path(local).write_bytes(b'original apk')


class NativeGuiBackupTests(unittest.TestCase):
    def setUp(self):
        self.hash = hashlib.sha256(b'original apk').hexdigest()
        self.hash_patch = patch.object(backup, 'APK_SHA256', self.hash)
        self.files_patch = patch.dict(backup.PINNED_FILES, {backup.APK: self.hash})
        self.hash_patch.start()
        self.files_patch.start()
        self.addCleanup(self.hash_patch.stop)
        self.addCleanup(self.files_patch.stop)

    def make_backup(self, directory):
        target = Path(directory) / 'backup'
        with patch.object(backup, 'capture', return_value=snapshot()):
            result = backup.create_backup(FakeAdb(), target)
        self.assertFalse(result['rollback_verified'])
        self.assertEqual(target.stat().st_mode & 0o777, 0o700)
        self.assertEqual((target / 'manifest.json').stat().st_mode & 0o777, 0o600)
        return target

    def test_explicit_device_is_required(self):
        with self.assertRaisesRegex(RuntimeError, 'explicit ADB transport'):
            backup.Adb('adb', '')

    def test_transport_is_never_trusted_without_physical_serial(self):
        class WrongPhone:
            def shell(self, command, name):
                return {'ro.serialno': 'UNRELATED_PHONE', 'ro.build.fingerprint': backup.FINGERPRINT,
                        'ro.build.version.sdk': '36', 'ro.product.cpu.abi': 'arm64-v8a'}[name]
        with self.assertRaisesRegex(RuntimeError, 'Physical device serial'):
            backup.read_identity(WrongPhone())

    def test_no_existing_backup_can_be_overwritten(self):
        with tempfile.TemporaryDirectory() as directory:
            target = self.make_backup(directory)
            with patch.object(backup, 'capture', side_effect=AssertionError('No device call')):
                with self.assertRaises(FileExistsError):
                    backup.create_backup(FakeAdb(), target)

    def test_race_during_pull_marks_backup_unusable(self):
        with tempfile.TemporaryDirectory() as directory:
            first = snapshot()
            second = copy.deepcopy(first)
            second['ability']['start_ticks'] = '999'
            target = Path(directory) / 'backup'
            with patch.object(backup, 'capture', side_effect=[first, second]):
                with self.assertRaisesRegex(RuntimeError, 'changed during backup'):
                    backup.create_backup(FakeAdb(), target)
            self.assertTrue((target / 'FAILED.json').is_file())
            self.assertFalse((target / 'manifest.json').exists())

    def test_local_corruption_prevents_device_verification(self):
        with tempfile.TemporaryDirectory() as directory:
            target = self.make_backup(directory)
            (target / 'ability.original.apk').write_bytes(b'corrupt')
            with patch.object(backup, 'capture', side_effect=AssertionError('No device call')):
                with self.assertRaisesRegex(RuntimeError, 'Local original APK is corrupt'):
                    backup.verify_backup(FakeAdb(), target)

    def test_manifest_cannot_change_remote_file_scope(self):
        with tempfile.TemporaryDirectory() as directory:
            target = self.make_backup(directory)
            manifest_path = target / 'manifest.json'
            value = json.loads(manifest_path.read_text())
            value['snapshot']['files']['/data/unrelated'] = {}
            manifest_path.write_text(json.dumps(value))
            with patch.object(backup, 'capture', side_effect=AssertionError('No device call')):
                with self.assertRaisesRegex(RuntimeError, 'file scope changed'):
                    backup.verify_backup(FakeAdb(), target)

    def test_changed_file_permissions_fail_verification(self):
        with tempfile.TemporaryDirectory() as directory:
            target = self.make_backup(directory)
            changed = snapshot()
            changed['files'][backup.APK]['mode'] = '777'
            with patch.object(backup, 'capture', return_value=changed):
                with self.assertRaisesRegex(RuntimeError, 'Baseline mismatch after patch: files'):
                    backup.verify_backup(FakeAdb(), target)

    def test_pid_reuse_or_reboot_does_not_match_original_process(self):
        original = snapshot()['ability']
        self.assertTrue(backup.same_process(original, original))
        for change in ({'start_ticks': '999'}, {'boot_id': 'new-boot'}, {'pid': 999}, {'present': False}):
            self.assertFalse(backup.same_process(original, dict(original, **change)))

    def test_process_restart_is_not_misreported_as_complete_rollback(self):
        with tempfile.TemporaryDirectory() as directory:
            target = self.make_backup(directory)
            changed = snapshot()
            changed['ability']['start_ticks'] = '999'
            with patch.object(backup, 'capture', return_value=changed):
                result = backup.verify_backup(FakeAdb(), target)
            self.assertTrue(result['target_process_replaced'])
            self.assertTrue(result['system_server_unchanged'])
            self.assertFalse(result['rollback_verified'])


if __name__ == '__main__':
    unittest.main()

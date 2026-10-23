"""Doze adapter tests. All Android I/O is mocked; no phone is modified."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SCRIPT = Path(__file__).resolve().parents[2] / 'scripts/companion-power.py'
spec = importlib.util.spec_from_file_location('companion_power', SCRIPT)
power = importlib.util.module_from_spec(spec)
spec.loader.exec_module(power)


class CompanionPowerTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.identity = {'scope':power.SCOPE, 'package':power.PACKAGE, 'serial':'test-phone',
                         'fingerprint':'verified', 'signer':'a'*64, 'uid':10215}
        self.lists = {'system-excidle':{}, 'system':{'android':1000}, 'user':{'user.preexisting':10200}}
        self.commands = []
        for mocked in [patch.object(power, 'STATE', Path(self.directory.name)),
                       patch.object(power, 'verified_identity', return_value=self.identity),
                       patch.object(power.permissions, 'shell', side_effect=self.shell)]:
            mocked.start(); self.addCleanup(mocked.stop)

    def shell(self, *args):
        self.commands.append(args)
        self.assertEqual(args[:3], ('cmd','deviceidle','whitelist'))
        if len(args) == 3:
            return '\n'.join(f'{kind},{name},{uid}' for kind, values in self.lists.items() for name, uid in values.items())
        self.assertTrue(power.state_path(self.identity).exists(), 'recovery must precede mutation')
        if args[3] == '+' + power.PACKAGE:
            self.lists['user'][power.PACKAGE] = 10215
        elif args[3] == '-' + power.PACKAGE:
            del self.lists['user'][power.PACKAGE]
        else:
            self.fail('must not change another package or global idle policy')
        return ''

    def test_apply_idempotent_and_revoke_only_owned_entry(self):
        before = {k: v.copy() for k,v in self.lists.items()}
        first = power.operate(Path('app.apk'), 'apply')
        self.assertTrue(first['doze_exempt']); self.assertTrue(first['doze_added_by_tool'])
        self.assertFalse(first['global_policy_changed']); self.assertFalse(first['oem_freezer_changed'])
        self.assertTrue(power.operate(Path('app.apk'), 'apply')['doze_exempt'])
        writes = [c for c in self.commands if len(c) == 4]
        self.assertEqual(writes, [('cmd','deviceidle','whitelist','+'+power.PACKAGE)])
        self.assertEqual(power.operate(Path('app.apk'), 'revoke-owned')['status'], 'revoked')
        self.assertEqual(self.lists, before)
        self.assertEqual(power.operate(Path('app.apk'), 'revoke-owned')['status'], 'unchanged')
        record = power.load_state(power.state_path(self.identity),self.identity)
        self.assertFalse(record['before']['user']); self.assertEqual(record['phase'],'revoked')

    def test_preexisting_exemption_is_never_removed(self):
        self.lists['user'][power.PACKAGE] = 10215
        self.assertFalse(power.operate(Path('app.apk'), 'apply')['doze_added_by_tool'])
        self.assertEqual(power.operate(Path('app.apk'), 'revoke-owned')['status'], 'unchanged')
        self.assertTrue(power.operate(Path('app.apk'), 'inspect')['doze_exempt'])
        self.assertFalse(any(len(c) == 4 for c in self.commands))

    def test_except_idle_is_not_full_doze_exemption(self):
        self.lists['system-excidle'][power.PACKAGE] = 10215
        self.assertFalse(power.operate(Path('app.apk'), 'inspect')['doze_exempt'])
        self.assertTrue(power.operate(Path('app.apk'), 'apply')['doze_exempt'])
        power.operate(Path('app.apk'), 'revoke-owned')
        self.assertIn(power.PACKAGE,self.lists['system-excidle'])

    def test_uid_mismatch_prevents_any_write(self):
        self.lists['user'][power.PACKAGE] = 10216
        with self.assertRaisesRegex(RuntimeError,'UID'):
            power.operate(Path('app.apk'),'apply')
        self.assertFalse(any(len(c) == 4 for c in self.commands))

    def test_no_recovery_record_never_removes_user_entry(self):
        self.lists['user'][power.PACKAGE] = 10215
        self.assertEqual(power.operate(Path('app.apk'),'revoke-owned')['status'],'unchanged')
        self.assertIn(power.PACKAGE,self.lists['user'])

    def test_missing_readback_fails_and_preserves_prepared_record(self):
        original = self.shell
        def ignored_add(*args):
            if len(args) == 4:
                self.assertEqual(args[3],'+'+power.PACKAGE)
                self.commands.append(args)
                return ''
            return original(*args)
        with patch.object(power.permissions,'shell',side_effect=ignored_add):
            with self.assertRaisesRegex(RuntimeError,'回读'):
                power.operate(Path('app.apk'),'apply')
        saved = power.load_state(power.state_path(self.identity),self.identity)
        self.assertEqual(saved['phase'],'prepared')
        with self.assertRaisesRegex(RuntimeError,'未完成核验'):
            power.operate(Path('app.apk'),'revoke-owned')

    def test_unknown_output_and_duplicate_entries_are_rejected(self):
        for text in ['permission denied', 'global,app,10000', 'user,app,no', 'user,app,10000\nuser,app,10000']:
            with self.assertRaises(RuntimeError):
                power.parse_whitelist(text)

    def test_identity_and_original_state_cannot_be_forged_for_restore(self):
        power.operate(Path('app.apk'),'apply')
        path = power.state_path(self.identity)
        record = power.load_state(path,self.identity)
        record['before']['user'] = True
        power.save_state(path,record)
        with self.assertRaisesRegex(RuntimeError,'预先存在'):
            power.operate(Path('app.apk'),'revoke-owned')
        self.assertIn(power.PACKAGE,self.lists['user'])


if __name__ == '__main__':
    unittest.main()

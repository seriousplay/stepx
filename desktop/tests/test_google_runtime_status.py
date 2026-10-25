"""A cached boot receipt must not mask permissions lost during a GMS update."""
import importlib.util
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

TOOLS = Path(__file__).resolve().parents[2] / 'handoff/google-play-stepx/tools'
sys.path.insert(0, str(TOOLS))
spec = importlib.util.spec_from_file_location('google_manage', TOOLS / 'manage.py')
manage = importlib.util.module_from_spec(spec)
spec.loader.exec_module(manage)


class GoogleRuntimeStatusTests(unittest.TestCase):
    def test_reads_current_grants_not_old_receipt(self):
        receipt = ['check com.google.android.gms android.permission.MANAGE_USERS=0']
        with patch.object(manage, 'shell', return_value='  android.permission.MANAGE_USERS: granted=false'):
            self.assertEqual(manage.current_permission_checks(receipt),
                             ['com.google.android.gms:android.permission.MANAGE_USERS'])
        with patch.object(manage, 'shell', return_value='  android.permission.MANAGE_USERS: granted=true, flags=[ GRANTED_BY_DEFAULT ]'):
            self.assertEqual(manage.current_permission_checks(receipt), [])

    def test_other_package_or_similar_permission_cannot_satisfy_check(self):
        receipt = ['check com.google.android.gms android.permission.MANAGE_USERS=0']
        def output(*args):
            return ('  android.permission.MANAGE_USERS: granted=true' if args[-1] == 'com.android.vending'
                    else '  vendor.android.permission.MANAGE_USERS: granted=true')
        with patch.object(manage, 'shell', side_effect=output):
            self.assertEqual(len(manage.current_permission_checks(receipt)), 1)

    def test_malformed_or_failed_receipt_is_rejected(self):
        with patch.object(manage, 'shell', return_value='  android.permission.MANAGE_USERS: granted=true'):
            for receipt in ['check unknown android.permission.MANAGE_USERS=0',
                            'check com.google.android.gms android.permission.MANAGE_USERS=-1']:
                self.assertEqual(manage.current_permission_checks([receipt]), ['unrecognized_receipt'])


if __name__ == '__main__':
    unittest.main()

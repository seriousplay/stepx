#!/usr/bin/env python3
"""Regression checks for reuse of a pairing process after Android restarts.
Loads pure orchestration functions with fake device I/O; never contacts a phone.
"""
import ast
from pathlib import Path
import unittest

class SessionTests(unittest.TestCase):
    def context(self, previous):
        source=ast.parse(Path(__file__).with_name('companion-system.py').read_text())
        module=ast.Module(body=[n for n in source.body if isinstance(n,ast.FunctionDef) and n.name in ('status','start')],type_ignores=[])
        identity={'dex_sha256':'dex','signer':'cert','companion_uid':10200,'system_session':{'pid':100,'start_ticks':'20'}}
        scope={'preflight':lambda apk:identity,'saved_service':lambda identity:previous}
        exec(compile(module,'companion-system.py','exec'),scope)
        return scope,identity
    def test_reused_pid_with_different_start_time_is_not_current(self):
        previous={'dex_sha256':'dex','signer':'cert','companion_uid':10200,'process':{'pid':400},'system_session':{'pid':100,'start_ticks':'19'}}
        scope,_=self.context(previous)
        self.assertFalse(scope['status'](None)['artifact_current'])
        with self.assertRaisesRegex(RuntimeError,'Stop the verified old'):
            scope['start'](None)
    def test_legacy_receipt_requires_one_verified_service_restart(self):
        scope,_=self.context({'dex_sha256':'dex','signer':'cert','companion_uid':10200,'process':{'pid':400}})
        self.assertFalse(scope['status'](None)['artifact_current'])
    def test_current_session_reuses_service_without_reinstall(self):
        previous={'dex_sha256':'dex','signer':'cert','companion_uid':10200,'process':{'pid':400},'system_session':{'pid':100,'start_ticks':'20'}}
        scope,_=self.context(previous)
        self.assertTrue(scope['status'](None)['artifact_current'])
        self.assertEqual(scope['start'](None)['status'],'already_running')
if __name__=='__main__':unittest.main()

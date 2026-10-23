"""The pairing daemon may be stopped/reused only with a matching installation identity."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('companion_system',ROOT/'scripts/companion-system.py')
service=importlib.util.module_from_spec(spec);spec.loader.exec_module(service)


class PairingServiceGuards(unittest.TestCase):
    def setUp(self):
        self.identity={'service':'stepx-pairing-v1','package':service.permissions.PACKAGE,
            'serial':service.permissions.ADB[-1],'fingerprint':'fixed','boot_id':'boot',
            'signer':'a'*64,'companion_uid':10215,'dex_sha256':'b'*64}
        self.process={'pid':4321,'start_ticks':'123','uid':2000,'context':'u:r:shell:s0',
            'executable':service.APP_PROCESS,'argv':[service.MAIN]}

    def test_unknown_existing_service_is_not_adopted_or_replaced(self):
        with tempfile.TemporaryDirectory() as directory,patch.object(service,'STATE',Path(directory)), \
                patch.object(service,'candidates',return_value=[4321]),patch.object(service,'process') as process:
            with self.assertRaisesRegex(RuntimeError,'without this installation record'):
                service.saved_service(self.identity)
            process.assert_not_called()

    def test_pid_reuse_never_triggers_stop_signal(self):
        prior={**self.identity,'process':self.process}
        changed={**self.process,'start_ticks':'124'}
        with patch.object(service,'preflight',return_value=self.identity), \
                patch.object(service,'saved_service',return_value=prior), \
                patch.object(service,'process',return_value=changed),patch.object(service.permissions,'shell') as shell:
            with self.assertRaisesRegex(RuntimeError,'changed before stop'):
                service.stop(Path('app.apk'))
            shell.assert_not_called()

    def test_new_artifact_requires_explicit_stop_of_verified_old_service(self):
        prior={**self.identity,'dex_sha256':'c'*64,'process':self.process}
        with patch.object(service,'preflight',return_value=self.identity), \
                patch.object(service,'saved_service',return_value=prior),patch.object(service.permissions,'shell') as shell:
            with self.assertRaisesRegex(RuntimeError,'Stop the verified old'):
                service.start(Path('app.apk'))
            shell.assert_not_called()

    def test_existing_current_service_start_is_read_only(self):
        prior={**self.identity,'process':self.process}
        with patch.object(service,'preflight',return_value=self.identity), \
                patch.object(service,'saved_service',return_value=prior),patch.object(service.permissions,'shell') as shell:
            result=service.start(Path('app.apk'))
            self.assertEqual(result['status'],'already_running');shell.assert_not_called()

    def test_exact_launch_arguments_are_required(self):
        valid={**self.process,'argv':[service.APP_PROCESS,'/system/bin',service.MAIN,'a'*64,'10215']}
        service.validate_launch_identity(valid,self.identity)
        for argv in ([service.MAIN,'--arbitrary'],[service.APP_PROCESS,'/system/bin',service.MAIN,'a'*64,'10216']):
            with self.assertRaisesRegex(RuntimeError,'fixed Java entry'):
                service.validate_launch_identity({**valid,'argv':argv},self.identity)

    def test_old_boot_record_does_not_authorize_touching_new_process(self):
        with tempfile.TemporaryDirectory() as directory,patch.object(service,'STATE',Path(directory)), \
                patch.object(service,'candidates',return_value=[4321]),patch.object(service,'process') as process:
            service.record_path().write_text(json.dumps({**self.identity,'boot_id':'old-boot','process':self.process}))
            with self.assertRaisesRegex(RuntimeError,'without this installation record'):
                service.saved_service(self.identity)
            process.assert_not_called()


if __name__=='__main__':unittest.main()

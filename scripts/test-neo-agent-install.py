#!/usr/bin/env python3
"""No device writes: exercise certificate rejection and scoped recovery with a fake transport."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('neo_install', Path(__file__).with_name('neo-agent-install.py'))
m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)


class Fake(m.Installer):
    def __init__(self, folder):
        self.folder = Path(folder); self.receipt = self.folder/'setup.json'
        self.identity = {'device':'digest','fingerprint':'test'}
        self.adb = ['fake-adb']; self.present = True; self.cert = 'a'*64
        self.settings = {'enabled_accessibility_services':'other.app/.Service', 'accessibility_enabled':'1',
                         'enabled_input_methods':'keyboard/.Ime', 'default_input_method':'keyboard/.Ime'}
        self.doze_on = False; self.freeze = {'cloud':False,'user':False,'local':False}
        self.commands = []; self.fail_ime = False
    def setting(self, name): return self.settings.get(name,'null')
    def put_setting(self, name, value): self.commands.append(('setting',name,value)); self.settings[name] = value
    def installed_cert(self, package, backup=None):
        return self.cert if self.present else None
    def installed_uid(self): return 10200
    def has_embedded_runtime(self): return True
    def doze(self): return self.doze_on
    def freezer(self): return self.freeze.copy()
    def status(self, verify=False):
        return {'installed':self.present, 'accessibility_enabled':m.contains(self.setting('enabled_accessibility_services'),m.ACCESS),
                'ime_enabled':m.contains(self.setting('enabled_input_methods'),m.IME),'doze_exempt':self.doze(),
                'oem_freezer_exempt':self.freezer()}
    def shell(self,*args):
        self.commands.append(args)
        if args[:2] == ('ime','enable'):
            if self.fail_ime: raise RuntimeError('IME enable failed')
            self.settings['enabled_input_methods']=m.add_component(self.setting('enabled_input_methods'),args[-1])
        elif args[:2] == ('ime','disable'): self.settings['enabled_input_methods']=m.remove_component(self.setting('enabled_input_methods'),args[-1])
        elif args[:2] == ('ime','set'): self.settings['default_input_method']=args[-1]
        elif args[:3] == ('dumpsys','deviceidle','whitelist'):
            self.doze_on=args[-1].startswith('+')
        elif args[:4] == ('dumpsys','activity','freeze','whiteAdd'): self.freeze.update(cloud=True,local=True)
        elif args[:4] == ('dumpsys','activity','freeze','whiteRemove'): self.freeze.update(cloud=False,local=False)
        else: raise AssertionError(args)
        return ''
    def baseline(self, **owned):
        return {'schema':2,'scope':m.SCOPE,'identity':self.identity,'package':m.PACKAGE,'signer':self.cert,'uid':10200,
                'before':dict(self.settings),**owned}


class RecoveryTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup);self.phone=Fake(self.temp.name)
        self.manifest = {'package':m.PACKAGE,'components':[
            {'type':'service','name':m.NEO_NAMESPACE+'.ControlService','permission':'android.permission.BIND_ACCESSIBILITY_SERVICE'},
            {'type':'service','name':m.NEO_NAMESPACE+'.AgentInputMethodService','permission':'android.permission.BIND_INPUT_METHOD'},
            {'type':'provider','name':m.NEO_NAMESPACE+'.CompanionBridgeProvider','authorities':m.AUTHORITY},
            {'type':'activity','name':'.NeoSettingsActivity'}]}
        manifest_patch = patch.object(m,'apk_manifest',return_value=self.manifest)
        manifest_patch.start(); self.addCleanup(manifest_patch.stop)
    def test_sdk_signer_formats_and_multiple_signers(self):
        digest='a'*64
        self.assertEqual(m.signer_digests('Signer #1 certificate SHA-256 digest: '+digest),{digest})
        self.assertEqual(m.signer_digests('V3.0 Signer: certificate SHA-256 digest: '+digest),{digest})
        self.assertEqual(len(m.signer_digests('Signer #1 certificate SHA-256 digest: '+digest+'\nSigner #2 certificate SHA-256 digest: '+'b'*64)),2)
    def test_alias_and_preserve_unrelated_components(self):
        full=m.canonical(m.ACCESS)
        self.assertEqual(m.add_component(full,m.ACCESS),full)
        self.assertEqual(m.remove_component('x/.A:'+full+':y/.B',m.ACCESS),'x/.A:y/.B')
    def test_restore_preserves_services_added_by_user(self):
        v=self.phone.baseline(access_added=True,doze_added=True)
        self.phone.settings['enabled_accessibility_services'] += ':'+m.ACCESS+':user.new/.Service'
        self.phone.doze_on=True
        self.phone.restore_settings(v)
        self.assertEqual(self.phone.setting('enabled_accessibility_services'),'other.app/.Service:user.new/.Service')
        self.assertTrue(self.phone.doze_on)
        self.assertEqual(v['phase'],'restored')
    def test_preexisting_grants_are_not_removed(self):
        self.phone.settings['enabled_accessibility_services']=m.ACCESS
        self.phone.doze_on=True
        v=self.phone.baseline()
        self.phone.restore_settings(v)
        self.assertEqual(self.phone.commands,[])
        self.assertTrue(self.phone.doze_on)
    def test_original_disabled_accessibility_master_restored(self):
        self.phone.settings['accessibility_enabled']='0'
        v=self.phone.baseline(access_added=True,master_changed=True)
        self.phone.settings['enabled_accessibility_services'] += ':'+m.ACCESS
        self.phone.settings['accessibility_enabled']='1'
        self.phone.restore_settings(v)
        self.assertEqual(self.phone.setting('accessibility_enabled'),'0')
    def test_new_keyboard_chosen_by_user_is_preserved(self):
        v=self.phone.baseline(ime_added=True)
        self.phone.settings['enabled_input_methods'] += ':'+m.IME+':user.keyboard/.Ime'
        self.phone.settings['default_input_method']='user.keyboard/.Ime'
        self.phone.restore_settings(v)
        self.assertEqual(self.phone.setting('default_input_method'),'user.keyboard/.Ime')
        self.assertFalse(m.contains(self.phone.setting('enabled_input_methods'),m.IME))
    def test_task_keyboard_restores_original_before_disable(self):
        v=self.phone.baseline(ime_added=True)
        self.phone.settings['enabled_input_methods'] += ':'+m.IME
        self.phone.settings['default_input_method']=m.IME
        self.phone.restore_settings(v)
        self.assertEqual(self.phone.commands[0],('ime','set','--user','0','keyboard/.Ime'))
        self.assertEqual(self.phone.commands[1],('ime','disable','--user','0',m.IME))
    def test_missing_original_keyboard_does_not_leave_no_keyboard(self):
        v=self.phone.baseline(ime_added=True)
        self.phone.settings['enabled_input_methods']=m.IME
        self.phone.settings['default_input_method']=m.IME
        with self.assertRaisesRegex(RuntimeError,'原输入法不可用'):self.phone.restore_settings(v)
        self.assertEqual(self.phone.commands,[])
        self.assertEqual(v['phase'],'restore_pending')
    def test_changed_certificate_blocks_recovery(self):
        v=self.phone.baseline(access_added=True)
        self.phone.cert='b'*64
        with self.assertRaisesRegex(RuntimeError,'签名已变化'):self.phone.restore_settings(v)
        self.assertEqual(self.phone.commands,[])
    def test_user_oem_exemption_not_removed(self):
        v=self.phone.baseline(freezer_added=True)
        self.phone.freeze={'cloud':True,'local':True,'user':True}
        self.phone.restore_settings(v)
        self.assertEqual(self.phone.commands,[])
        self.assertEqual(self.phone.freeze, {'cloud':True,'local':True,'user':True})
    def test_wrong_apk_package_rejected_before_changes(self):
        apk=Path(self.temp.name)/'input.apk';apk.write_bytes(b'apk')
        with patch.object(m,'apk_identity',return_value=('other.app','a'*64)):
            with self.assertRaisesRegex(RuntimeError,'包名必须'):self.phone.install(apk,True)
        self.assertEqual(self.phone.commands,[]);self.assertFalse(self.phone.receipt.exists())
    def test_wrong_signer_rejected_before_changes(self):
        apk=Path(self.temp.name)/'input.apk';apk.write_bytes(b'apk')
        with patch.object(m,'apk_identity',return_value=(m.PACKAGE,'b'*64)):
            with self.assertRaisesRegex(RuntimeError,'同一签名'):self.phone.install(apk,True)
        self.assertEqual(self.phone.commands,[]);self.assertFalse(self.phone.receipt.exists())
    def test_failure_rolls_back_added_permissions_preserves_user_data(self):
        apk=Path(self.temp.name)/'input.apk';apk.write_bytes(b'apk')
        self.phone.fail_ime=True
        before=dict(self.phone.settings)
        with patch.object(m,'apk_identity',return_value=(m.PACKAGE,'a'*64)),patch.object(m,'run',return_value='Success'):
            with self.assertRaisesRegex(RuntimeError,'权限配置已恢复'):self.phone.install(apk,True)
        self.assertEqual(before,self.phone.settings)
        self.assertTrue(self.phone.present)
        self.assertFalse(any('uninstall' in command for command in self.phone.commands))
    def test_success_does_not_change_default_keyboard(self):
        apk=Path(self.temp.name)/'input.apk';apk.write_bytes(b'apk')
        with patch.object(m,'apk_identity',return_value=(m.PACKAGE,'a'*64)),patch.object(m,'run',return_value='Success'):
            result=self.phone.install(apk,True)
        self.assertTrue(result['accessibility_enabled']);self.assertTrue(result['ime_enabled'])
        self.assertEqual(self.phone.setting('default_input_method'),'keyboard/.Ime')
        self.assertFalse(self.phone.doze_on)
        self.assertEqual(self.phone.freeze, {'cloud':False,'user':False,'local':False})

    def test_embedded_manifest_requires_all_components(self):
        self.assertTrue(m.embedded_runtime(self.manifest))
        for index in range(len(self.manifest['components'])):
            missing = {'package':m.PACKAGE,'components':self.manifest['components'][:index]+self.manifest['components'][index+1:]}
            self.assertFalse(m.embedded_runtime(missing))
    def test_wrong_authority_and_disabled_control_rejected(self):
        self.manifest['components'][2]['authorities']='com.stepx.companion.neo.control'
        self.assertFalse(m.embedded_runtime(self.manifest))
        self.manifest['components'][2]['authorities']=m.AUTHORITY
        self.manifest['components'][0]['enabled']=False
        self.assertFalse(m.embedded_runtime(self.manifest))
    def test_wrong_component_permission_rejected(self):
        self.manifest['components'][0]['permission']='android.permission.INTERNET'
        self.assertFalse(m.embedded_runtime(self.manifest))
    def test_manifest_rejection_precedes_device_changes(self):
        apk=Path(self.temp.name)/'input.apk';apk.write_bytes(b'apk')
        self.manifest['components'].clear()
        with patch.object(m,'apk_identity',return_value=(m.PACKAGE,'a'*64)):
            with self.assertRaisesRegex(RuntimeError,'未内置完整'):self.phone.install(apk,True)
        self.assertEqual(self.phone.commands,[]);self.assertFalse(self.phone.receipt.exists())
    def test_legacy_standalone_receipt_cannot_modify_shared_package(self):
        v=self.phone.baseline(access_added=True);v.update(schema=1,package='com.stepx.companion.neo')
        with self.assertRaisesRegex(RuntimeError,'恢复记录'):self.phone.restore_settings(v)
        self.assertEqual(self.phone.commands,[])
    def test_restore_keeps_other_companion_services_and_permissions(self):
        original=m.PACKAGE+'/.ScreenControlService:other.app/.Service'
        self.phone.settings['enabled_accessibility_services']=original
        self.phone.doze_on=True;self.phone.freeze={'cloud':True,'local':True,'user':False}
        v=self.phone.baseline(access_added=True,doze_added=True,freezer_added=True)
        self.phone.settings['enabled_accessibility_services']+=':'+m.ACCESS
        self.phone.restore_settings(v)
        self.assertEqual(self.phone.setting('enabled_accessibility_services'),original)
        self.assertTrue(self.phone.doze_on)
        self.assertEqual(self.phone.freeze,{'cloud':True,'local':True,'user':False})
        self.assertEqual(self.phone.commands,[('setting','enabled_accessibility_services',original)])
    def test_repeated_updates_retain_first_permission_baseline(self):
        apk=Path(self.temp.name)/'input.apk';apk.write_bytes(b'apk')
        before=dict(self.phone.settings)
        with patch.object(m,'apk_identity',return_value=(m.PACKAGE,'a'*64)),patch.object(m,'run',return_value='Success'):
            self.phone.install(apk,True);self.phone.install(apk,True)
        self.phone.restore()
        self.assertEqual(self.phone.settings,before)


if __name__=='__main__':unittest.main()

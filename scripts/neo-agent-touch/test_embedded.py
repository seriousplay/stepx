import unittest
import xml.etree.ElementTree as ET
from embedded import ANDROID, HOST, merge_manifest


class EmbeddedManifestTest(unittest.TestCase):
    def host(self):
        return ET.ElementTree(ET.fromstring('''<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="com.stepx.companion.app" android:versionCode="14"><uses-sdk android:minSdkVersion="33"/><application android:label="StepX伴侣"><activity android:name=".MainActivity" android:exported="true"><intent-filter><category android:name="android.intent.category.LAUNCHER"/></intent-filter></activity><provider android:name=".StudioProvider" android:authorities="com.stepx.companion.app.studio"/></application></manifest>'''))
    def neo(self):
        return ET.ElementTree(ET.fromstring('''<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="com.stepx.companion.neo"><uses-permission android:name="android.permission.INTERNET"/><queries><intent><action android:name="android.view.InputMethod"/></intent></queries><application android:theme="@style/neo_Theme.NeoAgent"><activity android:name=".MainActivity" android:exported="true"><intent-filter><category android:name="android.intent.category.LAUNCHER"/></intent-filter></activity><service android:name=".ControlService" android:permission="android.permission.BIND_ACCESSIBILITY_SERVICE"/><service android:name=".AgentInputMethodService" android:permission="android.permission.BIND_INPUT_METHOD"/><provider android:name=".CompanionBridgeProvider" android:authorities="com.stepx.companion.neo.control" android:exported="true"/></application></manifest>'''))
    def test_one_host_identity_with_private_neo_activity(self):
        merged=merge_manifest(self.host(),self.neo(),'com.stepx.companion.neo').getroot()
        self.assertEqual(merged.get('package'),HOST)
        self.assertEqual(merged.get(ANDROID+'versionCode'),'14')
        app=merged.find('application')
        self.assertEqual(app.get(ANDROID+'label'),'StepX伴侣')
        activities=app.findall('activity')
        self.assertEqual(activities[0].get(ANDROID+'name'),'.MainActivity')
        self.assertEqual(activities[1].get(ANDROID+'name'),'com.stepx.companion.neo.MainActivity')
        self.assertEqual(activities[1].get(ANDROID+'exported'),'false')
        self.assertEqual(activities[1].findall('intent-filter'),[])
        self.assertEqual(app.findall('provider')[-1].get(ANDROID+'authorities'),HOST+'.neo.control')
        self.assertEqual(app.findall('service')[0].get(ANDROID+'permission'),'android.permission.BIND_ACCESSIBILITY_SERVICE')
        self.assertEqual(len(merged.find('queries')),1)
    def test_unreviewed_permission_is_rejected(self):
        neo=self.neo()
        ET.SubElement(neo.getroot(),'uses-permission',{ANDROID+'name':'android.permission.READ_SMS'})
        with self.assertRaisesRegex(RuntimeError,'Unexpected new embedded permission'):
            merge_manifest(self.host(),neo,'com.stepx.companion.neo')
    def test_duplicate_authority_is_rejected(self):
        host=self.host()
        ET.SubElement(host.getroot().find('application'),'provider',{ANDROID+'name':'.Conflicting',ANDROID+'authorities':HOST+'.neo.control'})
        with self.assertRaisesRegex(RuntimeError,'Provider authority collision'):
            merge_manifest(host,self.neo(),'com.stepx.companion.neo')
    def test_host_identity_cannot_change(self):
        host=self.host();host.getroot().set('package','untrusted.app')
        with self.assertRaisesRegex(RuntimeError,'Unexpected companion package'):
            merge_manifest(host,self.neo(),'com.stepx.companion.neo')


if __name__ == '__main__':
    unittest.main()

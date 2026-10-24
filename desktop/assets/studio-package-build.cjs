// Fixed packaging for a self-contained Companion HTML app. User HTML is copied,
// never executed as a build script. No package downloads or model-written shell.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const cp = require('node:child_process');
const p = JSON.parse(fs.readFileSync('package.json', 'utf8'));
const manifest = p.companionManifest;
if (!manifest || !/^ai_p_[A-Za-z0-9_-]+$/.test(manifest.applicationId)) throw Error('Invalid app manifest');
const out = 'dist/client';
fs.mkdirSync(out + '/web', { recursive: true });
fs.copyFileSync('index.html', out + '/web/index.html');
fs.writeFileSync(out + '/manifest.json', JSON.stringify(manifest, null, 2));
fs.mkdirSync(out + '/signing', { recursive: true });
const signing = out + '/signing/capsule.p12';
if (!fs.existsSync(signing)) {
  const password = crypto.randomBytes(24).toString('hex');
  fs.writeFileSync(out + '/signing/capsule.properties', 'storePassword=' + password + '\nkeyAlias=companion\nkeyPassword=' + password + '\n', { mode: 0o600 });
  const keytool = process.env.JAVA_HOME ? path.join(process.env.JAVA_HOME, 'bin', 'keytool') : 'keytool';
  const result = cp.spawnSync(keytool, ['-genkeypair', '-noprompt', '-keystore', signing, '-storetype', 'PKCS12', '-alias', 'companion', '-keyalg', 'RSA', '-keysize', '2048', '-validity', '3650', '-storepass', password, '-keypass', password, '-dname', 'CN=Companion Preview'], { stdio: 'ignore' });
  if (result.status !== 0) throw Error('The Studio builder needs JDK keytool to package this preview');
}
console.log('Companion HTML package ready: dist/client');

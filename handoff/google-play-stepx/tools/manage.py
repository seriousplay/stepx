#!/usr/bin/env python3
"""Explicit workflow steps; invoking --help or preflight never installs anything."""
import argparse
import base64
import json
import re
import shutil
import subprocess
import sys
import time
import urllib.request
import zipfile
from common import ADB, CONFIG, ROOT, STATE, guard, init_state, private_write, root_json, run, save_json, sdk_tool, sha256, shell

PACKAGES = ['com.google.android.gsf', 'com.google.android.gms', 'com.android.vending']


def preflight():
    guard(native=True)
    result = {
        'fingerprint': shell('getprop', 'ro.build.fingerprint'),
        'boot_completed': shell('getprop', 'sys.boot_completed'),
        'user_unlocked': 'RUNNING_UNLOCKED' in shell('dumpsys', 'user'),
        'avb': shell('getprop', 'ro.boot.verifiedbootstate'),
        'bootloader_locked': shell('getprop', 'ro.boot.flash.locked'),
        'selinux': shell('getenforce'),
        'google_packages': {p: bool(shell('pm', 'list', 'packages', '--user', '0', p)) for p in PACKAGES},
    }
    print(json.dumps(result, indent=2))
    return result


def fetch(archive=None):
    folder = ROOT / 'downloads'
    folder.mkdir(exist_ok=True)
    meta = CONFIG['archive']
    if archive:
        path = __import__('pathlib').Path(archive).resolve()
    else:
        path = folder / meta['name']
        if not path.exists():
            temp = path.with_suffix('.partial')
            with urllib.request.urlopen(meta['url'], timeout=60) as source, open(temp, 'wb') as target:
                shutil.copyfileobj(source, target)
            assert sha256(temp) == meta['sha256'], 'Archive digest mismatch'
            temp.replace(path)
    assert sha256(path) == meta['sha256'], 'Archive digest mismatch'
    with zipfile.ZipFile(path) as z:
        for entry in CONFIG['apks']:
            dest = folder / entry['filename']
            dest.write_bytes(z.read(entry['zip_path']))
    check_apks()


def check_apks():
    for entry in CONFIG['apks']:
        path = ROOT / 'downloads' / entry['filename']
        assert sha256(path) == entry['sha256'], 'APK digest mismatch: ' + entry['package']
        result = run([sdk_tool('apksigner'), 'verify', '--print-certs', str(path)])
        assert 'Signer #1 certificate SHA-256 digest: ' + entry['signer_sha256'] in result.stdout
        print(entry['package'] + ': APK digest and signer OK')


def prepare():
    current = preflight()
    assert current['user_unlocked'], 'Unlock the phone first'
    assert not any(current['google_packages'].values()), 'Google packages already exist; use maintenance workflow'
    all_packages = shell('pm', 'list', 'packages', '-u')
    assert all('package:' + p + '\n' not in all_packages + '\n' for p in PACKAGES), 'Prior Google package state exists'
    assert re.findall(r'UserInfo\{(\d+):', shell('pm', 'list', 'users')) == ['0'], 'Only user 0 was validated'
    init_state()
    assert not (STATE / 'baseline.json').exists(), 'Do not overwrite an existing baseline'
    baseline = {'serial': ADB[-1], 'fingerprint': CONFIG['fingerprint'],
                'packages': [x.removeprefix('package:') for x in shell('pm', 'list', 'packages').splitlines()],
                'google_core': shell('settings', 'get', 'global', 'cn_gms_core'),
                'autofill': shell('settings', 'get', 'secure', 'autofill_service'),
                'startup_path': shell('getprop', 'persist.openclaw.zygote.path'),
                'start_trigger': shell('getprop', 'sys.openclaw.env.start')}
    assert baseline['startup_path'] == baseline['start_trigger'] == '', 'Non-default startup properties'
    lines = shell('ps', '-A', '-o', 'PID,PPID,NAME').splitlines()
    parent = next(int(x.split()[0]) for x in lines if x.split()[-1] == 'openclaw_env' and x.split()[1] == '1')
    code = '''import os,json,hashlib,base64
h=HOST
assert not os.path.exists('/usr/local/lib/stepx-gms'), 'Adapter already exists'
assert not os.path.exists('/usr/local/bin/ubuntu_zygote.stepx-stock-20261021'), 'Stock backup already exists'
p='/usr/local/bin/ubuntu_zygote';data=open(p,'rb').read()
r={'stock':base64.b64encode(data).decode(),'stock_sha256':hashlib.sha256(data).hexdigest(),'files':{}}
for path in ['/data/system/packages.xml','/data/misc_de/0/apexdata/com.android.permission/access.abx']:
 r['files'][path]=base64.b64encode(open(h+path,'rb').read()).decode()
print(json.dumps(r))
'''.replace('HOST', repr('/proc/' + str(parent) + '/root'))
    proof = root_json(code)
    assert proof['stock_sha256'] == CONFIG['stock_sha256'], 'Unexpected factory launcher'
    private_write(STATE / 'ubuntu_zygote.stock', base64.b64decode(proof['stock']))
    for path, encoded in proof['files'].items():
        private_write(STATE / ('before-' + path.rsplit('/', 1)[-1]), base64.b64decode(encoded))
    save_json('baseline.json', baseline)
    print('Original launcher and package/permission metadata saved privately. No Google APK installed yet.')


def install_apks():
    guard(native=True)
    baseline = json.loads((STATE / 'baseline.json').read_text())
    assert baseline['fingerprint'] == CONFIG['fingerprint']
    assert baseline.get('serial', CONFIG['serial']) == ADB[-1], 'Wrong device baseline'
    assert all(p not in baseline['packages'] for p in PACKAGES)
    assert all(not shell('pm', 'list', 'packages', '--user', '0', p) for p in PACKAGES), 'Do not overwrite an existing installation'
    check_apks()
    for package in PACKAGES:
        meta = next(x for x in CONFIG['apks'] if x['package'] == package)
        result = run(ADB + ['install', '--user', '0', str(ROOT / 'downloads' / meta['filename'])], timeout=180)
        assert 'Success' in result.stdout, 'Installation not confirmed: ' + package
        print('Installed ' + package, flush=True)
    shell('settings', 'put', 'global', 'cn_gms_core', '1')
    for package in PACKAGES:
        shell('pm', 'grant', package, 'android.permission.INTERACT_ACROSS_USERS')
    shell('pm', 'grant', 'com.google.android.gms', 'android.permission.WRITE_SECURE_SETTINGS')
    for package in ['com.google.android.gms', 'com.android.vending']:
        shell('cmd', 'appops', 'set', package, 'SYSTEM_EXEMPT_FROM_POWER_RESTRICTIONS', 'allow')
    print('APKs installed. Do not log in before adapter verification and an unlogged reboot test.')


def status():
    guard()
    init_state()
    result = root_json("import os,json; p='/var/lib/stepx-gms/last.json'; print(json.dumps({'boot_id':open('/proc/sys/kernel/random/boot_id').read().strip(),'adapter':json.load(open(p)) if os.path.exists(p) else None}))")
    adapter = result['adapter']
    save_json('adapter-status.json', result)
    if not adapter:
        print('No adapter status. Unlock phone and verify StepClaw is running.'); return
    checks = [x for x in adapter.get('java_result', '').splitlines() if x.startswith('check ')]
    summary = {k: adapter.get(k) for k in ['status', 'target_pid', 'unix_time', 'detached', 'error']}
    summary.update(current_boot_matches=adapter.get('identity') == result['boot_id'],
                   current_pid_matches=str(adapter.get('target_pid')) == shell('pidof', 'system_server'),
                   permission_checks=len(checks), all_checked_granted=bool(checks) and all(x.endswith('=0') for x in checks))
    print(json.dumps(summary, indent=2))
    assert summary['status'] == 'success' and summary['detached']
    assert summary['current_boot_matches'] and summary['current_pid_matches']
    assert len(checks) >= 120 and summary['all_checked_granted']


def verify():
    guard()
    account_dump = shell('dumpsys', 'account')
    result = {'device_epoch': int(shell('date', '+%s')),
              'google_account_count': len(re.findall(r'Account \{[^\n]*type=com\.google\}', account_dump)),
              'system_server_pid': shell('pidof', 'system_server'), 'selinux': shell('getenforce'),
              'user_unlocked': 'RUNNING_UNLOCKED' in shell('dumpsys', 'user'),
              'google_core': shell('settings', 'get', 'global', 'cn_gms_core'),
              'versions': {}, 'test_apps': {}, 'fatal_events': []}
    for package in PACKAGES:
        data = shell('dumpsys', 'package', package)
        match = re.search(r'versionName=([^\n]+)', data)
        result['versions'][package] = match.group(1) if match else None
    for package in ['com.google.android.calculator', 'com.google.android.deskclock']:
        data = shell('dumpsys', 'package', package)
        result['test_apps'][package] = {key: (m.group(1) if (m := re.search(r'\b' + key + r'=([^\s]+)', data)) else None)
                                       for key in ['versionName', 'installerPackageName', 'initiatingPackageName']}
    logs = run(ADB + ['logcat', '-b', 'crash', '-d', '-v', 'epoch']).stdout
    for block in re.split(r'(?=\s*\d{10}\.\d+\s+\d+\s+\d+ E AndroidRuntime: FATAL EXCEPTION)', logs):
        process = re.search(r'Process: (com\.google\.android\.(?:gms|gsf)[^,\s]*|com\.android\.vending[^,\s]*),', block)
        stamp = re.search(r'(\d{10}\.\d+)', block)
        if 'FATAL EXCEPTION' in block and process and stamp:
            result['fatal_events'].append({'device_epoch': float(stamp.group(1)), 'process': process.group(1)})
    # Never persist the account dump, account names, or raw logs.
    save_json('verification-' + str(result['device_epoch']) + '.json', result)
    print(json.dumps(result, indent=2))


def adopt(source):
    from pathlib import Path
    guard()
    init_state()
    assert not (STATE / 'baseline.json').exists(), 'Existing baseline must not be overwritten'
    origin = Path(source).resolve()
    baseline = json.loads((origin / 'baseline.json').read_text())
    install = json.loads((origin / 'boot-install.json').read_text())
    assert baseline['fingerprint'] == CONFIG['fingerprint']
    assert baseline.get('serial', CONFIG['serial']) == ADB[-1], 'Backup belongs to a different device'
    assert sha256(origin / 'ubuntu_zygote.stock') == CONFIG['stock_sha256'] == install['original_sha256']
    current = root_json("import hashlib,json;print(json.dumps({'sha':hashlib.sha256(open('/usr/local/bin/ubuntu_zygote','rb').read()).hexdigest()}))")
    assert current['sha'] == install['launcher_sha256'], 'Installed launcher differs from recovery metadata'
    for name in ['baseline.json', 'boot-install.json', 'ubuntu_zygote.stock']:
        private_write(STATE / name, (origin / name).read_bytes())
    print('Existing private rollback metadata imported; nothing changed on phone.')


def recover_metadata():
    """Recover a lost install response only after exact read-back of a complete install."""
    guard(native=True)
    baseline = json.loads((STATE / 'baseline.json').read_text())
    assert baseline['fingerprint'] == CONFIG['fingerprint']
    assert baseline.get('serial', CONFIG['serial']) == ADB[-1]
    assert sha256(STATE / 'ubuntu_zygote.stock') == CONFIG['stock_sha256']
    assert not (STATE / 'boot-install.json').exists(), 'Existing recovery metadata must be reviewed, not overwritten'
    expected = {p.name: sha256(p) for p in (ROOT / 'module').glob('*.py')}
    code = '''import json,hashlib
def digest(p):return hashlib.sha256(open(p,'rb').read()).hexdigest()
base='/usr/local/lib/stepx-gms'
manifest=json.load(open(base+'/manifest.json'))
actual={n:digest('/usr/local/bin/ubuntu_zygote' if n=='launcher.py' else base+'/'+n) for n in ['launcher.py','boot.py','inject.py']}
print(json.dumps({'manifest':manifest,'actual':actual,'stock':digest('/usr/local/bin/ubuntu_zygote.stepx-stock-20261021')}))
'''
    proof = root_json(code)
    assert proof['manifest'] == proof['actual'] == expected, 'Partial or different install; manual review required'
    assert proof['stock'] == CONFIG['stock_sha256']
    save_json('boot-install.json', {'installed': True, 'original_sha256': proof['stock'],
                                 'launcher_sha256': expected['launcher.py'], 'manifest': expected,
                                 'recovered_from_exact_readback': True})
    print('Local rollback metadata recovered from verified device files; phone unchanged.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    for name in ['preflight', 'check-apks', 'prepare', 'install-apks', 'status', 'verify', 'certification', 'recover-adapter-metadata']:
        commands.add_parser(name)
    commands.add_parser('fetch').add_argument('--archive')
    commands.add_parser('adopt-backup').add_argument('directory')
    args = parser.parse_args()
    if args.command == 'certification':
        from certification import inspect
        print(json.dumps(inspect(), ensure_ascii=False, indent=2))
        return
    if args.command == 'fetch': fetch(args.archive)
    elif args.command == 'adopt-backup': adopt(args.directory)
    else: {'preflight': preflight, 'check-apks': check_apks, 'prepare': prepare,
           'install-apks': install_apks, 'status': status, 'verify': verify,
           'recover-adapter-metadata': recover_metadata}[args.command]()


if __name__ == '__main__':
    main()

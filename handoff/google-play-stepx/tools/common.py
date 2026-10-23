"""Host helpers. All device operations are pinned to one explicitly selected serial."""
import hashlib
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
CONFIG = json.loads((ROOT / 'target.json').read_text())
STATE = ROOT / 'state'
SERIAL = os.environ.get('STEPX_SERIAL', CONFIG['serial'])
ADB = [os.environ.get('STEPX_ADB', shutil.which('adb') or 'adb'), '-s', SERIAL]


def run(args, **kwargs):
    return subprocess.run(args, text=True, capture_output=True, check=True,
                          timeout=kwargs.pop('timeout', 40), **kwargs)


def shell(*args):
    # adb shell joins arguments; quote empty values and metacharacters explicitly.
    return run(ADB + ['shell', shlex.join(str(x) for x in args)]).stdout.strip()


def guard(native=False):
    if not SERIAL or ':' in SERIAL:
        raise RuntimeError('Use an explicitly authorized USB device serial')
    assert shell('getprop', 'ro.build.fingerprint') == CONFIG['fingerprint'], 'Unsupported firmware'
    assert shell('getprop', 'ro.build.version.sdk') == CONFIG['sdk'], 'Unsupported SDK'
    assert shell('getprop', 'ro.product.cpu.abilist') == CONFIG['abi'], 'Unsupported ABI'
    assert shell('getenforce') == 'Enforcing', 'Unexpected SELinux state'
    if native:
        for path, expected in CONFIG['native_hashes'].items():
            assert shell('sha256sum', path).split()[0] == expected, 'Native image mismatch: ' + path


def init_state():
    STATE.mkdir(mode=0o700, exist_ok=True)
    STATE.chmod(0o700)


def private_write(path, data):
    path = Path(path)
    if isinstance(data, str):
        data = data.encode()
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(fd, 'wb') as stream:
        stream.write(data)
        stream.flush()
        os.fsync(stream.fileno())
    path.chmod(0o600)


def save_json(name, value):
    init_state()
    private_write(STATE / name, json.dumps(value, indent=2, ensure_ascii=False) + '\n')


def sha256(path):
    h = hashlib.sha256()
    with open(path, 'rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            h.update(block)
    return h.hexdigest()


def root_json(code):
    result = run([sys.executable, str(ROOT / 'tools/root_bridge.py')], input=code, timeout=45)
    # The apt hook deliberately exits before apt update; its exit 100 is expected.
    # Success is established by the operation's structured proof, never that exit code.
    try:
        return json.JSONDecoder().raw_decode(result.stdout.lstrip())[0]
    except ValueError:
        raise RuntimeError('No valid root-operation proof; inspect the bridge locally') from None


def sdk_tool(name):
    found = shutil.which(name)
    if found:
        return found
    sdk = os.environ.get('ANDROID_SDK_ROOT') or os.environ.get('ANDROID_HOME')
    if sdk:
        path = Path(sdk) / 'build-tools/36.0.0' / name
        if path.is_file():
            return str(path)
    raise RuntimeError('Put ' + name + ' from Android build-tools 36.0.0 on PATH')

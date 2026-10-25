"""Host helpers. Device operations target one selected StepX on the validated firmware."""
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


def adb_bin():
    return os.environ.get('STEPX_ADB', shutil.which('adb') or 'adb')


def is_wireless(serial):
    return ':' in serial or '._adb-tls-connect._tcp' in serial


def env_transport():
    return (os.environ.get('STEPX_SERIAL') or CONFIG.get('serial') or '').strip()


def env_physical_serial():
    return os.environ.get('STEPX_DEVICE_SERIAL', '').strip()


SERIAL = env_transport()
DEVICE_SERIAL = env_physical_serial()
# Mutate this list in place so `from common import ADB` stays bound to the selection.
ADB = [adb_bin(), '-s', SERIAL]


sys.path.insert(0, str(ROOT.parents[1] / 'scripts'))
from host_runtime import sdk_tool, sdk_command, subprocess_options, user_state_root, migrate_legacy_state

STATE = user_state_root() / 'google-play/state'
DOWNLOADS = user_state_root() / 'google-play/downloads'


def run(args, **kwargs):
    return subprocess.run(args, text=True, capture_output=True, check=True,
                          timeout=kwargs.pop('timeout', 40), encoding='utf-8', **subprocess_options(), **kwargs)


def bind_device(transport, physical=''):
    """Update the shared ADB target in place so importers see the same list."""
    global SERIAL, DEVICE_SERIAL
    SERIAL = transport
    if physical:
        DEVICE_SERIAL = physical
    ADB[:] = [adb_bin(), '-s', transport]


def parse_adb_devices(text):
    transports = []
    for line in text.splitlines():
        if not line or line.startswith('List of devices'):
            continue
        fields = line.split()
        if len(fields) < 2 or fields[1] != 'device':
            continue
        transports.append(fields[0])
    return transports


def list_online_transports():
    return parse_adb_devices(run([adb_bin(), 'devices', '-l'], timeout=15).stdout)


def probe_identity(transport):
    command = [adb_bin(), '-s', transport]
    def prop(name):
        return run(command + ['shell', 'getprop', name]).stdout.strip()
    return {
        'transport': transport,
        'serial': prop('ro.serialno'),
        'fingerprint': prop('ro.build.fingerprint'),
        'sdk': prop('ro.build.version.sdk'),
        'abi': prop('ro.product.cpu.abilist'),
    }


def firmware_matches(identity):
    return (identity.get('fingerprint') == CONFIG['fingerprint']
            and identity.get('sdk') == CONFIG['sdk']
            and identity.get('abi') == CONFIG['abi'])


def matching_stepx_devices(transports=None):
    matches = []
    for transport in transports if transports is not None else list_online_transports():
        try:
            identity = probe_identity(transport)
        except (OSError, subprocess.SubprocessError, RuntimeError):
            continue
        if identity.get('serial') and firmware_matches(identity):
            matches.append(identity)
    return matches


def prefer_usb(identities):
    return sorted(identities, key=lambda item: (is_wireless(item['transport']), item['transport']))[0]


def resolve_device():
    """Use STEPX_SERIAL / selected USB, or the only connected matching StepX firmware."""
    transport = SERIAL or env_transport()
    physical = DEVICE_SERIAL or env_physical_serial()
    if transport:
        if transport.startswith('-') or any(c.isspace() for c in transport):
            raise RuntimeError('请先选择已授权的手机连接')
        if is_wireless(transport) and not physical:
            raise RuntimeError('无线连接缺少设备身份，请重新选择手机后重试')
        bind_device(transport, physical)
        return transport
    matches = matching_stepx_devices()
    if not matches:
        raise RuntimeError('未找到已验证固件的 StepX 手机，请连接 USB 或设置 STEPX_SERIAL')
    unique = {}
    for identity in matches:
        unique.setdefault(identity['serial'], []).append(identity)
    if len(unique) > 1:
        raise RuntimeError('检测到多台匹配固件的 StepX 手机（' + ', '.join(sorted(unique))
                           + '），请设置 STEPX_SERIAL 或在桌面选择设备')
    chosen = prefer_usb(next(iter(unique.values())))
    bind_device(chosen['transport'], chosen['serial'])
    return chosen['transport']


def selected_physical_serial():
    return DEVICE_SERIAL or ('' if is_wireless(SERIAL) else SERIAL)


def shell(*args):
    # adb shell joins arguments; quote empty values and metacharacters explicitly.
    return run(ADB + ['shell', shlex.join(str(x) for x in args)]).stdout.strip()


def guard(native=False):
    # ADB transport names (USB, IP:port or mDNS) are not hardware identities.
    # Wireless operations must carry the physical serial selected by the desktop.
    # A missing serial is not a whitelist: auto-select the only matching StepX firmware.
    resolve_device()
    expected = selected_physical_serial()
    if not expected:
        raise RuntimeError('无线连接缺少设备身份，请重新选择手机后重试')
    if run(ADB + ['get-state']).stdout.strip() != 'device':
        raise RuntimeError('手机连接未授权或已离线，请重新连接')
    if shell('getprop', 'ro.serialno') != expected:
        raise RuntimeError('连接的手机与已选设备不一致，请重新选择设备')
    assert shell('getprop', 'ro.build.fingerprint') == CONFIG['fingerprint'], 'Unsupported firmware'
    assert shell('getprop', 'ro.build.version.sdk') == CONFIG['sdk'], 'Unsupported SDK'
    assert shell('getprop', 'ro.product.cpu.abilist') == CONFIG['abi'], 'Unsupported ABI'
    assert shell('getenforce') == 'Enforcing', 'Unexpected SELinux state'
    if native:
        for path, digest in CONFIG['native_hashes'].items():
            assert shell('sha256sum', path).split()[0] == digest, 'Native image mismatch: ' + path
    init_state()


def init_state():
    migrate_legacy_state(ROOT / 'state', STATE)
    STATE.mkdir(parents=True, mode=0o700, exist_ok=True)
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

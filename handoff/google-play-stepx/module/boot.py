"""No daemon or socket. One attempt per system_server lifetime, with a deadline."""
import json, ctypes, time, signal, hashlib, base64, struct, mmap, fcntl

STATE = '/var/lib/stepx-gms'
lockfd = os.open(STATE + '/lock', os.O_RDWR | os.O_CREAT, 0o600)
try:
    fcntl.flock(lockfd, fcntl.LOCK_EX | fcntl.LOCK_NB)
except BlockingIOError:
    os._exit(0)

host = '/proc/self/fd/' + str(rootfd)
boot_id = open(host + '/proc/sys/kernel/random/boot_id').read().strip()
identity = boot_id
try:
    previous = json.load(open(STATE + '/last.json'))
    if previous.get('identity') == identity and (previous.get('status') != 'success' or os.path.exists(host + '/proc/' + str(previous.get('target_pid', 0)))):
        os._exit(0)
except (FileNotFoundError, ValueError):
    pass

def save(result):
    result.update(identity=identity, unix_time=int(time.time()))
    temp = STATE + '/last.new'
    fd = os.open(temp, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(fd, 'w') as stream:
        json.dump(result, stream, indent=2)
        stream.flush()
        os.fsync(stream.fileno())
    os.replace(temp, STATE + '/last.json')

save({'status': 'started'})
# Give Android boot and its package/permission services time to become ready.
time.sleep(12)
payload = open('/usr/local/lib/stepx-gms/inject.py').read()
shared = mmap.mmap(-1, 256 * 1024, flags=mmap.MAP_SHARED | mmap.MAP_ANONYMOUS)
lib = ctypes.CDLL(None, use_errno=True)
json.dumps({'warmup': True}); hashlib.sha256(b'warmup'); base64.b64decode('AA==')
child = os.fork()
if child == 0:
    out = {}
    def expired(signum, frame):
        raise TimeoutError('GMS repair deadline')
    signal.signal(signal.SIGALRM, expired)
    signal.alarm(25)
    try:
        os.fchdir(rootfd); os.chroot('.'); os.chdir('/')
        os.setresgid(1000, 0, 0); os.setresuid(1000, 0, 0)
        lib.setfsgid(1000); lib.setfsuid(1000)
        for context in ('u:r:system_server_startup:s0', 'u:r:system_server:s0'):
            with open('/proc/self/attr/current', 'w') as stream:
                stream.write(context)
        lib.setfsuid(0)
        candidates = []
        for item in os.listdir('/proc'):
            if item.isdigit():
                try:
                    if open('/proc/' + item + '/comm').read().strip() == 'system_server':
                        candidates.append(int(item))
                except OSError:
                    pass
        assert len(candidates) == 1, 'No unique system_server'
        os.environ['STEPX_SYSTEM_SERVER_PID'] = str(candidates[0])
        exec(compile(payload, 'inject.py', 'exec'), globals())
        answer = out.get('java_result', '')
        checks = [line for line in answer.splitlines() if line.startswith('check ')]
        if not answer.startswith('GRANT_RESULT\nSYSTEM_GMS=true\n') or len(checks) < 120 or not all(line.endswith('=0') for line in checks):
            raise RuntimeError('Scoped grant validation failed')
        out['status'] = 'success'
    except BaseException as error:
        out.update(status='failed', error=repr(error))
    finally:
        signal.alarm(0)
        data = json.dumps(out).encode()
        shared[:4] = len(data).to_bytes(4, 'little')
        shared[4:4 + len(data)] = data
        os._exit(0)
deadline = time.monotonic() + 32
while True:
    waited, status = os.waitpid(child, os.WNOHANG)
    if waited:
        break
    if time.monotonic() > deadline:
        os.kill(child, signal.SIGKILL)
        os.waitpid(child, 0)
        save({'status': 'failed', 'error': 'Worker hard deadline'})
        os._exit(1)
    time.sleep(.02)
length = int.from_bytes(shared[:4], 'little')
save(json.loads(shared[4:4 + length]) if length else {'status': 'failed', 'child_status': status})

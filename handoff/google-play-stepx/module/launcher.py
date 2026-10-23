#!/usr/bin/python3
"""Run the factory launcher immediately; isolate one bounded GMS boot repair."""
import os

STOCK = '/usr/local/bin/ubuntu_zygote.stepx-stock-20261021'
try:
    host = '/proc/' + str(os.getppid()) + '/root'
    rootfd = os.open(host, os.O_RDONLY | os.O_DIRECTORY)
    child = os.fork()
except BaseException:
    os.execv(STOCK, ['/usr/local/bin/ubuntu_zygote'])
if child:
    os.close(rootfd)
    os.execv(STOCK, ['/usr/local/bin/ubuntu_zygote'])
try:
    os.setsid()
    fd = os.open(host + '/sys/fs/cgroup/cgroup.procs', os.O_WRONLY)
    os.write(fd, str(os.getpid()).encode())
    os.close(fd)
    # The original root directory stays valid even if openclaw_env restarts.
    for n in (0, 1, 2):
        null = os.open('/dev/null', os.O_RDWR)
        os.dup2(null, n)
        if null > 2:
            os.close(null)
    code = open('/usr/local/lib/stepx-gms/boot.py').read()
    exec(compile(code, '/usr/local/lib/stepx-gms/boot.py', 'exec'), globals())
except BaseException as error:
    try:
        with open('/var/lib/stepx-gms/launcher-error.txt', 'w') as stream:
            stream.write(repr(error))
    except BaseException:
        pass
os._exit(0)

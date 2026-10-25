"""Compatibility fix for the hash-pinned loader; never changes the Google payload."""
import os
import re

IMAGE_BASE = '''def image_base(name):
  # A library may also be mapped as a whole read-only file. That is not its ELF load bias.
  images=[l.split() for l in maps if l.endswith('/'+name) and 'x' in l.split()[1]]
  bases={int(l[0].split('-')[0],16)-int(l[2],16) for l in images}
  if len(bases)!=1:raise RuntimeError('No unique executable native image: '+name)
  return bases.pop()
'''

DOUBLE_FORK_SNIPPET = '''child=os.fork()
if child:
 try:os.waitpid(child,0)
 except ChildProcessError:pass
 os.execv('/usr/local/bin/ubuntu_zygote',['/usr/local/bin/ubuntu_zygote'])
grandchild=os.fork()
if grandchild:os._exit(0)
'''


def reap_zombies():
    """Reap any exited child processes in the current process group without blocking."""
    reaped = 0
    while True:
        try:
            pid, _ = os.waitpid(-1, os.WNOHANG)
            if pid <= 0:
                break
            reaped += 1
        except (ChildProcessError, OSError):
            break
    return reaped


def wrap_detached_subshell(command):
    """Wrap a background shell command in a double-fork subshell so init (PID 1) reaps it."""
    cmd = command.strip()
    return f"( nohup {cmd} >/dev/null 2>&1 & )"


def adapt_double_fork(launcher_source):
    """Replace single-fork before ubuntu_zygote execv with double-fork + waitpid to prevent [stepx-gms-worke] zombies."""
    target = "child=os.fork()\nif child:os.execv('/usr/local/bin/ubuntu_zygote',['/usr/local/bin/ubuntu_zygote'])\n"
    if target in launcher_source:
        return launcher_source.replace(target, DOUBLE_FORK_SNIPPET, 1)
    return launcher_source


def adapt(source):
    source,count=re.subn(r'def image_base\(name\):\n.*?(?=for soname,expected)',IMAGE_BASE,source,flags=re.S)
    if count!=1:raise RuntimeError('Native loader image-base template changed')
    marker="cb=image_base('libc.so');ab=image_base('libart.so')"
    validation='''
for address in (cb+0x79170,cb+0xd77c0,ab+0x669a4c):
 if not any('x' in l.split()[1] and int(l.split()[0].split('-')[0],16)<=address<int(l.split()[0].split('-')[1],16) for l in maps):
  raise RuntimeError('Native entry point is not executable; stopped before attaching')
'''
    if source.count(marker)!=1:raise RuntimeError('Native entry point template changed')
    return source.replace(marker,marker+validation)


"""Portable host dependencies; Android/remote commands are deliberately unchanged."""
from contextlib import contextmanager
import os
from pathlib import Path
import shutil
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]


def subprocess_options():
    return {'creationflags': subprocess.CREATE_NO_WINDOW} if os.name == 'nt' else {}


def java_environment():
    env = os.environ.copy()
    candidates = [Path(env[k]) for k in ('STEPX_JDK', 'JAVA_HOME') if env.get(k)]
    candidates += [ROOT / 'runtime/java', Path.home()/'.local/opt/android-studio/jbr',
                   Path('/opt/android-studio/jbr'), Path('/Applications/Android Studio.app/Contents/jbr/Contents/Home')]
    executable = 'java.exe' if os.name == 'nt' else 'java'
    for directory in candidates:
        if (directory / 'bin' / executable).is_file():
            env['JAVA_HOME'] = str(directory)
            env['PATH'] = str(directory / 'bin') + os.pathsep + env.get('PATH', '')
            break
    return env


def build_tools():
    """Do not guess a developer's home or a single installed SDK version."""
    yield ROOT / 'runtime/android-build-tools'
    for key in ('ANDROID_SDK_ROOT', 'ANDROID_HOME'):
        if os.environ.get(key):
            base = Path(os.environ[key]) / 'build-tools'
            if base.is_dir():
                yield from sorted((p for p in base.iterdir() if p.is_dir()), reverse=True)
    bases = [Path.home()/'Android/Sdk', Path.home()/'Library/Android/sdk']
    if os.environ.get('LOCALAPPDATA'):
        bases.append(Path(os.environ['LOCALAPPDATA'])/'Android/Sdk')
    for sdk in bases:
        base = sdk / 'build-tools'
        if base.is_dir():
            yield from sorted((p for p in base.iterdir() if p.is_dir()), reverse=True)


def sdk_tool(name):
    suffix = '.exe' if os.name == 'nt' else ''
    for directory in build_tools():
        candidate = directory / (name + suffix)
        if candidate.is_file():
            return str(candidate)
    found = shutil.which(name)
    if found:
        return found
    raise RuntimeError('缺少 Android '+name+' 工具。请下载完整安装包并完整解压，勿单独运行 EXE。')


def sdk_command(name):
    # Avoid .bat/cmd quoting, Unicode and space-path failures on Windows.
    if name == 'apksigner':
        for directory in build_tools():
            jar = directory / 'lib/apksigner.jar'
            if jar.is_file():
                env = java_environment()
                java = shutil.which('java', path=env.get('PATH'))
                if not java:
                    raise RuntimeError('缺少签名校验运行时。请重新解压完整安装包。')
                return [java, '-jar', str(jar)]
    return [sdk_tool(name)]


@contextmanager
def file_lock(path, timeout=30):
    """Per-operation lock with Windows byte locking and POSIX flock."""
    path = Path(path)
    if path.is_symlink():
        raise RuntimeError('锁文件不可为符号链接')
    fd = os.open(path, os.O_RDWR | os.O_CREAT | getattr(os, 'O_NOFOLLOW', 0), 0o600)
    with os.fdopen(fd, 'r+b') as stream:
        if os.name == 'nt':
            import msvcrt
            if os.fstat(fd).st_size == 0:
                stream.write(b'\0'); stream.flush()
            deadline = time.monotonic() + timeout
            while True:
                try:
                    stream.seek(0); msvcrt.locking(fd, msvcrt.LK_NBLCK, 1); break
                except OSError:
                    if time.monotonic() >= deadline:
                        raise RuntimeError('另一个安装或修复操作正在运行，请稍后重试')
                    time.sleep(.1)
            try:
                yield
            finally:
                stream.seek(0); msvcrt.locking(fd, msvcrt.LK_UNLCK, 1)
        else:
            import fcntl
            fcntl.flock(fd, fcntl.LOCK_EX)
            try:
                yield
            finally:
                fcntl.flock(fd, fcntl.LOCK_UN)

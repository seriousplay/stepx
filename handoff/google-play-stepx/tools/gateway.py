import subprocess,hmac,hashlib,json,urllib.request,urllib.error,sys
payload=json.load(sys.stdin)
from common import ADB as adb, guard
guard()
android_id=subprocess.check_output(adb+['shell','settings','get','secure','android_id'],text=True).strip()
assert android_id and android_id != 'null'
token=hmac.new(b'stepclaw-gateway-token-salt-v1',('stepclaw-gateway-v1|'+android_id).encode(),hashlib.sha256).hexdigest()
port=subprocess.check_output(adb+['forward','tcp:0','tcp:18789'],text=True).strip()
try:
 route=payload.pop('_route','/plugins/stepclaw-exec/run')
 assert route.startswith('/')
 req=urllib.request.Request('http://127.0.0.1:'+port+route,data=json.dumps(payload).encode(),headers={'Authorization':'Bearer '+token,'Content-Type':'application/json'})
 try:
  with urllib.request.urlopen(req,timeout=40) as r:
   data=r.read(2000000).decode();print(data.replace(token,'[REDACTED]'))
 except urllib.error.HTTPError as e: print('HTTP',e.code,e.read(3000).decode().replace(token,'[REDACTED]'))
finally: subprocess.run(adb+['forward','--remove','tcp:'+port],check=True)

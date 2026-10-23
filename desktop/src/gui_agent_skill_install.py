import json, pathlib, sys, urllib.request
workspace=pathlib.Path(sys.argv[1]);token=sys.argv[2]
if not str(workspace.resolve()).startswith('/root/.openclaw/') or workspace.is_symlink():
    raise ValueError('Invalid workspace')
root=workspace/'skills'/'stepx-gui-agent'
if root.is_symlink():raise ValueError('Invalid skill path')
root.mkdir(parents=True,exist_ok=True)
client='''import json,pathlib,sys,urllib.request
config=json.loads((pathlib.Path(__file__).parent/'endpoint.json').read_text())
request=json.loads(sys.argv[1])
r=urllib.request.Request(config['url'],json.dumps(request).encode(),{'Authorization':'Bearer '+config['token'],'Content-Type':'application/json'},method='POST')
with urllib.request.urlopen(r,timeout=10) as response:print(response.read().decode())
'''
(root/'client.py').write_text(client)
endpoint=root/'endpoint.json';endpoint.write_text(json.dumps({'url':'http://127.0.0.1:18796/tool','token':token}));endpoint.chmod(0o600)
(root/'SKILL.md').write_text('''---
name: stepx-gui-agent
description: Use StepX伴侣 to visually operate the user's connected phone, including Chinese input, clicks, swipes and multi-step navigation. Requires desktop connected and phone unlocked.
---
Use this skill for user-requested phone UI operations. Do not call the unavailable factory gui_agent.
Run `python3 {baseDir}/client.py '<JSON>'` using exec.
Start: {"op":"start","app":"应用名称或包名","goal":"用户完整目标与约束"}.
The tool returns a task ID. Poll {"op":"status"} at intervals of 5 seconds until active is false. Report the actual result, never claim task completion from submission alone.
Pause, resume or stop: {"op":"pause|resume|stop","task":"returned task ID"}.
Use only the user's authorized goal. If the phone is locked, ask the user to unlock; never request passwords. Do not start nested GUI tasks inside GUI observation/planning prompts. Endpoint tokens are private and must never be included in replies.
''')
r=urllib.request.Request('http://127.0.0.1:18796/tool',b'{"op":"status"}',{'Authorization':'Bearer '+token,'Content-Type':'application/json'},method='POST')
with urllib.request.urlopen(r,timeout=8) as response:
    assert json.load(response)['ok']
print('GUI skill installed and endpoint verified')

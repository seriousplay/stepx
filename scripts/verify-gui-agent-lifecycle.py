import os, shutil
#!/usr/bin/env python3
import subprocess,json,time,pathlib,base64
ROOT=pathlib.Path(__file__).resolve().parents[1]; binary=ROOT/'desktop/target/debug/examples/verify_gui_remote'; report=ROOT/'diagnostics/gui-agent-20261023/lifecycle.json'
def remote(q):
    result=json.loads(subprocess.check_output([str(binary),json.dumps(q,ensure_ascii=False)],text=True));assert result['ok'],result;return result
def phone(q):
    encoded=base64.b64encode(json.dumps(q).encode()).decode()
    raw=subprocess.check_output([os.environ.get('STEPX_ADB', shutil.which('adb') or 'adb'),'-s',os.environ['STEPX_SERIAL'],'shell','content','call','--user','0','--uri','content://com.stepx.companion.app.gui','--method','request','--arg',encoded],text=True)
    return json.loads(base64.b64decode(raw.split('result=')[1].split('}')[0].strip()))
assert not remote({'op':'status'})['active']
r=remote({'op':'start','app':'com.android.settings','goal':'在设置中通过搜索输入中文“关于手机”，打开关于手机页面，找到 Android 版本并读出版本号。只浏览，不修改设置。'})
task=r['task'];result={}
try:
    time.sleep(3);remote({'op':'pause','task':task})
    for _ in range(20):
        s=phone({'op':'status'})
        if s.get('phase')=='paused':break
        time.sleep(.3)
    assert s['phase']=='paused',s
    before=s['actions'];time.sleep(3);assert phone({'op':'status'})['actions']==before
    result['pause_during_model_stable']=True
    remote({'op':'resume','task':task})
    for _ in range(100):
        s=remote({'op':'status'})
        if not s['active']:break
        time.sleep(2)
    assert not s['active'] and s['result']['state']=='Completed',s
    result['resumed_task']=s['result']
    r=remote({'op':'start','app':'com.android.settings','goal':'在设置中搜索“电池”，进入电池页面查看电量。只浏览，不修改设置。'})
    task=r['task'];time.sleep(3);remote({'op':'stop','task':task})
    for _ in range(20):
        s=phone({'op':'status'})
        if s.get('phase')=='stopped':break
        time.sleep(.3)
    assert s['phase']=='stopped',s
    before=s['actions'];time.sleep(3);assert phone({'op':'status'})['actions']==before
    result['stop_during_model_stable']=True
finally:
    try:
        if remote({'op':'status'})['active']:remote({'op':'stop','task':task})
    finally:report.write_text(json.dumps(result,ensure_ascii=False,indent=2))
print('GUI Agent lifecycle passed')

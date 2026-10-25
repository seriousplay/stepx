#!/usr/bin/env python3
"""Real-device lifecycle verification against the companion executor (no model)."""
import base64,json,subprocess,time,pathlib
ADB=os.environ.get('STEPX_ADB', shutil.which('adb') or 'adb');SERIAL=os.environ['STEPX_SERIAL']
def call(q,allow_error=False):
 encoded=base64.b64encode(json.dumps(q,ensure_ascii=False).encode()).decode()
 out=subprocess.check_output([ADB,'-s',SERIAL,'shell','content','call','--user','0','--uri','content://com.stepx.companion.app.gui','--method','request','--arg',encoded],text=True)
 r=json.loads(base64.b64decode(out.split('result=')[1].split('}')[0].strip()))
 if not allow_error and not r.get('ok'):raise RuntimeError(r)
 return r
s=call({'op':'status'});assert s['phase'] not in ('running','paused'), 'Another task is active'
task='7c623a6a0df74ae08a6cb948d035cfc8'
try:
 call({'op':'start','task':task,'goal':'验证暂停与过期动作拒绝，不修改手机设置'})
 screen=call({'op':'observe','task':task})
 paused=call({'op':'pause','task':task})
 action={'op':'action','task':task,'epoch':screen['epoch'],'treeHash':screen['treeHash'],'action':{'type':'back'}}
 rejected_pause=call(action,True);assert not rejected_pause['ok']
 time.sleep(2);assert call({'op':'status'})['actions']==0
 call({'op':'resume','task':task});rejected_stale=call(action,True);assert not rejected_stale['ok']
 fresh=call({'op':'observe','task':task});action.update(epoch=fresh['epoch'],treeHash=fresh['treeHash'])
 executed=call(action);assert executed['actions']==1
 call({'op':'stop','task':task});assert not call(action,True)['ok']
 time.sleep(2);assert call({'op':'status'})['actions']==1
 print(json.dumps({'pause_rejects_actions':True,'pause_stable':True,'resume_rejects_old_observation':True,'fresh_navigation_executed':True,'stop_rejects_actions':True}))
finally:call({'op':'stop','task':task},True)

"""Apply narrowly scoped original instrumentation to the pinned private source."""
from pathlib import Path


def replace_exact(path: Path, old: str, new: str, count=1):
    text = path.read_text('utf-8')
    if text.count(old) != count:
        raise RuntimeError(f'Upstream instrumentation anchor changed: {path.name}')
    path.write_text(text.replace(old, new), 'utf-8')


def instrument(stage: Path):
    src = stage / 'src/studio/jiayu/neoagent'
    # Preserve the native overlay lifecycle/ownership checks, but avoid a blue,
    # continuously animated full-screen halo. Execution state lives in the bubble.
    effects = src / 'ControlEffects.java'
    old_draw = next(line for line in effects.read_text('utf-8').splitlines() if line.startswith(' @Override protected void onDraw(Canvas c)'))
    replace_exact(effects, old_draw, ' @Override protected void onDraw(Canvas c){super.onDraw(c);}')
    replace_exact(src / 'ControlService.java',
                  'final ControlBar controlBar=new ControlBar(this);',
                  'final CompanionControlBar controlBar=new CompanionControlBar(this);')
    replace_exact(src / 'ControlService.java',
                  'overlay=controlBar.view;overlayText=controlBar.detail;',
                  'overlay=controlBar.view;overlayText=controlBar.detail;CompanionHostSurface.sync(this);')
    replace_exact(src / 'SearchTarget.java',
                  'static String fromUser(String user){',
                  'static String fromUser(String user){user=CompanionSearchTarget.normalizeEditorInstruction(user);')
    for name, owner, count in (('AgentImeInputBridge.java', 'owner', 1), ('SearchInputBridge.java', 'owner', 2), ('ControlService.java', 'this', 2)):
        prefix = '' if owner == 'this' else 'owner.'
        old = f'SearchQueryPolicy.allowed({prefix}task,{prefix}activeRequest==null?"":{prefix}activeRequest.original,{prefix}activeRequest==null?"":{prefix}activeRequest.target,{prefix}allowed,text,new AppAccess({owner}).independentSearchFields({prefix}allowed))'
        new = old.replace('SearchQueryPolicy.allowed(', f'CompanionInputDiagnostics.searchAllowed({owner},', 1)
        replace_exact(src / name, old, new, count)
    replace_exact(src / 'ControlService.java',
                  'check(g);boolean updated=workingMemory.propose(',
                  'check(g);CompanionInputDiagnostics.candidate(this,action);boolean updated=workingMemory.propose(')
    replace_exact(src / 'ControlService.java',
                  'ui(()->{validateObservation(verified,g);return true;});',
                  'ui(()->{validateObservation(verified,g);return CompanionCompletion.capture(this,verified,g);});')
    replace_exact(src / 'ModelClient.java',
                  'int outputTokens=Math.max(512,Math.min(8192,maxOutput));',
                  'final long bridgeRequestStartedAt=System.currentTimeMillis();int outputTokens=CompanionModelPolicy.outputTokens(c.endpoint,c.model,c.protocol,maxOutput);')
    replace_exact(src / 'ModelClient.java',
                  'if(cancelled)throw new Exception("请求已停止");HttpURLConnection h=',
                  'CompanionModelDiagnostics.configure(c,body,system,image!=null);if(cancelled)throw new Exception("请求已停止");HttpURLConnection h=')
    replace_exact(src / 'ModelClient.java',
                  'JSONObject o=new JSONObject(out.toString("UTF-8"));',
                  'JSONObject o=new JSONObject(out.toString("UTF-8"));CompanionModelDiagnostics.inspect(this,c,o,outputTokens,bridgeRequestStartedAt);')

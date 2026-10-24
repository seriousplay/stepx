package com.stepx.companion.app;

import android.app.*;
import android.content.*;
import android.graphics.Color;
import android.os.*;
import android.provider.Settings;
import android.text.*;
import android.view.*;
import android.widget.*;
import org.json.*;
import java.util.*;
import java.util.concurrent.*;

/** User-owned configuration. External Intent extras cannot configure a model or app scope. */
public final class NeoSettingsActivity extends Activity {
    private final Handler main=new Handler(Looper.getMainLooper());
    private final ExecutorService worker=Executors.newSingleThreadExecutor();
    private CompanionUi ui;
    private EditText endpoint,model,key;
    private Spinner protocol;
    private SeekBar steps;
    private TextView status,scope,stepLabel;
    private Button save,choose;
    private Switch allApps;private LinearLayout advanced;
    private JSONArray applications=new JSONArray();
    private final LinkedHashSet<String> selected=new LinkedHashSet<>();
    private boolean loaded,busy,destroyed,resumed,polling; private String baseline="";private Bundle restored;
    private EditText field(LinearLayout p,String label,String hint){
        ui.text(p,label,13,CompanionUi.MUTED,12);EditText e=new EditText(this);e.setTextSize(15);e.setSingleLine(true);e.setTextColor(CompanionUi.INK);e.setHintTextColor(CompanionUi.MUTED);e.setHint(hint);e.setPadding(ui.dp(12),ui.dp(12),ui.dp(12),ui.dp(12));e.setBackground(ui.shape(CompanionUi.SURFACE,CompanionUi.LINE,8));LinearLayout.LayoutParams lp=new LinearLayout.LayoutParams(-1,-2);lp.topMargin=ui.dp(6);e.setMinHeight(ui.dp(48));p.addView(e,lp);return e;
    }
    @Override public void onCreate(Bundle state){
        super.onCreate(state);restored=state;ui=new CompanionUi(this);getWindow().addFlags(WindowManager.LayoutParams.FLAG_SECURE);
        LinearLayout shell=ui.column();shell.setBackgroundColor(CompanionUi.BG);LinearLayout header=ui.column();header.setPadding(ui.dp(20),ui.dp(8),ui.dp(20),ui.dp(12));shell.addView(header);ui.back(header,"模型设置",this::leave);
        ScrollView scroll=new ScrollView(this);LinearLayout root=ui.column();root.setPadding(ui.dp(20),0,ui.dp(20),ui.dp(24));scroll.addView(root);shell.addView(scroll,new LinearLayout.LayoutParams(-1,0,1));setContentView(shell);ui.bars(shell);
        
        LinearLayout access=ui.card(root,18);ui.title(access,"执行器状态",17,0);status=ui.text(access,"正在读取配置…",14,CompanionUi.MUTED,8);ui.row(access,UiGlyph.SCREEN,"看屏操作权限","开启 Neo 看屏服务后返回此页",()->startActivity(new Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS)));
        LinearLayout cfg=ui.card(root,16);ui.title(cfg,"模型连接",17,0);ui.text(cfg,"使用支持图片输入的模型服务，模型费用由该服务账号承担。",13,CompanionUi.MUTED,6);
        endpoint=field(cfg,"API 地址","https://…/v1");endpoint.setInputType(InputType.TYPE_CLASS_TEXT|InputType.TYPE_TEXT_VARIATION_URI);model=field(cfg,"模型名称","填写服务商提供的完整模型名");ui.text(cfg,"接口协议",13,CompanionUi.MUTED,12);protocol=new Spinner(this);protocol.setAdapter(new ArrayAdapter<String>(this,android.R.layout.simple_spinner_dropdown_item,new String[]{"OpenAI","Anthropic","Gemini"}));cfg.addView(protocol,new LinearLayout.LayoutParams(-1,ui.dp(48)));
        key=field(cfg,"API 密钥","留空保留当前密钥");key.setInputType(InputType.TYPE_CLASS_TEXT|InputType.TYPE_TEXT_VARIATION_PASSWORD);key.setImportantForAutofill(View.IMPORTANT_FOR_AUTOFILL_NO);key.setSaveEnabled(false);
        ui.text(cfg,"密钥加密保存在手机。更换地址或协议时须重新填写，不会传入 Amoo 对话。",12,CompanionUi.MUTED,10);
        LinearLayout apps=ui.card(root,16);ui.title(apps,"执行偏好",17,0);allApps=new Switch(this);allApps.setText("允许全部应用");allApps.setTextSize(15);allApps.setTextColor(CompanionUi.INK);allApps.setMinHeight(ui.dp(48));allApps.setChecked(true);apps.addView(allApps,new LinearLayout.LayoutParams(-1,-2));scope=ui.text(apps,"正在读取…",14,CompanionUi.MUTED,8);ui.text(apps,"自动包含新安装的可启动应用，仅在你发起任务时执行。",12,CompanionUi.MUTED,8);
        Button more=ui.button(apps,"高级设置",false,null);advanced=ui.column();apps.addView(advanced);advanced.setVisibility(View.GONE);more.setOnClickListener(v->{boolean open=advanced.getVisibility()!=View.VISIBLE;advanced.setVisibility(open?View.VISIBLE:View.GONE);more.setText(open?"收起高级设置":"高级设置");});choose=ui.button(advanced,"自定义应用范围",false,this::chooseApps);
        allApps.setOnCheckedChangeListener((v,on)->{if(on){selectAll();}else advanced.setVisibility(View.VISIBLE);updateScope();});
        LinearLayout limits=ui.card(advanced,16);ui.title(limits,"单任务执行上限",17,0);stepLabel=ui.text(limits,"24 步",14,CompanionUi.INK,8);steps=new SeekBar(this);steps.setMax(36);steps.setProgress(20);limits.addView(steps,new LinearLayout.LayoutParams(-1,ui.dp(48)));steps.setOnSeekBarChangeListener(new SeekBar.OnSeekBarChangeListener(){public void onStartTrackingTouch(SeekBar s){}public void onStopTrackingTouch(SeekBar s){}public void onProgressChanged(SeekBar s,int n,boolean user){stepLabel.setText((n+4)+" 步");}});ui.text(limits,"4–40 步。限制失误与长时间重复操作；到达边界后仍需你接管。",12,CompanionUi.MUTED,6);
        ui.text(root,"仅在你发起任务后发送所选应用的截图。密码、支付和系统安全确认仍需本人处理。",12,CompanionUi.MUTED,18);
        save=ui.button(root,"保存并启用",true,this::confirmSave);setBusy(true);worker.execute(this::load);
    }
    private void setBusy(boolean value){busy=value;for(View v:new View[]{endpoint,model,key,protocol,steps,choose,allApps,save})if(v!=null){v.setEnabled(!value&&loaded);v.setAlpha(!value&&loaded?1f:0.45f);}}
    private String signature(){return allApps.isChecked()+"|"+endpoint.getText()+"|"+model.getText()+"|"+protocol.getSelectedItemPosition()+"|"+steps.getProgress()+"|"+selected.toString();}
    private void leave(){if(busy){Toast.makeText(this,"正在处理，请稍候",Toast.LENGTH_SHORT).show();return;}if(loaded&&(!baseline.equals(signature())||key.length()>0))new AlertDialog.Builder(this).setTitle("放弃尚未保存的修改？").setNegativeButton("继续编辑",null).setPositiveButton("放弃修改",(d,w)->finish()).show();else finish();}
    @Override public void onBackPressed(){leave();}
    private void load(){try{
        JSONObject cfg=NeoAgent.call(this,new JSONObject().put("op","config_info")),list=NeoAgent.call(this,new JSONObject().put("op","apps")),info=NeoAgent.status(this);
        main.post(()->{if(destroyed)return;endpoint.setText(cfg.optString("endpoint"));model.setText(cfg.optString("model"));String p=cfg.optString("protocol");protocol.setSelection(p.equalsIgnoreCase("Anthropic")?1:p.equalsIgnoreCase("Gemini")?2:0);key.setHint(cfg.optBoolean("hasKey")?"已加密保存，留空保持":"请输入 API Key");steps.setProgress(Math.max(0,Math.min(36,cfg.optInt("maxSteps",24)-4)));applications=list.optJSONArray("apps");if(applications==null)applications=new JSONArray();selected.clear();for(int i=0;i<applications.length();i++){JSONObject a=applications.optJSONObject(i);if(a!=null&&a.optBoolean("enabled"))selected.add(a.optString("package"));}allApps.setChecked(!"custom".equals(cfg.optString("scopeMode")));if(allApps.isChecked())selectAll();baseline=signature();
            if(restored!=null){endpoint.setText(restored.getString("endpoint",endpoint.getText().toString()));model.setText(restored.getString("model",model.getText().toString()));protocol.setSelection(restored.getInt("protocol",protocol.getSelectedItemPosition()));steps.setProgress(restored.getInt("steps",steps.getProgress()));allApps.setChecked(restored.getBoolean("allApps",allApps.isChecked()));ArrayList<String> scopes=restored.getStringArrayList("scope");if(scopes!=null){selected.clear();selected.addAll(scopes);}restored=null;}
            updateScope();loaded=true;setBusy(false);showState(info);
        });
    }catch(Exception e){main.post(()->{if(!destroyed){status.setText("配置读取失败："+e.getMessage());busy=false;save.setText("重新读取");save.setEnabled(true);save.setAlpha(1);save.setOnClickListener(v->{save.setText("保存并启用");save.setOnClickListener(x->confirmSave());setBusy(true);worker.execute(this::load);});}});}}
    private void showState(JSONObject info){boolean active=NeoPolicy.active(info.optString("phase"))||info.optBoolean("executorBusy");status.setText(active?"任务正在运行或暂停中，请先结束任务再修改配置。":info.optBoolean("ready")?"本地配置已就绪 · 模型连通性以实际任务为准":!info.optBoolean("executorConnected")?"尚未开启看屏服务":!info.optBoolean("modelConfigured")?"请填写模型与密钥":"请选择应用并确认截图接收端");if(active)for(View v:new View[]{endpoint,model,key,protocol,steps,choose,allApps,save})v.setEnabled(false);}
    private final Runnable refreshState=new Runnable(){public void run(){if(!resumed||destroyed)return;if(loaded&&!busy&&!polling){polling=true;worker.execute(()->{try{JSONObject info=NeoAgent.status(NeoSettingsActivity.this);main.post(()->{polling=false;if(!destroyed&&!busy){setBusy(false);showState(info);}});}catch(Exception ignored){main.post(()->polling=false);}});}main.postDelayed(this,3000);}};
    @Override protected void onResume(){super.onResume();resumed=true;main.post(refreshState);}
    @Override protected void onPause(){resumed=false;main.removeCallbacks(refreshState);super.onPause();}
    private void selectAll(){selected.clear();for(int i=0;i<applications.length();i++){JSONObject a=applications.optJSONObject(i);if(a!=null)selected.add(a.optString("package"));}}
    private void updateScope(){if(allApps.isChecked()){scope.setText("全部 "+applications.length()+" 个可操作应用 · 自动维护");choose.setVisibility(View.GONE);return;}choose.setVisibility(View.VISIBLE);ArrayList<String> names=new ArrayList<>();for(int i=0;i<applications.length();i++){JSONObject a=applications.optJSONObject(i);if(a!=null&&selected.contains(a.optString("package"))&&names.size()<6)names.add(a.optString("name"));}scope.setText(selected.isEmpty()?"尚未授权应用":"已选择 "+selected.size()+" 个应用\n"+String.join("、",names)+(selected.size()>6?" 等":""));}
    private void chooseApps(){
        LinkedHashSet<String> draft=new LinkedHashSet<>(selected);LinearLayout panel=ui.column();panel.setPadding(ui.dp(20),0,ui.dp(20),0);EditText search=field(panel,"搜索应用","输入名称或包名");ListView list=new ListView(this);list.setChoiceMode(ListView.CHOICE_MODE_MULTIPLE);panel.addView(list,new LinearLayout.LayoutParams(-1,ui.dp(300)));ArrayList<JSONObject> visible=new ArrayList<>();
        Runnable filter=()->{String q=search.getText().toString().toLowerCase(Locale.ROOT).trim();visible.clear();ArrayList<String> names=new ArrayList<>();for(int i=0;i<applications.length();i++){JSONObject a=applications.optJSONObject(i);if(a!=null&&(a.optString("name")+" "+a.optString("package")).toLowerCase(Locale.ROOT).contains(q)){visible.add(a);names.add(a.optString("name")+"\n"+a.optString("package"));}}list.setAdapter(new ArrayAdapter<String>(this,android.R.layout.simple_list_item_multiple_choice,names));list.clearChoices();for(int i=0;i<visible.size();i++)list.setItemChecked(i,draft.contains(visible.get(i).optString("package")));};
        list.setOnItemClickListener((p,v,i,id)->{String pkg=visible.get(i).optString("package");if(list.isItemChecked(i))draft.add(pkg);else draft.remove(pkg);});search.addTextChangedListener(new TextWatcher(){public void beforeTextChanged(CharSequence s,int a,int c,int f){}public void onTextChanged(CharSequence s,int a,int b,int c){filter.run();}public void afterTextChanged(Editable e){}});filter.run();
        new AlertDialog.Builder(this).setTitle("允许看屏操作的应用").setView(panel).setNegativeButton("取消",null).setPositiveButton("确定",(d,w)->{selected.clear();selected.addAll(draft);updateScope();}).show();
    }
    private void confirmSave(){if(busy||!loaded)return;if(endpoint.getText().toString().trim().isEmpty()){endpoint.setError("请填写 API 地址");endpoint.requestFocus();return;}if(model.getText().toString().trim().isEmpty()){model.setError("请填写模型名称");model.requestFocus();return;}if(!allApps.isChecked()&&selected.isEmpty()){status.setText("请至少选择一个允许操作的应用");return;}
        new AlertDialog.Builder(this).setTitle("确认模型与应用范围").setMessage("截图接收端：\n"+endpoint.getText().toString().trim()+"\n\n"+(allApps.isChecked()?"全部可操作应用（含后续新安装应用）":"自选 "+selected.size()+" 个应用")+"，单任务上限 "+(steps.getProgress()+4)+" 步。").setNegativeButton("取消",null).setPositiveButton("保存",(d,w)->save()).show();
    }
    private void save(){final JSONObject cfg=new JSONObject(),apps=new JSONObject();try{cfg.put("op","config").put("endpoint",endpoint.getText().toString().trim()).put("model",model.getText().toString().trim()).put("protocol",protocol.getSelectedItem().toString()).put("userConfirmed",true).put("maxSteps",steps.getProgress()+4);String value=key.getText().toString().trim();if(!value.isEmpty())cfg.put("key",value);apps.put("op","apps").put("mode",allApps.isChecked()?"all":"custom").put("packages",new JSONArray(selected)).put("userConfirmed",true);}catch(Exception e){status.setText("配置格式无效");return;}setBusy(true);worker.execute(()->{try{
        NeoAgent.call(this,cfg);NeoAgent.call(this,apps);JSONObject info=NeoAgent.enable(this);main.post(()->{if(destroyed)return;key.setText("");baseline=signature();setBusy(false);showState(info);Toast.makeText(this,"配置已保存",Toast.LENGTH_SHORT).show();});
    }catch(Exception e){main.post(()->{if(!destroyed){setBusy(false);status.setText("保存未完成，请检查配置后重试："+e.getMessage());}});}});}
    @Override protected void onSaveInstanceState(Bundle out){super.onSaveInstanceState(out);if(loaded){out.putString("endpoint",endpoint.getText().toString());out.putString("model",model.getText().toString());out.putInt("protocol",protocol.getSelectedItemPosition());out.putInt("steps",steps.getProgress());out.putStringArrayList("scope",new ArrayList<>(selected));out.putBoolean("allApps",allApps.isChecked());}}
    @Override protected void onDestroy(){destroyed=true;main.removeCallbacksAndMessages(null);worker.shutdown();super.onDestroy();}
}

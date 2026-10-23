package com.stepx.companion.app;

import android.Manifest;
import android.app.*;
import android.content.*;
import android.content.pm.PackageManager;
import android.graphics.*;
import android.graphics.drawable.*;
import android.net.Uri;
import android.os.*;
import android.provider.Settings;
import android.view.*;
import android.widget.*;
import org.json.JSONObject;
import java.io.*;
import java.util.*;
import java.util.concurrent.*;

/** Phone companion: an explicit connection, scoped file sharing, and user-owned background controls. */
public final class MainActivity extends Activity {
    private static final int PICK_FILES=30,SAVE_FILE=31;
    private final Handler ui=new Handler(Looper.getMainLooper());
    private final ExecutorService files=Executors.newSingleThreadExecutor();
    private final int ink=Color.rgb(25,25,29),secondary=Color.rgb(113,113,124),line=Color.rgb(229,229,235),green=Color.rgb(14,169,124);
    private FrameLayout content; private LinearLayout navigation,transferList;
    private TextView linkTitle,linkHint,connectionFoot,transferHint,amooState,gameState,permissionState,notice;
    private TextView guiState;
    private Button send; private Switch amooSwitch;
    private TransferStore store; private TransferStore.Link selectedComputer; private TransferStore.Entry saving;
    private int tab=3,transferDirection; private boolean resumed,busy,updatingSwitch,listing,destroyed;
    private String lastListing="",enqueueError="";
    private int dp(float n){return Math.round(n*getResources().getDisplayMetrics().density);}
    private String version(){try{return getPackageManager().getPackageInfo(getPackageName(),0).versionName;}catch(Exception ignored){return "";}}
    private GradientDrawable surface(int fill,int stroke,int radius){GradientDrawable d=new GradientDrawable();d.setColor(fill);d.setCornerRadius(dp(radius));if(stroke!=0)d.setStroke(dp(1),stroke);return d;}
    private LinearLayout column(){LinearLayout v=new LinearLayout(this);v.setOrientation(LinearLayout.VERTICAL);return v;}
    private TextView text(LinearLayout p,String value,int size,int color,int top){TextView t=new TextView(this);t.setText(value);t.setTextSize(size);t.setTextColor(color);t.setLineSpacing(dp(3),1);LinearLayout.LayoutParams lp=new LinearLayout.LayoutParams(-1,-2);lp.topMargin=dp(top);p.addView(t,lp);return t;}
    private TextView heading(LinearLayout p,String value,int top){TextView t=text(p,value,20,ink,top);t.setTypeface(null,Typeface.BOLD);return t;}
    private LinearLayout card(LinearLayout p,int top){LinearLayout v=column();v.setPadding(dp(18),dp(18),dp(18),dp(18));v.setBackground(surface(Color.WHITE,line,18));LinearLayout.LayoutParams lp=new LinearLayout.LayoutParams(-1,-2);lp.topMargin=dp(top);p.addView(v,lp);return v;}
    private Button button(LinearLayout p,String title,boolean primary,Runnable action){Button b=new Button(this);b.setText(title);b.setAllCaps(false);b.setTextSize(15);b.setTextColor(primary?Color.WHITE:ink);b.setBackgroundTintList(null);b.setBackground(surface(primary?ink:Color.WHITE,primary?0:line,12));b.setPadding(dp(12),dp(8),dp(12),dp(8));b.setMinHeight(dp(48));LinearLayout.LayoutParams lp=new LinearLayout.LayoutParams(-1,-2);lp.topMargin=dp(12);p.addView(b,lp);b.setOnClickListener(v->action.run());return b;}
    private LinearLayout page(String title,String subtitle){ScrollView scroll=new ScrollView(this);scroll.setFillViewport(true);scroll.setClipToPadding(false);LinearLayout page=column();page.setPadding(dp(20),dp(24),dp(20),dp(24));heading(page,title,0).setTextSize(26);text(page,subtitle,14,secondary,8);scroll.addView(page);content.addView(scroll,new FrameLayout.LayoutParams(-1,-1));return page;}
    @Override public void onCreate(Bundle saved){super.onCreate(saved);store=new TransferStore(this);if(saved!=null){tab=saved.getInt("tab",3);transferDirection=saved.getInt("direction",0);try{String id=saved.getString("savingId");if(id!=null)saving=store.get(saved.getString("savingDirection"),id);String target=saved.getString("targetId");if(target!=null)selectedComputer=new TransferStore.Link(target,saved.getString("targetName","电脑"),true);}catch(Exception ignored){}}
        LinearLayout root=column();root.setBackgroundColor(Color.rgb(248,248,251));root.setOnApplyWindowInsetsListener((v,insets)->{Insets b=insets.getInsets(WindowInsets.Type.systemBars());v.setPadding(b.left,b.top,b.right,b.bottom);return insets;});
        LinearLayout header=new LinearLayout(this);header.setGravity(Gravity.CENTER_VERTICAL);header.setPadding(dp(20),dp(14),dp(20),dp(12));header.setBackgroundColor(Color.WHITE);ImageView icon=new ImageView(this);icon.setImageResource(getApplicationInfo().icon);header.addView(icon,new LinearLayout.LayoutParams(dp(36),dp(36)));LinearLayout titles=column();LinearLayout.LayoutParams titleParams=new LinearLayout.LayoutParams(-1,-2);titleParams.leftMargin=dp(12);header.addView(titles,titleParams);TextView name=text(titles,"StepX伴侣",19,ink,0);name.setTypeface(null,Typeface.BOLD);text(titles,"让手机与电脑一起工作",12,secondary,1);root.addView(header);
        content=new FrameLayout(this);root.addView(content,new LinearLayout.LayoutParams(-1,0,1));navigation=new LinearLayout(this);navigation.setPadding(dp(12),dp(8),dp(12),dp(8));navigation.setBackgroundColor(Color.WHITE);root.addView(navigation,new LinearLayout.LayoutParams(-1,dp(74)));setContentView(root);root.requestApplyInsets();
        getWindow().getInsetsController().setSystemBarsAppearance(WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS|WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS,WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS|WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS);
        showTab(tab);
    }
    private void showTab(int next){tab=next;content.removeAllViews();navigation.removeAllViews();linkTitle=linkHint=connectionFoot=transferHint=amooState=gameState=permissionState=notice=null;guiState=null;send=null;amooSwitch=null;transferList=null;lastListing="";
        String[] labels={"连接","文件","设置","任务"};for(int i:new int[]{3,0,1,2}){final int index=i;TextView item=new TextView(this);item.setText(labels[i]);item.setTextSize(12);item.setGravity(Gravity.CENTER);item.setTextColor(tab==i?ink:secondary);item.setCompoundDrawablePadding(dp(4));NavIcon glyph=new NavIcon(i,tab==i?ink:secondary);glyph.setBounds(0,0,dp(22),dp(22));item.setCompoundDrawables(null,glyph,null,null);item.setBackground(surface(tab==i?Color.rgb(240,240,245):Color.WHITE,0,12));LinearLayout.LayoutParams lp=new LinearLayout.LayoutParams(0,-1,1);lp.leftMargin=dp(4);lp.rightMargin=dp(4);navigation.addView(item,lp);item.setOnClickListener(v->showTab(index));}
        if(tab==3)taskPage();else if(tab==0)connectionPage();else if(tab==1)filesPage();else settingsPage();refresh();
    }
    private void taskPage(){
        LinearLayout p=page("手机任务","直接在手机执行，离开 desktop 也能继续。");
        LinearLayout gui=card(p,24);heading(gui,"GUI Agent",0);
        text(gui,"看屏、点击、滑动与中文输入，由手机上的独立执行器完成。",14,secondary,8);
        guiState=text(gui,"正在读取任务状态",14,secondary,16);
        button(gui,"新建手机任务",true,()->guiTaskDialog());
        LinearLayout controls=new LinearLayout(this);gui.addView(controls,new LinearLayout.LayoutParams(-1,-2));
        String[] titles={"暂停","继续","停止"},ops={"pause","resume","stop"};
        for(int i=0;i<3;i++){final String op=ops[i];LinearLayout cell=column();LinearLayout.LayoutParams cp=new LinearLayout.LayoutParams(0,-2,1);if(i>0)cp.leftMargin=dp(8);controls.addView(cell,cp);button(cell,titles[i],false,()->{guiControl(op);if(op.equals("resume"))finish();});}
        text(gui,"暂停后点击继续；停止后可直接新建任务。执行期间也可从通知栏控制。",12,secondary,12);
        LinearLayout setup=card(p,18);heading(setup,"模型与看屏",0);
        text(setup,"Step Plan · Step 3.7 Flash\n运行时需要手机联网并保持解锁。截图发给模型分析；密钥加密保存在本机。",14,secondary,8);
        button(setup,"配置模型",false,()->guiKeyDialog());
        button(setup,"看屏服务设置",false,()->startActivity(new Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS)));
        LinearLayout amoo=card(p,18);heading(amoo,"从 Amoo 委派",0);
        text(amoo,"安装“StepX伴侣看屏操作”技能后，可在 Amoo 对话中发送手机操作目标。电脑仅作为可选的控制入口。",14,secondary,8);
    }
    private void connectionPage(){LinearLayout p=page("连接你的电脑","文件互传、Amoo 后台协作，都从一次安全连接开始。");LinearLayout c=card(p,24);text(c,"当前连接",12,secondary,0);linkTitle=heading(c,"正在读取连接状态…",12);linkHint=text(c,"",14,secondary,8);button(c,"扫码连接 desktop",true,()->startActivity(new Intent(this,PairingActivity.class)));connectionFoot=text(c,"",12,secondary,12);
        LinearLayout guide=card(p,18);heading(guide,"两种连接方式",0);text(guide,"无线连接",15,ink,16);text(guide,"1. 手机和电脑连接同一 Wi-Fi。\n2. desktop 打开“连接设备”并显示二维码。\n3. 在此扫码，确认要连接的电脑。",14,secondary,8);text(guide,"USB 连接",15,ink,18);text(guide,"使用 USB 数据线连接，在手机上确认这台电脑。desktop 连接成功后，此处会显示电脑名称。",14,secondary,8);
        LinearLayout work=card(p,18);heading(work,"Amoo 后台协作",0);text(work,"后台任务需要手机固件支持。可在设置查看适配状态与权限，就绪后再启用。",14,secondary,8);button(work,"管理后台协作",false,()->showTab(2));
    }
    private void filesPage(){LinearLayout p=page("手机与电脑互传","只通过当前已授权的电脑传输，文件会校验完整性。");LinearLayout c=card(p,22);transferHint=text(c,"正在读取电脑连接…",14,secondary,0);send=button(c,busy?"正在准备文件…":"选择文件，发送到电脑",true,this::chooseFiles);text(c,"单个文件最多 256 MB，一次可选择 10 个。",12,secondary,10);notice=text(c,"",13,secondary,8);
        LinearLayout tabs=new LinearLayout(this);LinearLayout.LayoutParams tp=new LinearLayout.LayoutParams(-1,dp(46));tp.topMargin=dp(22);p.addView(tabs,tp);String[] labels={"收到的文件","发出的文件"};for(int i=0;i<2;i++){final int d=i;TextView t=new TextView(this);t.setText(labels[i]);t.setGravity(Gravity.CENTER);t.setTextSize(15);t.setTextColor(transferDirection==i?Color.WHITE:secondary);t.setBackground(surface(transferDirection==i?ink:Color.TRANSPARENT,0,12));tabs.addView(t,new LinearLayout.LayoutParams(0,-1,1));t.setOnClickListener(v->{transferDirection=d;showTab(1);});}
        transferList=column();p.addView(transferList);loadTransfers();
    }
    private void settingsPage(){LinearLayout p=page("由你掌控","管理后台协作、权限和背屏玩法。");LinearLayout c=card(p,22);heading(c,"Amoo 后台协作",0);amooSwitch=new Switch(this);amooSwitch.setText("启用后台协作");amooSwitch.setTextSize(15);amooSwitch.setTextColor(ink);amooSwitch.setPadding(0,dp(16),0,dp(16));c.addView(amooSwitch,new LinearLayout.LayoutParams(-1,-2));amooState=text(c,"",13,secondary,0);text(c,"完成固件适配后，连接期间可按需维持任务，断开电脑后自动释放。权限授权本身不代表熄屏任务已经可用。",13,secondary,10);amooSwitch.setOnCheckedChangeListener((b,checked)->{if(updatingSwitch)return;try{if(checked)notifications();Intent service=new Intent().setClassName(this,"com.stepx.companion.app.AmooConnectionService").putExtra("op",checked?"enable":"disable").putExtra("request_id","phone-"+System.currentTimeMillis());startForegroundService(service);amooState.setText(checked?"正在开启后台协作…":"正在停止后台协作…");}catch(Exception e){toast("操作未完成："+e.getMessage());refresh();}});
        LinearLayout permissions=card(p,18);heading(permissions,"权限与后台运行",0);permissionState=text(permissions,"",14,secondary,10);text(permissions,"desktop 安装时授予功能所需权限。相机仅用于扫码；你之后撤销权限，可在使用时重新授权。文件仍由系统选择器逐次授权。",13,secondary,10);button(permissions,"查看系统权限",false,()->startActivity(new Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS,Uri.parse("package:"+getPackageName()))));
        LinearLayout game=card(p,18);heading(game,"背屏 · 重力对齐",0);text(game,"三敲进入游戏，左右倾斜对齐长条。5 格到 1 格循环挑战，翻到手机正面即可退出。",14,secondary,10);gameState=text(game,"正在读取背屏状态…",13,secondary,12);button(game,"开启三敲游戏",true,()->game("standby"));button(game,"直接开始一局",false,()->game("game"));button(game,"停止并恢复背屏",false,()->game("stop"));text(p,"StepX伴侣 "+version()+"\n文件和扫码均在手机本地处理。",12,secondary,24);
    }
    private void guiControl(String op){GuiAccessibilityService s=GuiAccessibilityService.instance;if(s==null){toast("请先启用看屏服务");return;}try{java.util.concurrent.CompletableFuture<JSONObject> result=new java.util.concurrent.CompletableFuture<>();result.whenComplete((v,e)->ui.post(()->{if(e!=null)toast(e.getMessage());refresh();}));s.request(new JSONObject().put("op","user_"+op).put("task",s.task.id),result);}catch(Exception e){toast(e.getMessage());}}
    private void guiTaskDialog(){
        LinearLayout form=column();form.setPadding(dp(20),dp(8),dp(20),0);
        EditText input=new EditText(this);input.setHint("描述你想在手机上完成的任务");input.setMinLines(2);form.addView(input);
        text(form,"试试这些任务",12,secondary,12);
        for(String example:new String[]{"打开淘宝","打开京东，我的订单","在设置中搜索电池，不修改设置","打开设置，查看 Android 版本"}){
            TextView item=text(form,example,14,ink,10);item.setPadding(0,dp(5),0,dp(5));item.setOnClickListener(v->{input.setText(example);input.setSelection(input.length());});
        }
        new AlertDialog.Builder(this).setTitle("手机独立执行任务")
            .setMessage("需保持手机解锁。Step Plan 视觉模型会分析截图；可从通知栏暂停或停止。")
            .setView(form).setNegativeButton("取消",null)
            .setPositiveButton("开始",(d,w)->{try{PhoneGuiAgent.startFromPhoneUi(this,input.getText().toString());finish();}catch(Exception e){toast(e.getMessage());}}).show();
    }
    private void guiKeyDialog(){EditText input=new EditText(this);input.setInputType(android.text.InputType.TYPE_CLASS_TEXT|android.text.InputType.TYPE_TEXT_VARIATION_PASSWORD);input.setHint("Step Plan API Key");input.setImportantForAutofill(View.IMPORTANT_FOR_AUTOFILL_NO);new AlertDialog.Builder(this).setTitle("Step Plan 模型").setMessage("模型：step-3.7-flash\nAPI：api.stepfun.com/step_plan/v1\n密钥仅加密保存在手机，不写入 Amoo 技能或对话。").setView(input).setNegativeButton("取消",null).setPositiveButton("保存",(d,w)->{try{GuiModelVault.save(this,input.getText().toString());input.setText("");toast("密钥已加密保存");refresh();}catch(Exception e){toast(e.getMessage());}}).show();}
    private String grant(String permission){return checkSelfPermission(permission)==PackageManager.PERMISSION_GRANTED?"已授权":"未授权，可在系统权限中恢复";}
    private void notifications(){if(checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS)!=PackageManager.PERMISSION_GRANTED)requestPermissions(new String[]{Manifest.permission.POST_NOTIFICATIONS},1);}
    private void game(String operation){try{if(!operation.equals("stop")){List<String> missing=new ArrayList<>();if(checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS)!=PackageManager.PERMISSION_GRANTED)missing.add(Manifest.permission.POST_NOTIFICATIONS);if(checkSelfPermission(Manifest.permission.READ_PHONE_STATE)!=PackageManager.PERMISSION_GRANTED)missing.add(Manifest.permission.READ_PHONE_STATE);if(!missing.isEmpty())requestPermissions(missing.toArray(new String[0]),2);}startForegroundService(new Intent(this,CompanionService.class).putExtra("op",operation).putExtra("request_id","phone-"+System.currentTimeMillis()));}catch(Exception e){toast("无法启动背屏："+e.getMessage());}}
    private JSONObject state(String name){try{return TransferStore.readJson(new File(store.root(),name));}catch(Exception ignored){return new JSONObject();}}
    private void refresh(){if(guiState!=null){GuiAccessibilityService s=GuiAccessibilityService.instance;try{JSONObject agent=PhoneGuiAgent.status(this);guiState.setText((agent.optBoolean("modelConfigured")?"Step 3.7 Flash · 密钥已加密保存":"尚未配置模型")+"\n"+(s==null?"请启用看屏服务":s.task.active()?s.task.goal+"\n"+agent.optString("progress",s.task.detail)+" · 已执行 "+s.task.actions+" 步":agent.optString("progress","看屏服务已就绪")));}catch(Exception e){guiState.setText("暂时无法读取任务状态");}}
        TransferStore.Link link=store.link();if(linkTitle!=null){linkTitle.setText(link.online?"已连接 · "+link.name:"尚未连接电脑");linkTitle.setTextColor(link.online?green:ink);linkHint.setText(link.online?"可以互传文件，并在 desktop 中使用 Amoo 与 Amoo Work。":"扫码或通过 USB 连接后，即可开始协作。");connectionFoot.setText(link.online?"连接状态由 desktop 实时确认。":"已配对电脑仍需运行 desktop，连接后自动显示。");}
        if(send!=null){send.setEnabled(link.online&&!busy);send.setAlpha(link.online&&!busy?1f:0.45f);send.setText(busy?"正在准备文件…":"选择文件，发送到电脑");transferHint.setText(link.online?"发送给 "+link.name:"请先连接电脑，再选择要发送的文件。");loadTransfers();}
        if(amooSwitch!=null){
            JSONObject s=state("amoo-connection.json");boolean supported=s.optBoolean("supported",false);
            boolean enabled=getSharedPreferences("amoo_connection",MODE_PRIVATE).getBoolean("enabled",true);
            updatingSwitch=true;amooSwitch.setChecked(supported&&enabled);amooSwitch.setEnabled(supported);updatingSwitch=false;
            String message=s.optString("message"),error=s.optString("lastError");long elapsed=SystemClock.elapsedRealtime();
            boolean fresh=s.optBoolean("active")&&s.optLong("leaseExpiresElapsedMs")>elapsed&&s.optLong("uptimeMs")<=elapsed&&elapsed-s.optLong("uptimeMs")<30000;
            amooState.setText(!supported?(!error.isEmpty()?error:s.length()>0&&!message.isEmpty()?message:"后台协作尚未就绪，请在 desktop 检查手机固件适配状态。"):!enabled?"已停止，可随时重新开启。":!error.isEmpty()?error:fresh&&!message.isEmpty()?message:"已允许，等待 desktop 确认后台协作状态…");
            PowerManager power=getSystemService(PowerManager.class);NotificationManager notifications=getSystemService(NotificationManager.class);
            JSONObject pairing=state("pairing.json");boolean pairingReady=PairingBridgeProvider.available()&&pairing.optBoolean("pairingSupported")&&System.currentTimeMillis()-pairing.optLong("updatedAtMs")<30000;
            permissionState.setText("相机  ·  "+grant(Manifest.permission.CAMERA)+
                "\n通知  ·  "+(checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS)==PackageManager.PERMISSION_GRANTED&&notifications.areNotificationsEnabled()?"已授权":"未开启，可在系统权限中恢复")+
                "\n通话状态  ·  "+grant(Manifest.permission.READ_PHONE_STATE)+
                "\n背屏控制  ·  "+grant("com.stepos.permission.MANAGE_BACK_SCREEN")+
                "\n安全配对权限  ·  "+(PairingController.permitted(this)&&PairingController.settingsPermitted(this)?"已授权":"待 desktop 安装或修复")+
                "\n扫码服务  ·  "+(pairingReady?"已就绪":"尚未就绪，请在 desktop 修复")+
                "\n系统省电限制  ·  "+(power.isIgnoringBatteryOptimizations(getPackageName())?"已豁免":"受系统省电管理")+
                "\n厂商后台策略  ·  系统未提供可读取的状态");
            JSONObject g=state("status.json");String mode=g.optString("mode"),messageGame=mode.equals("standby")?"三敲已开启":mode.equals("game")?"游戏进行中 · 累计 "+g.optLong("score")+" 分":mode.equals("animation")?"正在显示："+g.optString("appliedTitle"):"背屏游戏未启动";String errorGame=g.optString("lastError"),paused=g.optString("pausedReason");gameState.setText(messageGame+(paused.isEmpty()?"":"\n"+paused)+(errorGame.isEmpty()?"":"\n"+errorGame)+"\n最高积分 "+g.optLong("bestScore")+" 分");
        }
    }
    private void chooseFiles(){selectedComputer=store.link();if(!selectedComputer.online){toast("请先连接电脑");refresh();return;}Intent i=new Intent(Intent.ACTION_OPEN_DOCUMENT).addCategory(Intent.CATEGORY_OPENABLE).setType("*/*").putExtra(Intent.EXTRA_ALLOW_MULTIPLE,true);startActivityForResult(i,PICK_FILES);}
    private void loadTransfers(){if(listing||transferList==null||destroyed)return;listing=true;files.execute(()->{try{List<TransferStore.Entry> entries=store.list();StringBuilder key=new StringBuilder();for(TransferStore.Entry e:entries)key.append(e.id).append(e.status).append(e.error);String signature=key.toString();ui.post(()->{listing=false;if(transferList==null||destroyed)return;updateSendNotice(entries);if(lastListing.equals(signature)&&transferList.getChildCount()>0)return;lastListing=signature;renderTransfers(entries);});}catch(Exception e){ui.post(()->{listing=false;if(transferList!=null&&transferList.getChildCount()==0)text(transferList,"暂时无法读取文件："+e.getMessage(),14,secondary,24);});}});}
    private void updateSendNotice(List<TransferStore.Entry> entries){
        if(notice==null||transferDirection!=1||busy)return;
        int received=0,queued=0,failed=0;
        for(TransferStore.Entry e:entries){if(!e.direction.equals("outbox"))continue;if("received".equals(e.status))received++;else if("error".equals(e.status))failed++;else queued++;}
        String summary=received+queued+failed==0?"":queued==0&&failed==0?"电脑已接收全部 "+received+" 个文件。":"电脑已接收 "+received+" 个文件"+(queued>0?" · 等待接收 "+queued+" 个":"")+(failed>0?" · 传输失败 "+failed+" 个":"");
        if(!enqueueError.isEmpty())summary+=(summary.isEmpty()?"":"\n")+enqueueError;
        notice.setText(summary);notice.setTextColor(queued==0&&failed==0&&enqueueError.isEmpty()?green:secondary);
    }
    private void finishQueued(int sent,List<String> errors){
        ui.post(()->{busy=false;if(destroyed)return;enqueueError=errors.isEmpty()?"":errors.size()+" 个文件未能发送："+errors.get(0);
            if(sent>0){transferDirection=1;if(tab==1)showTab(1);if(notice!=null){notice.setText("已提交 "+sent+" 个文件，正在确认电脑接收状态…");notice.setTextColor(secondary);}else toast("已提交 "+sent+" 个文件，可在发出的文件中查看进度。");}
            else if(notice!=null)notice.setText(enqueueError);else toast(enqueueError);
            lastListing="";refresh();loadTransfers();
        });
    }
    private void renderTransfers(List<TransferStore.Entry> entries){transferList.removeAllViews();int count=0;for(TransferStore.Entry e:entries){if(!e.direction.equals(transferDirection==0?"inbox":"outbox"))continue;if(++count>100)break;LinearLayout row=card(transferList,12);TextView filename=text(row,e.name,16,ink,0);filename.setTypeface(null,Typeface.BOLD);String time=android.text.format.DateFormat.format("MM-dd HH:mm",e.createdAt).toString();text(row,TransferStore.size(e.size)+" · "+time,12,secondary,6);String status=e.direction.equals("inbox")?"来自 "+e.computer:"received".equals(e.status)?"电脑已接收":"error".equals(e.status)?"传输失败："+e.error:"等待 "+e.computer+" 接收";text(row,status,13,"received".equals(e.status)?green:secondary,6);
            if(e.direction.equals("inbox")){button(row,"打开文件",true,()->open(e));button(row,"保存到手机…",false,()->save(e));}button(row,"移除这条传输",false,()->confirmDelete(e));}
        if(count==0){LinearLayout empty=card(transferList,16);heading(empty,transferDirection==0?"还没有收到文件":"还没有发出文件",0);text(empty,transferDirection==0?"在 desktop 的 StepX伴侣中选择“发送文件到手机”，接收后会出现在这里。":"连接电脑后，选择上方的文件发送按钮。电脑确认收到后，会显示接收状态。",14,secondary,10);}
    }
    private void open(TransferStore.Entry e){if(busy)return;busy=true;refresh();if(notice!=null)notice.setText("正在校验文件…");files.execute(()->{try{store.verifiedFile(e.direction,e.id);ui.post(()->{busy=false;refresh();if(destroyed)return;try{Intent i=new Intent(Intent.ACTION_VIEW).setDataAndType(e.uri(),TransferStore.mime(e.name)).addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);i.setClipData(ClipData.newRawUri(e.name,e.uri()));startActivity(Intent.createChooser(i,"打开文件"));if(notice!=null)notice.setText("文件校验通过。");}catch(ActivityNotFoundException ex){toast("没有可以打开此格式的应用，请先保存到手机。");}});}catch(Exception ex){finishFiles("打开失败："+ex.getMessage());}});}
    private void save(TransferStore.Entry e){saving=e;Intent i=new Intent(Intent.ACTION_CREATE_DOCUMENT).addCategory(Intent.CATEGORY_OPENABLE).setType(TransferStore.mime(e.name)).putExtra(Intent.EXTRA_TITLE,e.name);startActivityForResult(i,SAVE_FILE);}
    private void confirmDelete(TransferStore.Entry e){new AlertDialog.Builder(this).setTitle("移除这条传输？").setMessage("将删除伴侣中的传输记录和缓存文件。已经保存到手机或电脑的文件不受影响。").setNegativeButton("保留",null).setPositiveButton("移除",(d,w)->files.execute(()->{try{store.delete(e);ui.post(()->{lastListing="";loadTransfers();});}catch(Exception ex){ui.post(()->toast("未能移除："+ex.getMessage()));}})).show();}
    @Override protected void onActivityResult(int request,int result,Intent data){super.onActivityResult(request,result,data);if(result!=RESULT_OK||data==null)return;
        if(request==PICK_FILES){List<Uri> uris=new ArrayList<>();ClipData clips=data.getClipData();if(clips!=null){if(clips.getItemCount()>10){toast("一次最多选择 10 个文件，请重新选择。");return;}for(int i=0;i<clips.getItemCount();i++){Uri u=clips.getItemAt(i).getUri();if(u!=null&&!uris.contains(u))uris.add(u);}}else if(data.getData()!=null)uris.add(data.getData());if(uris.isEmpty())return;TransferStore.Link target=selectedComputer;if(target==null){toast("电脑连接已变化，请重新选择文件。");return;}busy=true;enqueueError="";refresh();if(notice!=null){notice.setText("正在准备 "+uris.size()+" 个文件…");notice.setTextColor(secondary);}files.execute(()->{int sent=0;List<String> errors=new ArrayList<>();for(Uri u:uris){try{store.enqueue(u,target);sent++;}catch(Exception ex){errors.add(ex.getMessage());}}finishQueued(sent,errors);});
        }else if(request==SAVE_FILE&&data.getData()!=null&&saving!=null){TransferStore.Entry entry=saving;saving=null;Uri destination=data.getData();busy=true;refresh();files.execute(()->{try{store.save(entry,destination);finishFiles("已保存到你选择的位置。");}catch(Exception e){finishFiles("保存失败："+e.getMessage());}});}
    }
    private void finishFiles(String message){ui.post(()->{busy=false;if(destroyed)return;refresh();if(notice!=null)notice.setText(message);else toast(message);lastListing="";loadTransfers();});}
    private void toast(String message){if(!destroyed)Toast.makeText(this,message,Toast.LENGTH_LONG).show();}
    private final Runnable tick=new Runnable(){public void run(){if(resumed){refresh();ui.postDelayed(this,2000);}}};
    @Override protected void onResume(){super.onResume();resumed=true;ui.post(tick);}
    @Override protected void onPause(){resumed=false;ui.removeCallbacks(tick);super.onPause();}
    @Override protected void onSaveInstanceState(Bundle out){super.onSaveInstanceState(out);out.putInt("tab",tab);out.putInt("direction",transferDirection);if(saving!=null){out.putString("savingId",saving.id);out.putString("savingDirection",saving.direction);}if(selectedComputer!=null){out.putString("targetId",selectedComputer.id);out.putString("targetName",selectedComputer.name);}}
    @Override protected void onDestroy(){destroyed=true;ui.removeCallbacksAndMessages(null);files.shutdown();super.onDestroy();}
    private static final class NavIcon extends Drawable {final Paint p=new Paint(Paint.ANTI_ALIAS_FLAG);final int kind;NavIcon(int k,int color){kind=k;p.setColor(color);p.setStyle(Paint.Style.STROKE);p.setStrokeWidth(1.8f);p.setStrokeCap(Paint.Cap.ROUND);p.setStrokeJoin(Paint.Join.ROUND);}@Override public void draw(Canvas c){c.save();c.translate(getBounds().left,getBounds().top);c.scale(getBounds().width()/24f,getBounds().height()/24f);if(kind==0){c.drawRoundRect(4,7,20,18,2,2,p);c.drawLine(8,22,16,22,p);c.drawLine(12,18,12,22,p);c.drawArc(7,0,17,10,210,120,false,p);c.drawCircle(12,4,0.8f,p);}else if(kind==1){Path path=new Path();path.moveTo(3,7);path.lineTo(3,20);path.lineTo(21,20);path.lineTo(21,8);path.lineTo(12,8);path.lineTo(10,4);path.lineTo(3,4);path.close();c.drawPath(path,p);}else if(kind==3){c.drawRoundRect(4,3,20,21,3,3,p);c.drawLine(8,8,16,8,p);c.drawLine(8,12,16,12,p);c.drawLine(8,16,12,16,p);}else{c.drawCircle(12,12,7,p);c.drawCircle(12,12,2.5f,p);for(int i=0;i<8;i++){c.save();c.rotate(i*45,12,12);c.drawLine(12,2,12,5,p);c.restore();}}c.restore();}@Override public void setAlpha(int a){p.setAlpha(a);}@Override public void setColorFilter(ColorFilter f){p.setColorFilter(f);}@Override public int getOpacity(){return PixelFormat.TRANSLUCENT;}}
}

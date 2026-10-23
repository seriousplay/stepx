package com.stepx.companion.app;

import android.content.Context;
import android.content.pm.PackageManager;
import android.os.*;
import org.json.JSONObject;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.util.UUID;
import java.util.concurrent.atomic.AtomicBoolean;

/** Fixed pairing operations over the shell-authenticated, certificate-bound Binder bridge. */
final class PairingController implements AutoCloseable {
    static final String PERMISSION="android.permission.MANAGE_DEBUGGING";
    static final String SETTINGS_PERMISSION="android.permission.WRITE_SECURE_SETTINGS";
    interface Listener { void changed(String state,String message); }
    private final Context context;
    private final Listener listener;
    private final Handler main=new Handler(Looper.getMainLooper());
    private final HandlerThread thread=new HandlerThread("StepXSecurePairing");
    private final Handler worker;
    private final AtomicBoolean refreshQueued=new AtomicBoolean();
    private final String pairingId=UUID.randomUUID().toString();
    private volatile JSONObject snapshot=new JSONObject();
    private volatile boolean closed,ready,pairing;
    private volatile int generation;
    private volatile String state="checking",message="正在检查安全扫码连接…";
    private String remoteState="",remoteMessage="";
    private volatile long startedAt;
    PairingController(Context c,Listener l){context=c.getApplicationContext();listener=l;thread.start();worker=new Handler(thread.getLooper());refresh();}
    static boolean permitted(Context c){return c.checkSelfPermission(PERMISSION)==PackageManager.PERMISSION_GRANTED;}
    static boolean settingsPermitted(Context c){return c.checkSelfPermission(SETTINGS_PERMISSION)==PackageManager.PERMISSION_GRANTED;}
    boolean ready(){return ready;}
    boolean supported(){return ready&&snapshot.optBoolean("supported");}
    boolean wirelessEnabled(){return snapshot.optBoolean("wirelessEnabled");}
    int port(){return snapshot.optInt("port",-1);}
    String readinessMessage(){return ready?"此手机暂不支持 Android 安全二维码配对。":message;}
    boolean isPairing(){return pairing;}
    private JSONObject exchange(JSONObject request)throws Exception { return PairingBridgeProvider.exchange(request); }
    void refresh(){if(closed||!refreshQueued.compareAndSet(false,true))return;final int revision=generation;worker.post(()->{try{JSONObject result=exchange(new JSONObject().put("op","pairing_status"));if(!closed&&revision==generation)accept(result);}catch(Exception e){if(!closed&&revision==generation){ready=false;pairing=false;emit("error",e.getMessage());writeStatus();}}finally{refreshQueued.set(false);}});}
    private void accept(JSONObject result){
        boolean becameReady=!ready;ready=true;snapshot=result;
        String next=result.optString("pairingState","idle"),text=result.optString("pairingMessage","安全扫码已就绪。");
        pairing=result.optBoolean("pairingActive")&&pairingId.equals(result.optString("pairingId"));
        if(result.optBoolean("pairingActive")&&!pairing){next="busy";text="另一次安全配对正在进行，扫描新二维码可重新配对。";}
        if(becameReady||!next.equals(remoteState)||!text.equals(remoteMessage)){remoteState=next;remoteMessage=text;emit(next,text);}writeStatus();
    }
    private void command(String op,PairingCode code){
        if(closed)return;final int revision=++generation;
        if(op.equals("pairing_begin")){startedAt=SystemClock.elapsedRealtime();pairing=true;emit("pairing","正在向手机系统提交安全配对…");}
        else if(op.equals("pairing_enable"))emit("awaiting_network","正在准备无线连接，请留意系统 Wi-Fi 确认弹窗…");
        worker.post(()->{try{JSONObject request=new JSONObject().put("op",op);if(op.equals("pairing_begin")||op.equals("pairing_cancel"))request.put("pairingId",pairingId);if(code!=null)request.put("service",code.service).put("password",code.password);JSONObject result=exchange(request);if(!closed&&revision==generation)accept(result);}catch(Exception e){if(!closed&&revision==generation){pairing=false;emit("error",e.getMessage());writeStatus();}}});
    }
    void enableWireless(){command("pairing_enable",null);}
    void disableWireless(){command("pairing_disable",null);}
    void begin(PairingCode code){if(!permitted(context)||!settingsPermitted(context)){update("error","请在 desktop 小工具中安装或修复 StepX伴侣权限。");return;}command("pairing_begin",code);}
    void cancel(){pairing=false;command("pairing_cancel",null);}
    private void emit(String next,String text){state=next;message=text==null?"操作未完成，请重试。":text;String shown=message;main.post(()->{if(!closed)listener.changed(next,shown);});}
    void update(String next,String text){emit(next,text);if(!closed)worker.post(this::writeStatus);}
    void publish(){refresh();}
    private void writeStatus(){try{
        JSONObject j=new JSONObject().put("protocol",1).put("pairingPermissionGranted",permitted(context)).put("wirelessSettingsPermissionGranted",settingsPermitted(context)).put("wirelessEnabled",wirelessEnabled()).put("pairingSupported",supported()).put("pairingBridgeReady",ready).put("pairingState",state).put("pairingMessage",message).put("updatedAtMs",System.currentTimeMillis()).put("pairingStartedElapsedMs",startedAt);
        File dir=context.getExternalFilesDir(null);if(dir==null)return;dir.mkdirs();File tmp=new File(dir,"pairing.json.tmp"),out=new File(dir,"pairing.json");Files.write(tmp.toPath(),j.toString().getBytes(StandardCharsets.UTF_8));Files.move(tmp.toPath(),out.toPath(),StandardCopyOption.REPLACE_EXISTING,StandardCopyOption.ATOMIC_MOVE);
    }catch(Exception ignored){}}
    @Override public void close(){if(closed)return;closed=true;++generation;worker.post(()->{try{exchange(new JSONObject().put("op","pairing_cancel").put("pairingId",pairingId));}catch(Exception ignored){}thread.quitSafely();});main.removeCallbacksAndMessages(null);}
}

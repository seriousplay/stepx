package com.stepx.companion.app;

import android.app.Activity;
import android.content.*;
import android.graphics.*;
import android.net.Uri;
import android.os.*;
import android.provider.DocumentsContract;
import android.view.View;
import android.widget.*;
import org.json.JSONObject;
import java.io.IOException;
import java.util.concurrent.*;

/** Explicit native-gallery share action. No accessibility interception or hidden downloads. */
public final class ExpansionGalleryActivity extends Activity {
    private final ExecutorService worker=Executors.newSingleThreadExecutor();
    private final Handler handler=new Handler(Looper.getMainLooper());
    private CompanionUi ui;private LinearLayout body;private TextView state,detail;private Button retry;private boolean running;
    public void onCreate(Bundle saved){super.onCreate(saved);ui=new CompanionUi(this);LinearLayout root=ui.column();root.setBackgroundColor(CompanionUi.BG);setContentView(root);ui.bars(root);ui.back(root,"取回电脑原图",this::finish);ScrollView scroll=new ScrollView(this);root.addView(scroll,new LinearLayout.LayoutParams(-1,0,1));body=ui.column();body.setPadding(ui.dp(20),ui.dp(12),ui.dp(20),ui.dp(24));scroll.addView(body);state=ui.title(body,"正在核对缩略图…",23,8);detail=ui.text(body,"原件保存在电脑，缩略图保留在手机相册。",14,CompanionUi.MUTED,12);retry=ui.button(body,"重新取回",true,this::retrieve);retry.setVisibility(View.GONE);ui.button(body,"返回相册",false,this::finish);retrieve();}
    private void retrieve(){if(running)return;running=true;retry.setVisibility(View.GONE);state.setText("正在核对缩略图…");
        worker.execute(()->{try{
            Intent intent=getIntent();if(!Intent.ACTION_SEND.equals(intent.getAction()))throw new IOException("请在相册中分享扩容缩略图，再选择取回电脑原图");
            Uri uri=intent.getParcelableExtra(Intent.EXTRA_STREAM);ExpansionStore s=new ExpansionStore(this);JSONObject j=ExpansionGallery.resolve(this,s,uri);
            String id=j.getString("id"),computer=j.optString("desktopName","保存原件的电脑");
            post(()->{state.setText("正在取回原件");detail.setText(j.optString("name")+"\n"+ExpansionActivity.bytes(j.optLong("size"))+" · "+computer+"\n取回后会自动打开，已缓存的原件也可离线查看。");});
            try {
                JSONObject r=s.request(id,"view");long deadline=SystemClock.elapsedRealtime()+300000;
                while(!r.optString("state").equals("ready")){
                    if(Thread.currentThread().isInterrupted())return;
                    if(r.optString("state").equals("error"))throw new IOException(r.optString("error"));
                    if(!s.online(s.host())||!j.optString("desktop").equals(s.host().optString("desktopId")))throw new IOException("保存原件的电脑已离线");
                    if(SystemClock.elapsedRealtime()>deadline)throw new IOException("取回超时，请检查电脑客户端后重试");
                    Thread.sleep(500);r=s.requestState(id);
                }
                post(()->{running=false;open(j);});
            } catch(Exception e){if(e instanceof InterruptedException)return;boolean available=s.online(s.host())&&j.optString("desktop").equals(s.host().optString("desktopId"));String title=available?"暂未取回原件":"原件在电脑，当前离线";String message=j.optString("name")+"\n"+computer+"\n\n"+(available?e.getMessage():"请通过 USB 或无线连接保存原件的电脑，并打开 StepX Companion，然后重试。手机相册中的缩略图仍可查看。");post(()->showError(title,message));}
        }catch(Exception e){post(()->showError("无法取回这张图片",e.getMessage()));}});
    }
    private void post(Runnable action){handler.post(()->{if(!isDestroyed()&&!isFinishing())action.run();});}
    private void showError(String title,String message){running=false;state.setText(title);detail.setText(message);retry.setVisibility(View.VISIBLE);}
    private void open(JSONObject j){try{Uri uri=DocumentsContract.buildDocumentUri(ExpansionStore.DOCS,j.optString("id"));Intent view=new Intent(Intent.ACTION_VIEW).setDataAndType(uri,TransferStore.mime(j.optString("name"))).addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);Intent gallery=new Intent(view).setPackage("com.stepos.media.gallery");try{startActivity(gallery);}catch(ActivityNotFoundException absent){startActivity(view);}finish();}catch(Exception e){showError("原件已取回","没有可打开此格式的应用。可在电脑查看，或在伴侣文件库恢复到手机。");}}
    public void onDestroy(){handler.removeCallbacksAndMessages(null);worker.shutdownNow();super.onDestroy();}
}

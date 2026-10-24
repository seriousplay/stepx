package com.stepx.companion.app;

import android.app.Activity;
import android.os.Bundle;
import android.webkit.*;
import android.widget.*;
import android.graphics.Color;
import org.json.JSONObject;

/** Official sign-in surface. No JS interface, credentials interception or SSL bypass. */
public final class StudioWebActivity extends Activity {
    private CompanionUi design;private WebView web;private TextView status;private boolean checking,destroyed;
    @Override public void onCreate(Bundle state){
        super.onCreate(state);design=new CompanionUi(this);
        LinearLayout layout=new LinearLayout(this);layout.setOrientation(1);layout.setBackgroundColor(CompanionUi.BG);
        layout.setOnApplyWindowInsetsListener((v,i)->{android.graphics.Insets b=i.getInsets(android.view.WindowInsets.Type.systemBars()|android.view.WindowInsets.Type.ime());v.setPadding(b.left,b.top,b.right,b.bottom);return i;});
        LinearLayout bar=new LinearLayout(this);bar.setPadding(design.dp(12),design.dp(8),design.dp(12),design.dp(8));bar.setGravity(android.view.Gravity.CENTER_VERTICAL);
        Button back=new Button(this);back.setText("返回");design.styleButton(back,false);back.setOnClickListener(v->finish());bar.addView(back,new LinearLayout.LayoutParams(design.dp(68),-2));
        status=new TextView(this);status.setPadding(design.dp(10),0,design.dp(10),0);status.setText("Studio 官网 · 请使用手机 Amoo 同一账号登录");status.setTextColor(0xff242428);status.setTextSize(14);bar.addView(status,new LinearLayout.LayoutParams(0,-2,1));
        Button check=new Button(this);check.setText("检查登录");design.styleButton(check,true);check.setOnClickListener(v->check(true));bar.addView(check);layout.addView(bar);
        getWindow().getInsetsController().setSystemBarsAppearance(android.view.WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS|android.view.WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS,android.view.WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS|android.view.WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS);
        web=new WebView(this);web.getSettings().setJavaScriptEnabled(true);web.getSettings().setDomStorageEnabled(true);
        web.getSettings().setAllowFileAccess(false);web.getSettings().setAllowContentAccess(false);web.getSettings().setMixedContentMode(WebSettings.MIXED_CONTENT_NEVER_ALLOW);
        CookieManager.getInstance().setAcceptCookie(true);CookieManager.getInstance().setAcceptThirdPartyCookies(web,true);
        web.setWebChromeClient(new WebChromeClient());
        web.setWebViewClient(new WebViewClient(){
            @Override public boolean shouldOverrideUrlLoading(WebView v,WebResourceRequest r){if(!StudioWeb.allowed(r.getUrl().toString())){status.setText("仅支持 Studio 官方登录与项目页面");return true;}return false;}
            @Override public void onReceivedError(WebView v,WebResourceRequest r,WebResourceError e){
                if(r.isForMainFrame())status.setText("官网页面未载入，请检查网络后重新打开");
            }
            @Override public void onPageFinished(WebView v,String url){if(StudioWeb.allowed(url)){CookieManager.getInstance().flush();check(false);}}
            @Override public void onReceivedSslError(WebView v,SslErrorHandler handler,android.net.http.SslError error){handler.cancel();status.setText("官网证书校验失败，请稍后重试");}
        });
        layout.addView(web,new LinearLayout.LayoutParams(-1,0,1));setContentView(layout);layout.requestApplyInsets();
        getSharedPreferences("studio-web",0).edit().putBoolean("enabled",true).apply();
        if(state==null||web.restoreState(state)==null)web.loadUrl(StudioWeb.ORIGIN+"/projects");
    }
    private void check(boolean explicit){
        if(checking||destroyed)return;checking=true;
        if(explicit)status.setText("正在核对官网登录状态…");
        new Thread(()->{
            String message;boolean done=false;
            try{JSONObject result=StudioWeb.snapshot(this);done=result.optBoolean("authorized");message=done?"已连接 "+result.optString("name")+" · "+result.getJSONArray("items").length()+" 个项目已可同步到电脑":"请完成官网登录，再点击“检查登录”";}
            catch(Exception e){message="官网连接暂未完成，请稍后重试";}
            String text=message;boolean success=done;
            runOnUiThread(()->{checking=false;if(destroyed)return;if(explicit||success)status.setText(text);if(success&&explicit){new android.app.AlertDialog.Builder(this).setTitle("Studio 已连接").setMessage(text+"。之后可自动同步，无需复制作品链接。").setPositiveButton("返回伴侣",(d,w)->finish()).setNegativeButton("继续浏览",null).show();}});
        },"studio-web-check").start();
    }
    @Override protected void onSaveInstanceState(Bundle out){super.onSaveInstanceState(out);if(web!=null)web.saveState(out);}
    @Override public void onBackPressed(){if(web.canGoBack())web.goBack();else super.onBackPressed();}
    @Override public void onDestroy(){destroyed=true;if(web!=null){web.stopLoading();web.destroy();}super.onDestroy();}
}

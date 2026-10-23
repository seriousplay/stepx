package com.stepx.companion.app;

import android.accounts.*;
import android.app.Activity;
import android.content.*;
import android.os.Bundle;
import android.widget.TextView;

/** Uses the system account chooser; never assumes another application's identity. */
public final class StudioAccountActivity extends Activity {
    private TextView status;
    private boolean destroyed;
    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        android.widget.LinearLayout page=new android.widget.LinearLayout(this);page.setOrientation(1);page.setPadding(48,60,48,36);page.setBackgroundColor(0xfff8f8fb);
        android.widget.ScrollView scroll=new android.widget.ScrollView(this);scroll.addView(page);setContentView(scroll);
        scroll.setOnApplyWindowInsetsListener((v,i)->{android.graphics.Insets b=i.getInsets(android.view.WindowInsets.Type.systemBars());v.setPadding(b.left,b.top,b.right,b.bottom);return i;});scroll.requestApplyInsets();
        getWindow().getInsetsController().setSystemBarsAppearance(android.view.WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS|android.view.WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS,android.view.WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS|android.view.WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS);
        TextView title=new TextView(this);title.setText("Studio 账号");title.setTextColor(0xff19191d);title.setTextSize(26);title.setTypeface(null,android.graphics.Typeface.BOLD);page.addView(title);
        status = new TextView(this); status.setTextSize(16);status.setTextColor(0xff62626d);status.setPadding(0,32,0,32);status.setLineSpacing(6,1.2f);page.addView(status);
        android.widget.Button done=new android.widget.Button(this);done.setText("返回作品库");done.setTextColor(0xffffffff);done.setBackgroundTintList(android.content.res.ColorStateList.valueOf(0xff19191d));page.addView(done);done.setOnClickListener(v->{startActivity(new Intent(this,StudioActivity.class));finish();});
        note("正在连接手机账号…");
        if (state == null) {
            Account[] accounts = AccountManager.get(this).getAccountsByType("com.stepos.account");
            if (accounts.length == 1) authorize(accounts[0]);
            else startActivityForResult(AccountManager.newChooseAccountIntent(null, null,
                new String[]{"com.stepos.account"}, null, null, null, null), 41);
        }
    }
    @Override protected void onActivityResult(int request, int result, Intent data) {
        super.onActivityResult(request,result,data);
        if(request==41 && result==RESULT_OK && data!=null) {
            String name=data.getStringExtra(AccountManager.KEY_ACCOUNT_NAME);
            String type=data.getStringExtra(AccountManager.KEY_ACCOUNT_TYPE);
            if(name!=null && "com.stepos.account".equals(type)) {authorize(new Account(name,type));return;}
        }
        note("未授权手机账号；可以稍后重试");
    }
    private void authorize(Account account) {
        Bundle options=new Bundle(); options.putBoolean("invalidateToken",false); options.putBoolean("retry",false);
        AccountManager.get(this).getAuthToken(account,"basic",options,this,future->{
            try {
                Bundle reply=future.getResult();
                String token=reply.getString(AccountManager.KEY_AUTHTOKEN);
                if(token!=null && !token.isEmpty()) new Thread(()->probe(account),"studio-account-check").start();
                else note("系统账号未提供授权凭据（代码 "+reply.getInt(AccountManager.KEY_ERROR_CODE,0)+"）");
            } catch(Exception e) {note("系统账号暂不支持此应用授权："+e.getClass().getSimpleName());}
        },null);
    }
    private void probe(Account expected) {
        try {
            String scope=StudioCloud.scope(expected);
            if(!scope.equals(StudioCloud.scope(StudioCloud.account(this)))) throw new IllegalStateException("手机账号已变化，请重新连接");
            getSharedPreferences("studio-account",0).edit().putBoolean("enabled",true).putString("boundScope",scope).remove("catalog").commit();
            StringBuilder result=new StringBuilder("手机 Amoo 账号已连接\n\n");
            for(String category:new String[]{"应用","智能体","技能"}) {
                org.json.JSONObject page=StudioCloud.page(this,category,1);
                result.append(category).append("：").append(page.getInt("total")).append(" 个\n");
            }
            result.append("\n返回 desktop 即可自动同步，无需复制作品链接。\n同步范围为 Amoo 我的作品；尚未提交真机测试的官网草稿仍在 Studio 网页中。");
            runOnUiThread(()->note(result.toString()));
        } catch(Exception e) {runOnUiThread(()->note("账号已连接，同步暂未完成："+e.getMessage()));}
    }
    @Override public void onDestroy(){destroyed=true;super.onDestroy();}
    private void note(String value) {
        if(destroyed)return;
        status.setText(value);
        getSharedPreferences("studio-account",0).edit().putString("status",value).apply();
    }
}

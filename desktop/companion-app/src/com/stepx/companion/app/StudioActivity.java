package com.stepx.companion.app;

import android.app.*;
import android.content.*;
import android.content.pm.*;
import android.graphics.*;
import android.graphics.drawable.*;
import android.net.Uri;
import android.os.*;
import android.view.*;
import android.widget.*;
import org.json.*;
import java.util.concurrent.*;

/** The phone owns its library. Desktop disconnecting does not remove any entry. */
public final class StudioActivity extends Activity {
    private LinearLayout page;private StudioStore store;private boolean destroyed;
    private final ExecutorService worker=Executors.newSingleThreadExecutor();
    private final int ink=Color.rgb(25,25,29),muted=Color.rgb(113,113,124);
    private int dp(int x){return Math.round(x*getResources().getDisplayMetrics().density);}
    private TextView text(LinearLayout parent,String s,int size,int color){TextView t=new TextView(this);t.setText(s);t.setTextSize(size);t.setTextColor(color);t.setPadding(0,dp(8),0,dp(8));parent.addView(t);return t;}
    private Button button(LinearLayout parent,String s,Runnable action){Button b=new Button(this);boolean primary=s.equals("打开作品");b.setAllCaps(false);b.setText(s);b.setTextSize(15);b.setTextColor(primary?Color.WHITE:ink);b.setMinHeight(dp(48));b.setPadding(dp(12),dp(8),dp(12),dp(8));b.setBackgroundTintList(null);GradientDrawable bg=new GradientDrawable();bg.setColor(primary?ink:Color.WHITE);bg.setCornerRadius(dp(10));if(!primary)bg.setStroke(dp(1),Color.rgb(229,229,235));b.setBackground(bg);LinearLayout.LayoutParams lp=new LinearLayout.LayoutParams(-1,-2);lp.topMargin=dp(8);lp.rightMargin=dp(4);parent.addView(b,lp);b.setOnClickListener(v->action.run());return b;}
    @Override public void onCreate(Bundle state){super.onCreate(state);store=new StudioStore(this);build();String op=state==null?getIntent().getStringExtra("op"):null;String id=getIntent().getStringExtra("id");load(op,id);}
    @Override protected void onNewIntent(Intent intent){super.onNewIntent(intent);setIntent(intent);build();load(intent.getStringExtra("op"),intent.getStringExtra("id"));}
    private void build(){
        ScrollView scroll=new ScrollView(this);page=new LinearLayout(this);page.setOrientation(LinearLayout.VERTICAL);page.setPadding(dp(24),dp(24),dp(24),dp(24));page.setBackgroundColor(Color.rgb(248,248,251));scroll.addView(page);setContentView(scroll);
        scroll.setOnApplyWindowInsetsListener((v,i)->{Insets b=i.getInsets(WindowInsets.Type.systemBars());v.setPadding(b.left,b.top,b.right,b.bottom);return i;});scroll.requestApplyInsets();
        getWindow().getInsetsController().setSystemBarsAppearance(WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS|WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS,WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS|WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS);
        TextView back=text(page,"返回伴侣",14,muted);back.setMinHeight(dp(44));back.setOnClickListener(v->{startActivity(new Intent(this,MainActivity.class));finish();});text(page,"Studio 作品",26,ink).setTypeface(null,Typeface.BOLD);
        text(page,"电脑发送的作品保存在这里，断开电脑后仍可打开。网页作品需要联网；体验链接有效期由 Studio 管理。",14,muted);
        button(page,"连接手机账号 · 同步作品",()->startActivity(new Intent(this,StudioAccountActivity.class)));
        button(page,"Amoo · 我的作品",()->{try{startActivity(new Intent(Intent.ACTION_VIEW,Uri.parse("stepos://amoo/testspace")));}catch(Exception e){message("此手机无法打开 Amoo 我的作品");}});
        text(page,"智能体与技能：在 Studio 真机测试后，使用同一账号到 Amoo 我的作品确认安装。",13,muted);
    }
    private void load(String op,String id){worker.execute(()->{try{JSONArray all=store.list();runOnUiThread(()->{if(destroyed)return;render(all);if(op!=null&&id!=null){try{JSONObject w=null;for(int i=0;i<all.length();i++)if(all.getJSONObject(i).getString("id").equals(id))w=all.getJSONObject(i);if(w==null)throw new Exception("作品不在这台手机中，请重新发送");if(op.equals("open"))open(w);else if(op.equals("pin"))pin(w);}catch(Exception e){message(e.getMessage());}}});}catch(Exception e){runOnUiThread(()->message("读取失败："+e.getMessage()));}});}
    private void render(JSONArray all){
        if(all.length()==0)text(page,"还没有作品\n在电脑 Studio 保存体验链接，再点击“发送到手机”。",16,muted);
        for(int i=0;i<all.length();i++){try{
            JSONObject w=all.getJSONObject(i);LinearLayout card=new LinearLayout(this);card.setOrientation(LinearLayout.VERTICAL);card.setPadding(dp(16),dp(12),dp(16),dp(12));GradientDrawable bg=new GradientDrawable();bg.setColor(Color.WHITE);bg.setCornerRadius(dp(12));bg.setStroke(dp(1),Color.rgb(229,229,235));card.setBackground(bg);LinearLayout.LayoutParams lp=new LinearLayout.LayoutParams(-1,-2);lp.topMargin=dp(18);page.addView(card,lp);
            text(card,w.getString("name"),20,ink).setTypeface(null,Typeface.BOLD);text(card,w.getString("category")+" · Studio 体验链接",13,muted);
            button(card,"打开作品",()->open(w));LinearLayout row=new LinearLayout(this);card.addView(row);
            for(int action=0;action<2;action++){final int which=action;LinearLayout cell=new LinearLayout(this);cell.setOrientation(LinearLayout.VERTICAL);row.addView(cell,new LinearLayout.LayoutParams(0,-2,1));button(cell,action==0?"添加桌面图标":"分享链接",()->{if(which==0)pin(w);else share(w);});}
        }catch(Exception e){message(e.getMessage());}}
    }
    private void open(JSONObject w){try{StudioStore.validate(w);startActivity(new Intent(Intent.ACTION_VIEW,Uri.parse(w.getString("url"))).addCategory(Intent.CATEGORY_BROWSABLE));getSharedPreferences("studio-state",0).edit().putString("lastOpen",w.getString("id")).putLong("lastOpenAt",System.currentTimeMillis()).putString("lastError","").apply();}catch(Exception e){getSharedPreferences("studio-state",0).edit().putString("lastError",String.valueOf(e.getMessage())).apply();message("无法打开："+e.getMessage());}}
    private void share(JSONObject w){try{StudioStore.validate(w);Intent i=new Intent(Intent.ACTION_SEND).setType("text/plain").putExtra(Intent.EXTRA_TEXT,w.getString("name")+"\n"+w.getString("url"));startActivity(Intent.createChooser(i,"分享 Studio 作品"));}catch(Exception e){message(e.getMessage());}}
    private void pin(JSONObject w){try{
        StudioStore.validate(w);ShortcutManager m=getSystemService(ShortcutManager.class);if(m==null||!m.isRequestPinShortcutSupported()){message("当前桌面不支持添加图标，作品仍可从伴侣打开");return;}
        Intent launch=new Intent(this,StudioActivity.class).setAction(Intent.ACTION_VIEW).putExtra("op","open").putExtra("id",w.getString("id"));
        ShortcutInfo s=new ShortcutInfo.Builder(this,w.getString("id")).setShortLabel(w.getString("name")).setIcon(Icon.createWithResource(this,getApplicationInfo().icon)).setIntent(launch).build();
        if(m.requestPinShortcut(s,null))message("已请求添加，请在系统提示中确认；作品库仍保留此入口");else message("桌面未接受添加请求");
    }catch(Exception e){message("添加失败："+e.getMessage());}}
    private void message(String s){if(!destroyed)Toast.makeText(this,s,Toast.LENGTH_LONG).show();}
    @Override public void onDestroy(){destroyed=true;worker.shutdown();super.onDestroy();}
}

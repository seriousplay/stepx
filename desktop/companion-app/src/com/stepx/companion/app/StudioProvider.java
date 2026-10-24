package com.stepx.companion.app;

import android.content.*;
import android.content.pm.ShortcutManager;
import android.database.Cursor;
import android.net.Uri;
import android.os.*;
import android.util.Base64;
import org.json.*;
import java.nio.charset.StandardCharsets;

/** ADB-shell-only bridge. No unauthenticated network server or account tokens. */
public final class StudioProvider extends ContentProvider {
    @Override public boolean onCreate(){return true;}
    @Override public Bundle call(String method,String arg,Bundle extras){
        int uid=Binder.getCallingUid();if(uid!=2000&&uid!=android.os.Process.myUid())throw new SecurityException("不允许的调用方");
        long identity=Binder.clearCallingIdentity();
        Bundle b=new Bundle();JSONObject result;
        try{
            if(!"request".equals(method)||arg==null||arg.length()>32768)throw new IllegalArgumentException("请求无效");
            JSONObject q=new JSONObject(new String(Base64.decode(arg,Base64.DEFAULT),StandardCharsets.UTF_8));StudioStore store=new StudioStore(getContext());
            result=new JSONObject().put("ok",true);
            if(q.optString("op").equals("local-begin")){
                result=StudioLocal.begin(getContext(),q);
            }else if(q.optString("op").equals("local-chunk")){
                result=StudioLocal.chunk(getContext(),q);
            }else if(q.optString("op").equals("local-commit")){
                result=StudioLocal.commit(getContext(),q.getString("upload"));
            }else if(q.optString("op").equals("local-abort")){
                result=StudioLocal.abort(getContext(),q.getString("upload"));
            }else if(q.optString("op").equals("local-list")){
                result.put("items",StudioLocal.list(getContext()));
            }else if(q.optString("op").equals("local-status")){
                result=StudioLocal.status(getContext());
            }else if(q.optString("op").equals("web-api")){
                result=StudioWeb.request(getContext(),q);
            }else if(q.optString("op").equals("web-session")){
                result=StudioWeb.sessionHandoff(getContext(),q.getString("url"));
            }else if(q.optString("op").equals("web-snapshot")){
                result=StudioWeb.snapshot(getContext());
            }else if(q.optString("op").equals("web-disconnect")){
                getContext().getSharedPreferences("studio-web",0).edit().clear().commit();
                new Handler(Looper.getMainLooper()).post(()->android.webkit.CookieManager.getInstance().removeAllCookies(null));
                result.put("authorized",false);
            }else if(q.optString("op").equals("gui-skill-status")){
                result=OfficialGuiSkill.status(getContext());
            }else if(q.optString("op").equals("gui-skill-install")){
                result=OfficialGuiSkill.install(getContext(),q);
            }else if(q.optString("op").equals("cloud-status")){
                result=StudioCloud.status(getContext());
            }else if(q.optString("op").equals("cloud-page")){
                result=StudioCloud.page(getContext(),q.getString("category"),q.getInt("page"));
            }else if(q.optString("op").equals("cloud-disconnect")){
                getContext().getSharedPreferences("studio-account",0).edit().clear().commit();
                result.put("authorized",false);
            }else if(q.optString("op").equals("save")){
                JSONObject work=q.getJSONObject("work");store.save(work);JSONObject saved=store.get(work.getString("id"));
                result.put("saved",saved.getString("id")).put("url",saved.getString("url"));
            }else if(q.optString("op").equals("list")){
                JSONArray all=store.list();result.put("works",all);
            }else if(q.optString("op").equals("status")){
                ShortcutManager manager=getContext().getSystemService(ShortcutManager.class);
                JSONArray pinned=new JSONArray();if(manager!=null)for(android.content.pm.ShortcutInfo item:manager.getPinnedShortcuts())pinned.put(item.getId());
                result.put("count",store.list().length()).put("pinSupported",manager!=null&&manager.isRequestPinShortcutSupported()).put("pinned",pinned);
                result.put("lastOpenAt",getContext().getSharedPreferences("studio-state",0).getLong("lastOpenAt",0));
                result.put("lastOpen",getContext().getSharedPreferences("studio-state",0).getString("lastOpen","")).put("lastError",getContext().getSharedPreferences("studio-state",0).getString("lastError",""));
            }else throw new IllegalArgumentException("未知作品操作");
        }catch(Exception e){result=new JSONObject();try{result.put("ok",false).put("error",String.valueOf(e.getMessage()));}catch(Exception ignored){}}finally{Binder.restoreCallingIdentity(identity);}
        b.putString("result",Base64.encodeToString(result.toString().getBytes(StandardCharsets.UTF_8),Base64.NO_WRAP));return b;
    }
    @Override public Cursor query(Uri u,String[] p,String s,String[] a,String o){throw new UnsupportedOperationException();}
    @Override public String getType(Uri u){return null;}
    @Override public Uri insert(Uri u,ContentValues v){throw new UnsupportedOperationException();}
    @Override public int delete(Uri u,String s,String[] a){throw new UnsupportedOperationException();}
    @Override public int update(Uri u,ContentValues v,String s,String[] a){throw new UnsupportedOperationException();}
}

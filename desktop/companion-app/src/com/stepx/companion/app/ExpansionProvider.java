package com.stepx.companion.app;
import android.content.*;
import android.database.Cursor;
import android.net.Uri;
import android.os.*;
import android.util.Base64;
import org.json.*;
import java.nio.charset.StandardCharsets;
/** Only a locally authorized ADB shell may drive the two-phase migration. */
public final class ExpansionProvider extends ContentProvider {
 public boolean onCreate(){return true;}
 public Bundle call(String method,String arg,Bundle extras){
  int uid=Binder.getCallingUid();if(uid!=2000&&uid!=android.os.Process.myUid())throw new SecurityException("不允许的调用方");long identity=Binder.clearCallingIdentity();JSONObject r;
  try{if(!"request".equals(method)||arg==null||arg.length()>32768)throw new IllegalArgumentException("请求无效");JSONObject q=new JSONObject(new String(Base64.decode(arg,0),StandardCharsets.UTF_8));ExpansionStore s=new ExpansionStore(getContext());r=new JSONObject().put("ok",true);
   switch(q.getString("op")){
   case "status":r=s.status();break;
   case "forget":s.forget(q.getString("id"));break;
   case "offer":r=s.offer(q);break;
   case "configure":r=s.configure(q);break;
   case "list":r.put("items",s.list(q.optString("kind","all"),q.optString("sort","date"),q.optBoolean("descending",true),q.optInt("offset"),q.optInt("limit",100),q.optString("state","all")));break;
   case "scan":r.put("items",s.scan(q.getString("desktopId")));break;
   case "inspect":r.put("report",s.inspect(q.optJSONObject("policy")));break;
   case "prepare":r.put("file",s.prepare(q));break;
   case "commit":r.put("file",s.commit(q));break;
   case "finalize":r.put("file",s.finalizeCleanup(q));break;
   case "get":r.put("file",s.get(q.getString("id")));break;
   case "fail":r.put("file",s.fail(q.getString("id"),q.optString("error")));break;
   case "requests":r.put("items",s.requests(q.getString("desktopId")));break;
   case "request":r.put("request",s.request(q.getString("id"),q.getString("mode")));break;
   case "request-state":r.put("request",s.requestState(q.getString("id")));break;
   case "request-error":s.requestError(q.getString("id"),q.optString("error"));break;
   case "deliver":r.put("file",s.deliver(q));break;
   default:throw new IllegalArgumentException("未知扩容操作");}
  }catch(Exception e){r=new JSONObject();try{r.put("ok",false).put("error",String.valueOf(e.getMessage()));}catch(Exception ignored){}}finally{Binder.restoreCallingIdentity(identity);}
  Bundle b=new Bundle();b.putString("result",Base64.encodeToString(r.toString().getBytes(StandardCharsets.UTF_8),Base64.NO_WRAP));return b;
 }
 public Cursor query(Uri u,String[] p,String s,String[] a,String o){throw new UnsupportedOperationException();}
 public String getType(Uri u){return null;}public Uri insert(Uri u,ContentValues v){throw new UnsupportedOperationException();}public int delete(Uri u,String s,String[] a){throw new UnsupportedOperationException();}public int update(Uri u,ContentValues v,String s,String[] a){throw new UnsupportedOperationException();}
}

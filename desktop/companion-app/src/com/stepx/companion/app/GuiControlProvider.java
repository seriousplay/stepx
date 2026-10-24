package com.stepx.companion.app;
import android.content.*;
import android.database.Cursor;
import android.net.Uri;
import android.os.*;
import android.util.Base64;
import org.json.JSONObject;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.*;
/** Shell-only authenticated local control; never accepts arbitrary shell commands. */
public final class GuiControlProvider extends ContentProvider {
    @Override public boolean onCreate(){return true;}
    @Override public Bundle call(String method,String arg,Bundle extras){
        int uid=Binder.getCallingUid();if(uid!=2000&&uid!=android.os.Process.myUid())throw new SecurityException("不允许的调用方");
        long identity=Binder.clearCallingIdentity();
        Bundle b=new Bundle();
        try{
            if(!"request".equals(method)||arg==null||arg.length()>32768)throw new IllegalArgumentException("请求无效");
            JSONObject q=new JSONObject(new String(Base64.decode(arg,Base64.DEFAULT),StandardCharsets.UTF_8));
            if(q.optString("op").equals("agent")){
                JSONObject value=GuiDelegation.desktop(getContext(),q);
                b.putString("result",Base64.encodeToString(value.toString().getBytes(StandardCharsets.UTF_8),Base64.NO_WRAP));return b;
            }
            GuiAccessibilityService service=GuiAccessibilityService.instance;
            if(service==null)throw new IllegalStateException("请先启用 StepX伴侣的看屏操作服务");
            CompletableFuture<JSONObject> answer=new CompletableFuture<>();
            service.main.post(()->{if(!answer.isDone())service.request(q,answer);});
            JSONObject value;
            try{value=answer.get(8,TimeUnit.SECONDS);}catch(TimeoutException e){answer.cancel(false);throw e;}
            b.putString("result",Base64.encodeToString(value.toString().getBytes(StandardCharsets.UTF_8),Base64.NO_WRAP));
        }catch(Exception error){
            Throwable cause=error.getCause()==null?error:error.getCause();
            try{b.putString("result",Base64.encodeToString(new JSONObject().put("ok",false).put("error",String.valueOf(cause.getMessage())).toString().getBytes(StandardCharsets.UTF_8),Base64.NO_WRAP));}catch(Exception ignored){}
        }finally{Binder.restoreCallingIdentity(identity);}return b;
    }
    @Override public Cursor query(Uri u,String[] p,String s,String[] a,String o){throw new UnsupportedOperationException();}
    @Override public String getType(Uri u){return null;}
    @Override public Uri insert(Uri u,ContentValues v){throw new UnsupportedOperationException();}
    @Override public int delete(Uri u,String s,String[] a){throw new UnsupportedOperationException();}
    @Override public int update(Uri u,ContentValues v,String s,String[] a){throw new UnsupportedOperationException();}
}

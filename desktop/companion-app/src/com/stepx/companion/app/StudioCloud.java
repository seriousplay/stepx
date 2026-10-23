package com.stepx.companion.app;

import android.accounts.*;
import android.content.Context;
import android.os.Bundle;
import org.json.*;
import java.io.*;
import java.net.*;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.concurrent.TimeUnit;

/** Phone-owned official account authorization. Tokens never cross the ADB bridge. */
final class StudioCloud {
    static final String TYPE="com.stepos.account";
    static String scope(Account a)throws Exception {
        byte[] hash=MessageDigest.getInstance("SHA-256").digest((TYPE+"\n"+a.name).getBytes(StandardCharsets.UTF_8));
        StringBuilder s=new StringBuilder();for(byte b:hash)s.append(String.format("%02x",b&255));return s.toString();
    }
    static Account account(Context c)throws Exception {
        Account[] all=AccountManager.get(c).getAccountsByType(TYPE);
        if(all.length!=1)throw new IOException("请先在手机授权 Studio 使用 Amoo 账号");
        return all[0];
    }
    static JSONObject status(Context c)throws Exception {
        JSONObject out=new JSONObject().put("ok",true).put("authorized",false);
        if(!c.getSharedPreferences("studio-account",0).getBoolean("enabled",false))return out;
        try {String scope=scope(account(c));if(scope.equals(c.getSharedPreferences("studio-account",0).getString("boundScope","")))out.put("scope",scope).put("authorized",true);}catch(Exception ignored){}
        return out;
    }
    static JSONObject page(Context c,String category,int page)throws Exception {
        if(!c.getSharedPreferences("studio-account",0).getBoolean("enabled",false))throw new IOException("请先连接手机账号");
        if(page<1||page>4)throw new IOException("作品页码无效");
        Account a=account(c);String scope=scope(a);
        if(!scope.equals(c.getSharedPreferences("studio-account",0).getString("boundScope","")))throw new IOException("账号已变化，请重新连接手机账号");
        Bundle options=new Bundle();options.putBoolean("invalidateToken",false);options.putBoolean("retry",false);
        AccountManagerFuture<Bundle> future=AccountManager.get(c).getAuthToken(a,"basic",options,false,null,null);
        Bundle reply;
        try{reply=future.getResult(8,TimeUnit.SECONDS);}finally{if(!future.isDone())future.cancel(true);}
        String token=reply.getString(AccountManager.KEY_AUTHTOKEN);
        if(token==null||token.isEmpty()||reply.containsKey(AccountManager.KEY_INTENT))throw new IOException("手机账号需要重新授权，请点击连接账号");
        JSONObject body=new JSONObject().put("testing",1);String method,key;
        switch(category){
            case "应用":method="GetAppList";key="apps";body.put("page_number",page).put("page_size",50).put("keyword","").put("app_type","");break;
            case "智能体":method="GetOrSearchAgentList";key="agents";body.put("page",page).put("size",50).put("keyword","");break;
            case "技能":method="GetSkillList";key="skills";body.put("page_number",page).put("page_size",50).put("necessary",0).put("reload",-1).put("scene",-1).put("use_cache",0);break;
            default:throw new IOException("未知作品类型");
        }
        JSONObject payload=request(method,token,body);
        if(!scope.equals(scope(account(c)))||!c.getSharedPreferences("studio-account",0).getBoolean("enabled",false))throw new IOException("账号已变更，请重新同步");
        JSONArray raw=payload.getJSONArray(key),items=new JSONArray();
        if(raw.length()>50)throw new IOException("官网返回的作品页过大");
        for(int i=0;i<raw.length();i++)items.put(normalize(raw.getJSONObject(i),category));
        return new JSONObject().put("ok",true).put("scope",scope).put("items",items).put("total",payload.getInt("total")).put("page",page);
    }
    private static JSONObject request(String method,String token,JSONObject body)throws Exception {
        URL url=new URL("https://biz-gw.stepaos.com/skillhub/skillmarket/step.otter.skillhub.skillmarket/"+method);
        HttpURLConnection c=(HttpURLConnection)url.openConnection();
        try {
            c.setConnectTimeout(5000);c.setReadTimeout(5000);c.setInstanceFollowRedirects(false);c.setRequestMethod("POST");c.setDoOutput(true);
            c.setRequestProperty("Authorization",token.startsWith("Bearer ")?token:"Bearer "+token);
            c.setRequestProperty("Content-Type","application/json");
            try(OutputStream out=c.getOutputStream()){out.write(body.toString().getBytes(StandardCharsets.UTF_8));}
            int code=c.getResponseCode();if(code==401||code==403)throw new IOException("官方作品服务要求重新授权账号");if(code!=200)throw new IOException("官方作品服务暂不可用（HTTP "+code+"）");
            byte[] bytes;try(InputStream in=c.getInputStream()){bytes=in.readNBytes(1024*1024+1);}
            if(bytes.length>1024*1024)throw new IOException("官网返回数据过大");
            JSONObject j=new JSONObject(new String(bytes,StandardCharsets.UTF_8));
            if(j.optInt("code",-1)!=0)throw new IOException("官方作品服务未完成请求（代码 "+j.optInt("code",-1)+"）");
            return j.getJSONObject("data");
        }finally{c.disconnect();}
    }
    private static String value(JSONObject j,int max,String... keys)throws Exception {
        for(String k:keys){Object v=j.opt(k);if(v instanceof String&&!((String)v).trim().isEmpty()){String s=((String)v).trim();if(s.length()>max) s=s.substring(0,max);return s.replaceAll("[\\p{Cntrl}&&[^\\n\\t]]","");}}return "";
    }
    static JSONObject normalize(JSONObject j,String category)throws Exception {
        String id=value(j,200,"appId","app_id","agentId","agent_id","id");
        if(id.isEmpty())throw new IOException("作品缺少编号");
        String name=value(j,120,"displayName","display_name","name");if(name.isEmpty())name="未命名作品";
        String url=value(j,2048,"h5Url","h5_url");
        // Only public Studio test URLs are transferable. Signed download URLs stay on phone.
        if(!url.matches("https://studio-api\\.stepx\\.com/t/[A-Za-z0-9_-]{1,128}"))url="";
        return new JSONObject().put("id",id).put("name",name).put("category",category).put("url",url)
            .put("description",value(j,1200,"description")).put("version",value(j,80,"version"))
            .put("installed",j.optInt("installStatus",j.optInt("install_status",-1)))
            .put("updated",value(j,80,"updatedAt","updated_at"));
    }
}

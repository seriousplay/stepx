package com.stepx.companion.app;

import android.accounts.*;
import android.content.*;
import android.os.Bundle;
import java.io.*;
import java.net.*;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.TimeUnit;
import org.json.*;

/** Official account calls stay on the phone. Only our existing private test skill is writable. */
final class OfficialGuiSkill {
    private static final String MARKET="/skillhub/skillmarket/step.otter.skillhub.skillmarket/";
    private static final String BOT="/botmsg/step.agi.botmsg.BotMsg/";
    private static final class Session {
        final String token,scope;
        Session(Context c)throws Exception {
            JSONObject status=StudioCloud.status(c);
            if(!status.optBoolean("authorized"))throw new IOException("请先在 Studio 中连接手机 Amoo 账号");
            Account a=StudioCloud.account(c);scope=StudioCloud.scope(a);
            Bundle options=new Bundle();options.putBoolean("invalidateToken",false);options.putBoolean("retry",false);
            AccountManagerFuture<Bundle> f=AccountManager.get(c).getAuthToken(a,"basic",options,false,null,null);
            Bundle reply;try{reply=f.getResult(8,TimeUnit.SECONDS);}finally{if(!f.isDone())f.cancel(true);}
            token=reply.getString(AccountManager.KEY_AUTHTOKEN);
            if(token==null||token.isEmpty()||reply.containsKey(AccountManager.KEY_INTENT))throw new IOException("手机账号需要重新授权，请在 Studio 连接账号");
        }
        void check(Context c)throws Exception {
            JSONObject now=StudioCloud.status(c);
            if(!now.optBoolean("authorized")||!scope.equals(now.optString("scope")))throw new IOException("手机账号已变化，请重新连接");
        }
    }
    private static JSONObject call(Session s,String path,JSONObject body)throws Exception {
        if(!path.equals(MARKET+"GetSkillList")&&!path.equals(BOT+"ListSkills")&&!path.equals(BOT+"InstallClawSkill"))throw new IOException("不支持的技能请求");
        HttpURLConnection c=(HttpURLConnection)new URL("https://biz-gw.stepaos.com"+path).openConnection();
        try {
            c.setConnectTimeout(5000);c.setReadTimeout(8000);c.setInstanceFollowRedirects(false);c.setRequestMethod("POST");c.setDoOutput(true);
            c.setRequestProperty("Authorization",s.token.startsWith("Bearer ")?s.token:"Bearer "+s.token);c.setRequestProperty("Content-Type","application/json");
            try(OutputStream out=c.getOutputStream()){out.write(body.toString().getBytes(StandardCharsets.UTF_8));}
            int http=c.getResponseCode();InputStream stream=http>=400?c.getErrorStream():c.getInputStream();
            if(stream==null)return new JSONObject().put("code",-1).put("http",http).put("message","官方技能服务没有返回状态");
            byte[] bytes;try(InputStream in=stream){bytes=in.readNBytes(1024*1024+1);}
            if(bytes.length>1024*1024)throw new IOException("官方技能响应过大");
            JSONObject j;
            try{j=new JSONObject(new String(bytes,StandardCharsets.UTF_8));}catch(JSONException e){return new JSONObject().put("code",-1).put("http",http).put("message","官方技能服务返回 HTTP "+http+"，响应不是 JSON；未确认安装成功");}
            return j.put("http",http);
        }finally{c.disconnect();}
    }
    private static JSONObject summary(JSONObject j)throws Exception {
        return new JSONObject().put("http",j.optInt("http")).put("code",j.optInt("code",-1))
            .put("message",OfficialGuiSkillPolicy.safeMessage(j.optString("message",j.optString("msg"))));
    }
    private static JSONObject clean(JSONObject row)throws Exception {
        JSONObject out=new JSONObject();
        for(String key:new String[]{"id","name","displayName","display_name","version","installVersion","install_status","installStatus","status","statusMessage"}){
            Object v=row.opt(key);if(v instanceof Number||v instanceof Boolean)out.put(key,v);
            else if(v instanceof String)out.put(key,key.equals("statusMessage")?OfficialGuiSkillPolicy.safeMessage((String)v):OfficialGuiSkillPolicy.identityText((String)v));
        }
        return out;
    }
    private static JSONObject candidate(Session s)throws Exception {
        JSONObject j=call(s,MARKET+"GetSkillList",new JSONObject().put("testing",1).put("page_number",1).put("page_size",50).put("necessary",0).put("reload",-1).put("scene",-1).put("use_cache",0));
        if(j.optInt("http")!=200||j.optInt("code",-1)!=0)throw new IOException("无法读取官方测试技能（代码 "+j.optInt("code",-1)+"）："+summary(j).optString("message"));
        JSONArray rows=j.getJSONObject("data").getJSONArray("skills");
        for(int i=0;i<rows.length();i++){JSONObject row=rows.getJSONObject(i);if(OfficialGuiSkillPolicy.ID.equals(row.optString("id"))){
            String title=row.optString("displayName");if(title.isEmpty())title=row.optString("display_name");if(title.isEmpty())title=row.optString("name");
            OfficialGuiSkillPolicy.target(row.optString("id"),row.optString("name"),title,row.optString("version"));return row;}}
        throw new IOException("当前账号中未找到 StepX伴侣的官方测试技能");
    }
    private static JSONObject installed(Session s,String name)throws Exception {
        // Mirrors the original app's GetSkillInstallStatus.nameIn JSON field.
        JSONObject j=call(s,BOT+"ListSkills",new JSONObject().put("nameIn",new JSONArray().put(name)));
        JSONObject out=summary(j);JSONArray found=new JSONArray();JSONObject data=j.optJSONObject("data");JSONArray rows=data==null?null:data.optJSONArray("skills");
        if(rows!=null)for(int i=0;i<rows.length();i++){JSONObject row=rows.optJSONObject(i);if(row!=null&&name.equals(row.optString("name")))found.put(clean(row));}
        return out.put("skills",found);
    }
    static JSONObject status(Context c)throws Exception {
        Session s=new Session(c);JSONObject row=candidate(s),state=installed(s,row.getString("name"));s.check(c);
        return new JSONObject().put("ok",true).put("scope",s.scope).put("candidate",clean(row)).put("installedState",state);
    }
    static synchronized JSONObject install(Context c,JSONObject q)throws Exception {
        if(!NeoAgent.installed(c))throw new IOException("当前是标准伴侣，请先安装包含 Neo Agent Touch 的完整伴侣 APK");
        Session s=new Session(c);if(!s.scope.equals(q.optString("scope")))throw new IOException("账号已变化，请重新检查技能");
        JSONObject row=candidate(s);String version=row.getString("version"),requestId=q.optString("requestId");
        OfficialGuiSkillPolicy.install(q.optString("version"),version,q.optBoolean("testing"),requestId);
        String name=row.getString("name");JSONObject before=installed(s,name);s.check(c);
        if(before.optInt("http")!=200||before.optInt("code",-1)!=0)throw new IOException("无法核对当前技能状态，未发起安装："+before.optString("message"));
        SharedPreferences p=c.getSharedPreferences("official-gui-skill",0);String key=s.scope+":"+requestId;
        if(p.contains(key))throw new IOException("此安装请求已提交，请查询状态，不要重复安装");
        if(!p.edit().putString(key,"submitted").commit())throw new IOException("无法保存安装请求，未发起安装");
        JSONObject ack=call(s,BOT+"InstallClawSkill",new JSONObject().put("skill_name",name).put("version",version).put("force",true).put("testing",true));
        s.check(c);JSONObject after;try{after=installed(s,name);}catch(Exception e){after=new JSONObject().put("code",-1).put("message","安装请求已提交，状态回读暂不可用");}
        return new JSONObject().put("ok",ack.optInt("http")==200&&ack.optInt("code",-1)==0).put("scope",s.scope).put("candidate",clean(row)).put("before",before).put("install",summary(ack)).put("after",after);
    }
    static String display(JSONObject result){
        JSONObject ack=result.optJSONObject("install");if(ack!=null&&(ack.optInt("http")!=200||ack.optInt("code",-1)!=0))return "官方技能安装未完成（HTTP "+ack.optInt("http")+"，代码 "+ack.optInt("code",-1)+"）\n"+ack.optString("message");
        JSONObject state=result.optJSONObject(ack==null?"installedState":"after");JSONArray skills=state==null?null:state.optJSONArray("skills");
        if(skills!=null&&skills.length()>0){JSONObject item=skills.optJSONObject(0);return "官方技能状态："+item.optString("status","待确认")+"\n版本 "+item.optString("installVersion",item.optString("version","未知"))+"\n普通对话调用仍需实际验收。";}
        return ack==null?"官方测试技能尚未安装。":"安装请求已提交，尚未确认安装完成。请稍后重新检查。";
    }
}

package com.stepx.companion.app;

import android.content.Context;
import android.webkit.CookieManager;
import org.json.*;
import java.io.*;
import java.net.*;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;

/** Studio's own website session, created by its official login UI in our WebView.
 * API requests keep credentials on the phone. Only the explicit sessionHandoff
 * operation transfers this app's website cookies to Companion's temporary
 * desktop editor window; native Amoo account credentials are never exported. */
final class StudioWeb {
    static final String ORIGIN="https://studio.stepx.com";
    private static final String CREATED_PREFS="studio-created-projects";
    static boolean validProject(String id){return id!=null&&id.matches("p_[A-Za-z0-9_-]{1,126}");}
    static String createdKey(String scope,String id)throws IOException {
        if(scope==null||!scope.matches("[a-f0-9]{64}")||!validProject(id))throw new IOException("官网账号或项目编号无效");
        return scope+":"+id;
    }
    static boolean mayWriteProject(String scope,String id,java.util.Set<String> records){
        try{return records.contains(createdKey(scope,id));}catch(IOException e){return false;}
    }
    static boolean mayRecordProject(String scope,String id,java.util.Set<String> existing,java.util.Set<String> records){
        try{createdKey(scope,id);return !existing.contains(id)||mayWriteProject(scope,id,records);}catch(IOException e){return false;}
    }
    private static java.util.Set<String> createdRecords(Context context){java.util.Set<String> keys=new java.util.HashSet<>();for(java.util.Map.Entry<String,?> e:context.getSharedPreferences(CREATED_PREFS,0).getAll().entrySet())if(Boolean.TRUE.equals(e.getValue()))keys.add(e.getKey());return keys;}
    static String sourcePath(String path)throws Exception {
        URI relative=new URI(path);String query=relative.getRawQuery();
        if(relative.getScheme()!=null||relative.getRawAuthority()!=null||relative.getRawFragment()!=null||query==null||!relative.getRawPath().matches("/api/projects/p_[A-Za-z0-9_-]{1,126}/file"))throw new IOException("源码请求地址无效");
        String source=null;boolean kindSeen=false;
        for(String param:query.split("&",-1)){
            String[] pair=param.split("=",2);if(pair.length!=2)throw new IOException("源码查询参数无效");
            if(pair[0].equals("path")){if(source!=null)throw new IOException("源码路径重复");source=URLDecoder.decode(pair[1],"UTF-8");}
            else if(pair[0].equals("kind")&&!kindSeen&&pair[1].equals("file")){kindSeen=true;}
            else throw new IOException("源码查询参数无效");
        }
        if(source==null||source.isEmpty()||source.length()>240||source.startsWith("/")||source.contains("\\")||source.contains(":")||source.contains("%")||source.chars().anyMatch(c->c<32||c==127))throw new IOException("源码路径无效");
        for(String part:source.split("/",-1))if(part.isEmpty()||part.equals(".")||part.equals(".."))throw new IOException("源码路径无效");
        return source;
    }
    static boolean allowed(String raw) {
        try { URI u=new URI(raw);String h=u.getHost();return "https".equals(u.getScheme()) && u.getRawUserInfo()==null && (u.getPort()==-1||u.getPort()==443)
            && ("studio.stepx.com".equals(h)||"studio-api.stepx.com".equals(h)||"login-studio.stepx.com".equals(h));
        }catch(Exception e){return false;}
    }
    static JSONObject get(String path)throws Exception {
        if(!path.equals("/api/identity/me")&&!path.equals("/api/projects"))throw new IOException("不支持的官网请求");
        String target=ORIGIN+path;
        HttpURLConnection c=(HttpURLConnection)new URL(target).openConnection();
        try {
            c.setConnectTimeout(5000);c.setReadTimeout(7000);c.setInstanceFollowRedirects(false);
            c.setRequestProperty("Accept","application/json");c.setRequestProperty("zy-client-lang","zh-CN");
            String cookie=CookieManager.getInstance().getCookie(target);
            if(cookie!=null&&!cookie.isEmpty())c.setRequestProperty("Cookie",cookie);
            int code=c.getResponseCode();
            if(code==401||code==403)throw new LoginRequired();
            if(code!=200)throw new IOException("Studio 官网暂不可用（HTTP "+code+"）");
            byte[] bytes;try(InputStream in=c.getInputStream()){bytes=in.readNBytes(2*1024*1024+1);}
            if(bytes.length>2*1024*1024)throw new IOException("官网项目数据过大");
            return new JSONObject(new String(bytes,StandardCharsets.UTF_8));
        }finally{c.disconnect();}
    }
    static final class LoginRequired extends IOException {LoginRequired(){super("请在手机完成 Studio 官网登录");}}
    static String scope(String id)throws Exception {
        byte[] hash=MessageDigest.getInstance("SHA-256").digest(("studio-web\n"+id).getBytes(StandardCharsets.UTF_8));
        StringBuilder s=new StringBuilder();for(byte b:hash)s.append(String.format("%02x",b&255));return s.toString();
    }
    static JSONObject snapshot(Context context)throws Exception {
        JSONObject out=new JSONObject().put("ok",true).put("authorized",false);
        if(!context.getSharedPreferences("studio-web",0).getBoolean("enabled",false))return out;
        try {
            JSONObject identity=get("/api/identity/me");String id=identity.getString("userId");if(id.isEmpty())throw new IOException("官网账号标识缺失");
            JSONObject response=get("/api/projects");JSONArray raw=response.getJSONArray("projects"),items=new JSONArray();
            if(raw.length()>500)throw new IOException("项目数量超过当前同步上限，请在官网查看");
            for(int i=0;i<raw.length();i++)items.put(normalize(raw.getJSONObject(i)));
            if(!id.equals(get("/api/identity/me").getString("userId"))||!context.getSharedPreferences("studio-web",0).getBoolean("enabled",false))throw new IOException("官网登录账号已变化，请重新同步");
            return out.put("authorized",true).put("scope",scope(id)).put("name",clean(identity.optString("nickname","Studio 账号"),80)).put("items",items);
        }catch(LoginRequired e){return out;}
    }
    static JSONObject normalize(JSONObject j)throws Exception {
        String id=j.getString("id"),kind=j.optString("builderType","app");
        if(!id.matches("p_[A-Za-z0-9_-]{1,126}"))throw new IOException("官网项目编号无效");
        String category;switch(kind){case "app":category="应用";break;case "agent":category="智能体";break;case "plugin":category="技能";break;default:throw new IOException("官网返回了未知项目类型");}
        return new JSONObject().put("id",id).put("name",clean(j.getString("name"),120)).put("category",category)
            .put("url","").put("description",clean(j.optString("description"),1200)).put("version","").put("installed",-1)
            .put("updated",clean(j.optString("updatedAt"),80)).put("source","website");
    }
    static String responseError(int status,byte[] data){
        String prefix="Studio 操作未完成（HTTP "+status+"）";
        try{
            JSONObject body=new JSONObject(new String(data,StandardCharsets.UTF_8));Object raw=body.opt("message");if(!(raw instanceof String))raw=body.opt("error");
            if(raw instanceof JSONObject)raw=((JSONObject)raw).opt("message");
            if(raw instanceof String){String message=clean((String)raw,300);if(!message.isEmpty()&&!message.matches("(?is).*(cookie|authorization|bearer|password|secret|token|https?://|[a-zA-Z0-9_=-]{64,}).*"))return prefix+"："+message;}
        }catch(Exception ignored){}
        return prefix;
    }
    /** Explicit handoff to Companion's own temporary desktop Studio window. */
    static JSONObject sessionHandoff(Context context,String target)throws Exception {
        URI uri=new URI(target);String path=uri.getPath();
        if(!"https".equals(uri.getScheme())||!"studio.stepx.com".equals(uri.getHost())||uri.getRawUserInfo()!=null||(uri.getPort()!=-1&&uri.getPort()!=443)||uri.getRawQuery()!=null||!path.matches("/builder/(app|agent|plugin)/p_[A-Za-z0-9_-]+"))throw new IOException("只能为当前账号的官网项目连接编辑窗口");
        JSONObject info=snapshot(context);if(!info.optBoolean("authorized"))throw new LoginRequired();
        String id=path.substring(path.lastIndexOf('/')+1);boolean found=false;JSONArray items=info.getJSONArray("items");
        for(int i=0;i<items.length();i++)if(id.equals(items.getJSONObject(i).getString("id")))found=true;
        if(!found)throw new IOException("当前官网账号没有此项目，请先同步");
        JSONArray values=new JSONArray();
        for(String origin:new String[]{ORIGIN+"/","https://studio-api.stepx.com/","https://login-studio.stepx.com/"}){
            String raw=CookieManager.getInstance().getCookie(origin);if(raw==null)continue;
            for(String pair:raw.split(";")){int split=pair.indexOf('=');if(split<1)continue;String name=pair.substring(0,split).trim(),value=pair.substring(split+1).trim();
                if(!name.isEmpty()&&!value.isEmpty())values.put(new JSONObject().put("url",origin).put("name",name).put("value",value));
            }
        }
        if(values.length()==0||values.length()>64)throw new LoginRequired();
        return new JSONObject().put("ok",true).put("scope",info.getString("scope")).put("cookies",values);
    }
    /** Fixed project operations; no arbitrary URLs, headers, credentials or redirects. */
    static JSONObject request(Context context,JSONObject q)throws Exception {
        if(!context.getSharedPreferences("studio-web",0).getBoolean("enabled",false))throw new LoginRequired();
        JSONObject me=get("/api/identity/me");String expected=q.getString("scope");
        if(!scope(me.getString("userId")).equals(expected))throw new IOException("官网登录账号已变化，请重新同步");
        String method=q.getString("method"),path=q.getString("path"),body=q.optString("body","");
        boolean allowed=false,creating=method.equals("POST")&&path.equals("/api/projects");java.util.Set<String> beforeCreate=new java.util.HashSet<>();
        if(method.equals("GET")&&(path.equals("/api/chat-models")||path.equals("/api/builder-models")))allowed=true;
        if(creating){
            JSONObject create=new JSONObject(body);
            if(!create.optString("builderType").equals("app")||!create.optString("projectType").equals("web"))throw new IOException("只支持创建独立应用项目");
            java.util.Set<String> keys=new java.util.HashSet<>(java.util.Arrays.asList("name","projectType","builderType","productType","model","description","source","clientRequestId"));
            java.util.Iterator<String> it=create.keys();while(it.hasNext())if(!keys.contains(it.next()))throw new IOException("创建独立项目的参数无效");
            if(!create.optString("clientRequestId").matches("companion-[a-f0-9]{32}")||create.optString("name").trim().isEmpty()||create.optString("name").length()>120||create.optString("model").isEmpty()||create.optString("model").length()>160||create.optString("description").length()>1200)throw new IOException("创建独立项目的参数无效");
            JSONArray existing=get("/api/projects").getJSONArray("projects");for(int i=0;i<existing.length();i++)beforeCreate.add(existing.getJSONObject(i).getString("id"));
            allowed=true;
        }
        String bare=path.split("\\?",2)[0];String[] parts=bare.split("/");
        if(parts.length>=4&&parts[1].equals("api")&&parts[2].equals("projects")&&parts[3].matches("p_[A-Za-z0-9_-]{1,126}")){
            boolean owns=false;JSONArray projects=get("/api/projects").getJSONArray("projects");
            for(int i=0;i<projects.length();i++)if(parts[3].equals(projects.getJSONObject(i).getString("id")))owns=true;
            if(!owns)throw new IOException("项目不属于当前官网账号");
            if(method.equals("GET")&&path.equals(bare)&&(parts.length==4||(parts.length==5&&(parts[4].equals("files")||parts[4].equals("device-tests")||parts[4].equals("sessions")))||(parts.length==6&&parts[4].equals("builds")&&parts[5].equals("latest"))))allowed=true;
            if(method.equals("POST")&&path.equals(bare)&&parts.length==5&&parts[4].equals("device-tests")){if(!q.optString("requestId").matches("[a-fA-F0-9]{8}-[a-fA-F0-9]{4}-[a-fA-F0-9]{4}-[a-fA-F0-9]{4}-[a-fA-F0-9]{12}"))throw new IOException("真机测试请求编号无效");allowed=true;}
            if(method.equals("POST")&&path.equals(bare)&&parts.length==6&&parts[4].equals("sandbox")&&parts[5].equals("ensure")){
                if(!body.isEmpty()||!mayWriteProject(expected,parts[3],createdRecords(context)))throw new IOException("仅支持准备本伴侣新建项目的编辑环境");
                allowed=true;
            }
            if(method.equals("POST")&&path.equals(bare)&&parts.length==6&&parts[4].equals("workspace")&&parts[5].equals("command")){
                if(!mayWriteProject(expected,parts[3],createdRecords(context)))throw new IOException("仅支持保存本伴侣新建项目的工作区");
                JSONObject request=new JSONObject(body),command=request.getJSONObject("command");String action=command.getString("action");
                if(request.length()!=2||!request.has("sessionId")||request.getString("sessionId").isEmpty()||request.getString("sessionId").length()>128||command.length()!=2||!command.optString("requestId").matches("[a-fA-F0-9]{8}-[a-fA-F0-9]{4}-[a-fA-F0-9]{4}-[a-fA-F0-9]{4}-[a-fA-F0-9]{12}")||(!action.equals("status")&&!action.equals("save_checkpoint")))throw new IOException("只支持读取工作区状态或保存源码检查点");
                allowed=true;
            }
            if(method.equals("PATCH")&&path.equals(bare)&&parts.length==4){JSONObject edit=new JSONObject(body);if(edit.length()!=1||!edit.has("model")||edit.getString("model").isEmpty()||edit.getString("model").length()>160)throw new IOException("只支持修改构建模型");allowed=true;}
            if(parts.length==5&&parts[4].equals("file")&&(method.equals("GET")||method.equals("PUT")||method.equals("POST"))){
                String source=sourcePath(path);
                if(!method.equals("GET")){
                    if(!mayWriteProject(expected,parts[3],createdRecords(context)))throw new IOException("为保护已有作品，源码同步仅限本伴侣新建的独立项目");
                    if(!source.equals("index.html")&&!source.equals("package.json"))throw new IOException("本地同步只写入独立项目的入口与构建配置");
                }
                if(q.has("bodyLocalId")){if(!method.equals("PUT")||!source.equals("index.html"))throw new IOException("本地作品只能用于上传页面入口");body=new String(StudioLocal.html(context,q.getString("bodyLocalId")),StandardCharsets.UTF_8);}
                allowed=true;
            }
        }
        if(!allowed||path.contains("#")||path.contains("..")||path.length()>2048)throw new IOException("不支持的 Studio 项目操作");
        if(q.optBoolean("responseDigest")&&(!method.equals("GET")||!bare.endsWith("/file")))throw new IOException("只支持校验源码文件");
        if(body.getBytes(StandardCharsets.UTF_8).length>2*1024*1024)throw new IOException("作品源码过大");
        HttpURLConnection c=(HttpURLConnection)new URL(ORIGIN+path).openConnection();
        try{
            c.setConnectTimeout(5000);c.setReadTimeout(bare.endsWith("/sandbox/ensure")||bare.endsWith("/workspace/command")?30000:12000);c.setInstanceFollowRedirects(false);c.setRequestMethod(method);
            c.setRequestProperty("Accept","application/json");c.setRequestProperty("Origin",ORIGIN);c.setRequestProperty("Referer",ORIGIN+"/projects");
            String cookie=CookieManager.getInstance().getCookie(ORIGIN+path);if(cookie!=null)c.setRequestProperty("Cookie",cookie);
            if(method.equals("POST")&&bare.endsWith("/device-tests"))c.setRequestProperty("Idempotency-Key",q.getString("requestId"));
            if(!method.equals("GET")&&!body.isEmpty()){c.setRequestProperty("Content-Type",bare.endsWith("/file")?"text/plain; charset=utf-8":"application/json");c.setDoOutput(true);try(OutputStream out=c.getOutputStream()){out.write(body.getBytes(StandardCharsets.UTF_8));}}
            int code=c.getResponseCode();if(code==401||code==403)throw new LoginRequired();
            InputStream stream=code>=400?c.getErrorStream():c.getInputStream();byte[] data=stream==null?new byte[0]:stream.readNBytes(2*1024*1024+1);if(stream!=null)stream.close();
            if(data.length>2*1024*1024)throw new IOException("官网响应过大");
            if(code>=300)throw new IOException(responseError(code,data));
            String text=new String(data,StandardCharsets.UTF_8);
            // Large HTML stays on the phone. Returning it Base64 through Binder
            // would exceed its transaction limit before our 2 MB upload limit.
            if(q.optBoolean("responseDigest"))return new JSONObject().put("ok",true).put("status",code).put("sha256",StudioLocal.hash(data)).put("size",data.length);
            if(creating){
                JSONObject created=new JSONObject(text).getJSONObject("project");String id=created.getString("id");
                if(!mayRecordProject(expected,id,beforeCreate,createdRecords(context)))throw new IOException("官网返回了既有项目，已停止源码同步以保留原作品");
                if(!context.getSharedPreferences("studio-web",0).getBoolean("enabled",false)||!scope(get("/api/identity/me").getString("userId")).equals(expected))throw new IOException("官网登录账号已变化；项目可能已创建，请先重新同步");
                if(!context.getSharedPreferences(CREATED_PREFS,0).edit().putBoolean(createdKey(expected,id),true).commit())throw new IOException("项目已创建，但本机同步记录未能保存；请从官网继续编辑");
            }
            return new JSONObject().put("ok",true).put("status",code).put("text",text);
        }finally{c.disconnect();}
    }
    static String clean(String s,int max){s=s.replaceAll("[\\p{Cntrl}&&[^\\n\\t]]","").trim();return s.length()>max?s.substring(0,max):s;}
}

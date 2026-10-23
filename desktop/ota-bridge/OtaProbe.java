import android.content.AttributionSource;
import android.content.ContentResolver;
import android.database.Cursor;
import android.net.Uri;
import android.os.Bundle;
import android.os.Looper;
import android.util.Base64;
import dalvik.system.DexClassLoader;
import org.json.JSONArray;
import org.json.JSONObject;
import java.io.*;
import java.net.*;
import java.nio.charset.StandardCharsets;
import java.util.*;
import java.util.zip.*;
import javax.crypto.Mac;
import javax.crypto.spec.SecretKeySpec;

/** Official updater metadata only. Runs as shell; never installs, enrolls or changes a channel.
 * Identities and native OAuth credentials remain on the phone and are never printed.
 */
public final class OtaProbe {
    static String prop(String key) throws Exception {
        return (String) Class.forName("android.os.SystemProperties").getMethod("get",String.class).invoke(null,key);
    }
    static String readValue(String uri,String selection,String[] args) throws Exception {
        Uri target=Uri.parse(uri);
        Class<?> am=Class.forName("android.app.ActivityManager"), iam=Class.forName("android.app.IActivityManager");
        Object service=am.getMethod("getService").invoke(null);
        int user=(Integer)am.getMethod("getCurrentUser").invoke(null);
        android.os.IBinder token=new android.os.Binder();
        Object holder=iam.getMethod("getContentProviderExternal",String.class,int.class,android.os.IBinder.class,String.class).invoke(service,target.getAuthority(),user,token,"*cmd*");
        if(holder==null) return "";
        try {
            Object provider=holder.getClass().getField("provider").get(holder);
            Bundle extras=new Bundle();
            if(selection!=null) extras.putString(ContentResolver.QUERY_ARG_SQL_SELECTION,selection);
            if(args!=null) extras.putStringArray(ContentResolver.QUERY_ARG_SQL_SELECTION_ARGS,args);
            AttributionSource attribution=new AttributionSource.Builder(android.os.Process.myUid()).setPackageName("com.android.shell").build();
            try(Cursor cursor=(Cursor)Class.forName("android.content.IContentProvider").getMethod("query",AttributionSource.class,Uri.class,String[].class,Bundle.class,Class.forName("android.os.ICancellationSignal")).invoke(provider,attribution,target,null,extras,null)) {
                if(cursor!=null && cursor.moveToFirst()) return cursor.getString(cursor.getColumnIndexOrThrow("value"));
                return "";
            }
        } finally {iam.getMethod("removeContentProviderExternalAsUser",String.class,android.os.IBinder.class,int.class).invoke(service,target.getAuthority(),token,user);}
    }
    static final JSONObject identityErrors=new JSONObject();
    static String optionalValue(String uri,String selection,String[] args) {
        try {String s=readValue(uri,selection,args);return s==null?"":s;} catch(Throwable denied) {
            while(denied.getCause()!=null)denied=denied.getCause();
            try{identityErrors.put(uri.contains("open.DataProvider")?"dsid":uri.contains("openidsdk")?"oaid":"ppid",denied.getClass().getSimpleName());}catch(Exception ignored){}
            return "";
        }
    }
    static android.os.IBinder service(String name) throws Exception {
        return (android.os.IBinder)Class.forName("android.os.ServiceManager").getMethod("getService",String.class).invoke(null,name);
    }
    static Integer actualDeviceState() {
        try {
            String name=(String)Class.forName("android.content.ContextExt").getField("DEVICE_STATE_SERVICE").get(null);
            Object iface=Class.forName("stepos.os.IDeviceStateService$Stub").getMethod("asInterface",android.os.IBinder.class).invoke(null,service(name));
            return (Integer)Class.forName("stepos.os.IDeviceStateService").getMethod("doCheckState",int.class).invoke(iface,1);
        }catch(Throwable unavailable){return null;}
    }
    static JSONObject installStatus() throws Exception {
        JSONObject status=new JSONObject();
        try {
            Class<?> build=Class.forName("android.os.BuildExt");
            java.lang.reflect.Method method;
            try {method=build.getMethod("isShopDemoVersion");}
            catch(NoSuchMethodException olderName){method=build.getMethod("isShopDemo");}
            status.put("shop_demo",method.invoke(null));
        }
        catch(Throwable unavailable){status.put("shop_demo",JSONObject.NULL);}
        try {
            Object pm=Class.forName("android.app.ActivityThread").getMethod("getPackageManager").invoke(null);
            int result=(Integer)Class.forName("android.content.pm.IPackageManager").getMethod("checkPermission",String.class,String.class,int.class).invoke(pm,"com.stepos.update.permission.INTERNAL","com.android.shell",0);
            status.put("internal_permission",result==0);
        }catch(Throwable denied){status.put("internal_permission",JSONObject.NULL);}
        try {
            Object manager=Class.forName("android.os.ISystemUpdateManager$Stub").getMethod("asInterface",android.os.IBinder.class).invoke(null,service("system_update"));
            Bundle info=(Bundle)Class.forName("android.os.ISystemUpdateManager").getMethod("retrieveSystemUpdateInfo").invoke(manager);
            if(info!=null) {
                JSONObject state=new JSONObject();
                for(String key:new String[]{"status","ab_system_update_channel","ab_system_update_mask_id"})if(info.containsKey(key))state.put(key.equals("ab_system_update_channel")?"channel":key.equals("ab_system_update_mask_id")?"mask_id":key,info.get(key));
                status.put("system_update",state);
            }
        }catch(Throwable denied){status.put("system_update_error","unavailable");}
        return status;
    }
    static String enc(String s) throws Exception {return URLEncoder.encode(s,"UTF-8");}
    static byte[] body(String url,JSONObject sys,String key,String secret) throws Exception {
        TreeMap<String,String> params=new TreeMap<>();
        params.put("sys",sys.toString()); params.put("oauth_consumer_key",key);
        params.put("oauth_timestamp",Long.toString(System.currentTimeMillis()/1000));
        params.put("oauth_version","1.0");params.put("oauth_signature_method","HMAC-SHA256");
        StringBuilder canonical=new StringBuilder("POST|"+enc(url)+"|");
        boolean first=true;
        for(Map.Entry<String,String> pair:params.entrySet()) {
            if(!first)canonical.append('&');first=false;
            canonical.append(pair.getKey()).append('=').append(enc(pair.getValue()).replace("+","%20").replace("*","%2A"));
        }
        canonical.append('|').append(secret).append('|');
        Mac mac=Mac.getInstance("HmacSHA256");mac.init(new SecretKeySpec(secret.getBytes(StandardCharsets.UTF_8),"HmacSHA256"));
        params.put("oauth_signature",Base64.encodeToString(mac.doFinal(canonical.toString().getBytes(StandardCharsets.UTF_8)),Base64.NO_WRAP));
        StringBuilder encoded=new StringBuilder();
        for(Map.Entry<String,String> pair:params.entrySet()){if(encoded.length()>0)encoded.append('&');encoded.append(enc(pair.getKey())).append('=').append(enc(pair.getValue()));}
        return encoded.toString().getBytes(StandardCharsets.UTF_8);
    }
    static JSONObject objectField(JSONObject parent,String field) throws Exception {
        Object value=parent.opt(field);
        if(value==null||value==JSONObject.NULL)return null;
        if(value instanceof JSONObject)return (JSONObject)value;
        if(value instanceof String)return new JSONObject((String)value);
        throw new IOException("malformed_response");
    }
    static JSONObject safeRelease(String slot,JSONObject raw) throws Exception {
        JSONObject out=new JSONObject().put("slot",slot);
        for(String key:new String[]{"latestVersion","systemName","systemVersion","releaseDate","verType","markDownText","releaseNote"}) {
            String value=raw.optString(key,"");
            if(value.length()>32768)value=value.substring(0,32768);
            out.put(key,value);
        }
        out.put("existsUpdate",raw.optBoolean("existsUpdate",false));
        out.put("needUpdate",raw.optBoolean("needUpdate",false));
        out.put("size",raw.optLong("size",0));
        for(String flag:new String[]{"mandatoryClear","optionalClear","mandatoryUpgrade"})out.put(flag,raw.optBoolean(flag,false));
        for(String setting:new String[]{"packageType","installation","downloadCondition","upgradeCondition","checkType"})if(raw.has(setting))out.put(setting,raw.get(setting));
        String url=raw.optString("updateUrl","");
        if(url.startsWith("https://") && url.length()<=8192)out.put("download_url",url);
        for(String digest:new String[]{"digest","digestV2"})out.put(digest,raw.optString(digest,""));
        // Transfer metadata goes directly to desktop memory; exported diagnostics exclude URL.
        // Never return signatures, account fields, device identities or test-device lists.
        return out;
    }
    static JSONObject check(String host,String channel,JSONObject sys,String key,String secret) throws Exception {
        String endpoint=host+(channel.equals("current")?"/v4/firmware/check":"/v4/firmware/switchover");
        JSONObject params=new JSONObject(sys.toString());
        if(!channel.equals("current")){params.put("targetFirmwareType",channel);params.remove("ppid");}
        JSONObject out=new JSONObject().put("channel",channel).put("endpoint",endpoint).put("releases",new JSONArray());
        HttpURLConnection conn=null;
        try {
            conn=(HttpURLConnection)new URL(endpoint).openConnection();
            conn.setInstanceFollowRedirects(false);
            conn.setConnectTimeout(12000);conn.setReadTimeout(15000);conn.setRequestMethod("POST");conn.setDoOutput(true);
            conn.setRequestProperty("Content-Type","application/x-www-form-urlencoded; charset=UTF-8");
            conn.setRequestProperty("retryCheckCounts","0");conn.setRequestProperty("isCheckCdnBandWidth","false");
            try(OutputStream output=conn.getOutputStream()){output.write(body(endpoint,params,key,secret));}
            int status=conn.getResponseCode();out.put("http_status",status);
            if(status!=200){out.put("error",status==401||status==403?"access_denied":"http_error");return out;}
            ByteArrayOutputStream bytes=new ByteArrayOutputStream();
            try(InputStream input=conn.getInputStream()) {
                byte[] block=new byte[8192];int count;
                while((count=input.read(block))>=0){if(bytes.size()+count>1024*1024)throw new IOException("response_limit");bytes.write(block,0,count);}
            }
            JSONObject envelope=new JSONObject(bytes.toString("UTF-8"));
            JSONObject reply=objectField(envelope,"reply");if(reply==null)reply=envelope;
            out.put("server_code",reply.optInt("code",-1));
            String message=reply.optString("message","");
            for(String idKey:new String[]{"oaid","dsid","ppid"}) {String id=params.optString(idKey,"");if(!id.isEmpty())message=message.replace(id,"[设备标识已隐藏]");}
            message=message.replaceAll("[A-Za-z0-9_-]{24,}","[已隐藏]").replaceAll("https?://[^\\s]+","[链接已隐藏]");
            out.put("server_message",message.length()>240?message.substring(0,240):message);
            JSONObject value=objectField(reply,"value");
            if(reply.optInt("code",-1)==200 && value==null)throw new IOException("malformed_response");
            JSONArray releases=new JSONArray();
            if(value!=null){
                for(String slot:new String[]{"new","cur"}){JSONObject candidate=objectField(value,slot);if(candidate!=null)releases.put(safeRelease(slot,candidate));}
                if(value.has("latestVersion")) releases.put(safeRelease("candidate",value));
                else if(!value.has("new")&&!value.has("cur")&&!value.has("plan"))throw new IOException("malformed_response");
                JSONObject plan=objectField(value,"plan");
                if(plan!=null)out.put("plan",new JSONObject().put("status",plan.optInt("status",-1)).put("planPublishTime",plan.optLong("planPublishTime",-1)).put("planPublicTimeoutTips",plan.optString("planPublicTimeoutTips","")));
            }
            out.put("releases",releases);
        } catch(Throwable failure) {out.put("error",failure instanceof SocketTimeoutException?"timeout":failure instanceof org.json.JSONException || "malformed_response".equals(failure.getMessage())?"malformed_response":"request_failed");}
        finally {if(conn!=null)conn.disconnect();}
        return out;
    }
    public static void main(String[] args) {
        JSONObject report=new JSONObject();
        try {
            if(args.length<3||args.length>4)throw new IllegalArgumentException();
            String apk=args[0],directory=args[1],environment=args[2];
            if(!apk.startsWith("/system/")&&!apk.startsWith("/product/")&&!apk.startsWith("/system_ext/")&&!apk.startsWith("/data/app/"))throw new IllegalArgumentException();
            Looper.prepareMainLooper();
            File lib=new File(directory,"libupgrade.so");
            try(ZipFile zip=new ZipFile(apk)) {
                ZipEntry entry=zip.getEntry("lib/arm64-v8a/libupgrade.so");
                if(entry==null || entry.getSize()>1024*1024)throw new IOException("unsupported_updater");
                try(InputStream input=zip.getInputStream(entry);OutputStream output=new FileOutputStream(lib)){byte[] buffer=new byte[8192];int n,total=0;while((n=input.read(buffer))>=0){total+=n;if(total>1024*1024)throw new IOException("lib_limit");output.write(buffer,0,n);}}
            }
            DexClassLoader loader=new DexClassLoader(apk,directory,directory,ClassLoader.getSystemClassLoader());
            Class<?> jni=loader.loadClass("com.stepos.update.jni.UpgradeNdkMain");Object instance=jni.getConstructor().newInstance();
            String key=(String)jni.getMethod("getOauthConsumerKey").invoke(instance),secret=(String)jni.getMethod("getOauthSecret").invoke(instance);
            String dsid=optionalValue("content://com.stepos.secureservice.open.DataProvider",null,new String[]{"5"});
            String oaid=optionalValue("content://com.stepos.secureservice.openidsdk/",null,new String[]{"oaid"});
            String ppid=optionalValue("content://com.stepos.secureservice.open.NormalDataProvider","1",new String[]{"1"});
            String version=prop("ro.build.mask.id");if(version.isEmpty())version=prop("ro.build.aios_version");
            String model=prop("ro.vendor.stepos.product.model");if(model.isEmpty())model=android.os.Build.MODEL;
            String domain=environment.equals("dev")?"d-stepos.com":environment.equals("stg")?"s-stepos.com":environment.equals("test")?"t-stepos.com":"stepaos.com";
            String host=(prop("ro.stepos.international").equals("true")?"https://u.in.":"https://upush.")+domain;
            Integer state=actualDeviceState();
            // doCheckState==1 is the official modified-state signal. For other values the
            // updater also consults private verification state, so root remains unknown.
            Integer rootFlag=state!=null && state==1?1:null;
            JSONObject sys=new JSONObject().put("isCheckCdnBandwidth",false).put("retryCheckCounts",0).put("unitType","0")
                .put("oaid",oaid).put("dsid",dsid).put("deviceType",model).put("firmware",android.os.Build.VERSION.RELEASE)
                .put("sysVer",version).put("version",version);
            if(rootFlag!=null)sys.put("root",rootFlag);
            if(!ppid.isEmpty())sys.put("ppid",ppid);
            report.put("protocol",1).put("current_version",version).put("model",model).put("environment",environment)
                .put("identity_complete",!oaid.isEmpty()&&!dsid.isEmpty()&&!ppid.isEmpty())
                .put("identity_fields",new JSONObject().put("oaid",!oaid.isEmpty()).put("dsid",!dsid.isEmpty()).put("ppid",!ppid.isEmpty()))
                .put("eligibility_verified",false).put("root_parameter",rootFlag==null?JSONObject.NULL:rootFlag)
                .put("device_state",state==null?JSONObject.NULL:state).put("identity_errors",identityErrors).put("install_status",installStatus());
            JSONArray channels=new JSONArray();
            if(args.length==3)for(String channel:new String[]{"current","stable","beta","daily"}) channels.put(check(host,channel,sys,key,secret));
            report.put("channels",channels);
        }catch(Throwable error){try{report.put("error","probe_unavailable");}catch(Exception ignored){}}
        System.out.println("STEPX_OTA_RESULT:"+report.toString());
        System.exit(0);
    }
}

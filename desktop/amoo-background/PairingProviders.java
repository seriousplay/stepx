package stepx;

import android.content.AttributionSource;
import android.os.*;
import java.lang.reflect.*;

/** Fixed provider calls using Android's shell-command transport, without an app ProcessRecord. */
final class PairingProviders {
    private PairingProviders(){}
    static Bundle attach(IBinder service)throws Exception{
        Bundle extras=new Bundle();extras.putBinder("service",service);extras.putInt("version",1);
        return call("com.stepx.companion.app.pairing","attach",null,extras);
    }
    static boolean wirelessEnabled()throws Exception{
        Bundle extras=new Bundle();extras.putInt("_user",0);
        Bundle result=call("settings","GET_global","adb_wifi_enabled",extras);
        return result!=null&&"1".equals(result.getString("value"));
    }
    static void setWirelessEnabled(boolean enabled)throws Exception{
        Bundle extras=new Bundle();extras.putInt("_user",0);extras.putString("value",enabled?"1":"0");
        call("settings","PUT_global","adb_wifi_enabled",extras);
        if(wirelessEnabled()!=enabled)throw new IllegalStateException(enabled?"未能开启无线连接，请检查系统网络确认弹窗":"未能关闭无线连接");
    }
    private static Bundle call(String authority,String method,String argument,Bundle extras)throws Exception{
        if(android.os.Process.myUid()!=2000||Build.VERSION.SDK_INT!=36)throw new SecurityException("Unsupported pairing provider identity");
        Object manager=Class.forName("android.app.ActivityManager").getMethod("getService").invoke(null);
        Class<?> type=Class.forName("android.app.IActivityManager");IBinder token=new Binder();Object holder=null;
        try{
            holder=type.getMethod("getContentProviderExternal",String.class,int.class,IBinder.class,String.class).invoke(manager,authority,0,token,"stepx-pairing");
            if(holder==null)throw new IllegalStateException("配对所需的系统组件暂不可用");
            Object provider=holder.getClass().getField("provider").get(holder);
            AttributionSource source=new AttributionSource.Builder(2000).setPackageName("com.android.shell").build();
            return (Bundle)Class.forName("android.content.IContentProvider").getMethod("call",AttributionSource.class,String.class,String.class,String.class,Bundle.class).invoke(provider,source,authority,method,argument,extras);
        }catch(InvocationTargetException error){Throwable cause=error.getCause();if(cause instanceof Exception)throw (Exception)cause;throw error;}
        finally{if(holder!=null)try{type.getMethod("removeContentProviderExternalAsUser",String.class,IBinder.class,int.class).invoke(manager,authority,token,0);}catch(Exception ignored){}}
    }
}

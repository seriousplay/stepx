package stepx;

import android.content.Context;
import android.content.pm.*;
import android.os.*;
import org.json.JSONObject;
import java.security.MessageDigest;

/** ADB-installed shell process with exactly five pairing operations, no network listener. */
public final class PairingDaemon extends Binder {
    private static final String PACKAGE="com.stepx.companion.app";
    private static final String DESCRIPTOR="com.stepx.companion.Pairing.v1";
    private static final String SERVICE="stepx-pairing-v1";
    private final Context context;
    private final String certificate;
    private final int companionUid;
    private volatile String attachStage="starting";
    private PairingDaemon(Context c,String cert,int uid)throws Exception{
        context=c;certificate=cert;companionUid=uid;verifyCompanion();attachInterface(null,DESCRIPTOR);
    }
    private void verifyCompanion()throws Exception{
        PackageManager pm=context.getPackageManager();
        PackageInfo info=pm.getPackageInfo(PACKAGE,PackageManager.GET_SIGNING_CERTIFICATES);
        String[] names=pm.getPackagesForUid(companionUid);
        if(info.applicationInfo==null||info.applicationInfo.uid!=companionUid||info.signingInfo==null||names==null||names.length!=1||!PACKAGE.equals(names[0]))throw new SecurityException("Companion identity mismatch");
        Signature[] signers=info.signingInfo.getApkContentsSigners();
        if(signers.length!=1)throw new SecurityException("Companion signer mismatch");
        StringBuilder hash=new StringBuilder();for(byte value:MessageDigest.getInstance("SHA-256").digest(signers[0].toByteArray()))hash.append(String.format(java.util.Locale.ROOT,"%02x",value&255));
        if(!certificate.equals(hash.toString()))throw new SecurityException("Companion certificate mismatch");
        if(pm.checkPermission("android.permission.MANAGE_DEBUGGING",PACKAGE)!=PackageManager.PERMISSION_GRANTED||pm.checkPermission("android.permission.WRITE_SECURE_SETTINGS",PACKAGE)!=PackageManager.PERMISSION_GRANTED)throw new SecurityException("Companion pairing permission revoked");
    }
    @Override protected boolean onTransact(int code,Parcel data,Parcel reply,int flags)throws RemoteException{
        if(code==INTERFACE_TRANSACTION){reply.writeString(DESCRIPTOR);return true;}
        if(code!=FIRST_CALL_TRANSACTION||reply==null)return false;
        data.enforceInterface(DESCRIPTOR);
        int caller=Binder.getCallingUid();
        if(caller!=companionUid)throw new SecurityException("Untrusted pairing caller");
        String raw=data.readString();
        if(raw==null||raw.length()>2048||data.dataAvail()!=0)throw new SecurityException("Invalid pairing request");
        // In system_server, IAdbManager is a local Binder. Preserve the verified
        // companion caller so its granted MANAGE_DEBUGGING is checked by Android.
        // The shell fallback uses its own shell identity as before.
        boolean shellIdentity=android.os.Process.myUid()==2000;
        JSONObject result;long identity=shellIdentity?Binder.clearCallingIdentity():0;
        try{
            verifyCompanion();
            result=ScopedPairing.handle(context,new JSONObject(raw)).put("ok",true);
        }catch(Exception e){
            result=new JSONObject();try{result.put("ok",false).put("error",e.getMessage()==null?"安全配对未完成":e.getMessage());}catch(Exception ignored){}
        }finally{if(shellIdentity)Binder.restoreCallingIdentity(identity);}
        try{result.put("version",1).put("service",SERVICE).put("pid",android.os.Process.myPid()).put("processUid",android.os.Process.myUid()).put("uid",companionUid);}catch(Exception ignored){}
        reply.writeNoException();reply.writeString(result.toString());return true;
    }
    private void attach()throws Exception{
        attachStage="verify_companion";verifyCompanion();
        attachStage="provider_call";
        Bundle answer=PairingProviders.attach(this);
        attachStage="provider_reply";
        if(answer==null||!answer.getBoolean("accepted"))throw new IllegalStateException("Companion did not accept pairing service");
        attachStage="attached";
    }
    private void diagnostic(Throwable error){
        Throwable source=error;while(source instanceof java.lang.reflect.InvocationTargetException&&source.getCause()!=null)source=source.getCause();
        String message=String.valueOf(source.getMessage()).replaceAll("[\\r\\n\\t]"," ");
        if(source.getCause()!=null)message+="; cause="+source.getCause().getClass().getSimpleName()+": "+String.valueOf(source.getCause().getMessage()).replaceAll("[\\r\\n\\t]"," ");
        // Attach requests contain no QR or password. Still redact long identifiers and bound output.
        message=message.replaceAll("[A-Za-z0-9_-]{20,}","[redacted]");if(message.length()>200)message=message.substring(0,200);
        String line="StepXPairing stage="+attachStage+" error="+source.getClass().getSimpleName()+": "+message;
        System.err.println(line);android.util.Log.w("StepXPairing",line);
    }
    /** Installed only by the pinned boot grant after package/signature validation. */
    public static void installSystem(String certificate,int uid)throws Exception{
        if(android.os.Process.myUid()!=1000||Build.VERSION.SDK_INT!=36)throw new SecurityException("System pairing context required");
        Class<?> threadType=Class.forName("android.app.ActivityThread");
        Object thread=threadType.getMethod("currentActivityThread").invoke(null);
        Context context=(Context)threadType.getMethod("getSystemContext").invoke(thread);
        PairingDaemon daemon=new PairingDaemon(context,certificate,uid);
        // A deliberate repair can replace our old boot instance without restarting Android.
        for(java.util.Map.Entry<Thread,StackTraceElement[]> entry:Thread.getAllStackTraces().entrySet()){
            if(!"StepXBootPairing".equals(entry.getKey().getName()))continue;
            for(StackTraceElement frame:entry.getValue())if(frame.getClassName().startsWith("stepx.PairingDaemon")){entry.getKey().interrupt();break;}
        }
        Thread registration=new Thread(()->{
            // User 0's encrypted app data may be locked at startup. Register after unlock,
            // without opening its activity or modifying the user's lockscreen.
            while(true){try{
                if(context.getSystemService(UserManager.class).isUserUnlocked())daemon.attach();
            }catch(SecurityException revoked){daemon.diagnostic(revoked);return;}catch(Exception unavailable){daemon.diagnostic(unavailable);}
                try{Thread.sleep(10000);}catch(InterruptedException stopped){return;}}
        },"StepXBootPairing");registration.setDaemon(true);registration.start();
    }
    public static void main(String[] args)throws Exception{
        if(android.os.Process.myUid()!=2000||Build.VERSION.SDK_INT!=36||args.length!=2||!args[0].matches("[0-9a-f]{64}")||!args[1].matches("[0-9]{5}"))throw new SecurityException("Unsupported pairing service identity");
        int uid=Integer.parseInt(args[1]);if(uid<10000||uid>=100000)throw new SecurityException("Invalid companion UID");
        Looper.prepareMainLooper();
        Class<?> activityThread=Class.forName("android.app.ActivityThread");
        Object thread=activityThread.getMethod("systemMain").invoke(null);
        Context system=(Context)activityThread.getMethod("getSystemContext").invoke(thread);
        Context context=system.createPackageContext("com.android.shell",0);
        PairingDaemon daemon=new PairingDaemon(context,args[0],uid);
        Runtime.getRuntime().addShutdownHook(new Thread(ScopedPairing::shutdown,"StepXPairingStop"));
        Thread registration=new Thread(()->{
            while(true){try{daemon.attach();}catch(SecurityException revoked){daemon.diagnostic(revoked);ScopedPairing.shutdown();System.exit(1);}catch(Exception unavailable){daemon.diagnostic(unavailable);}
                try{Thread.sleep(10000);}catch(InterruptedException stopped){return;}}
        },"StepXPairingAttach");registration.setDaemon(true);registration.start();
        Looper.loop();
    }
}

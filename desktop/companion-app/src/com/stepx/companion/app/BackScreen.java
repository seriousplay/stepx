package com.stepx.companion.app;

import android.content.Context;
import android.content.SharedPreferences;
import android.os.IBinder;
import android.os.Parcel;
import java.io.IOException;

/** Explicit, audited Step AOS interface. No shell, raw HAL, or arbitrary Binder operations. */
final class BackScreen {
    private static final String DESCRIPTOR="android.hardware.display.IStepOSDisplayManager";
    private final IBinder binder;
    private final SharedPreferences saved;
    boolean active=false;
    BackScreen(Context context) throws Exception {
        saved=context.getSharedPreferences("backscreen_restore",0);
        binder=(IBinder)Class.forName("android.os.ServiceManager").getMethod("getService",String.class).invoke(null,"display_ext");
        if(binder==null||!DESCRIPTOR.equals(binder.getInterfaceDescriptor()))throw new IOException("当前手机不支持 StepX 背屏接口");
    }
    private Parcel call(int code,Integer number,String text,int[]frame,boolean frameCall) throws Exception {
        Parcel data=Parcel.obtain(),reply=Parcel.obtain();
        try {
            data.writeInterfaceToken(DESCRIPTOR);
            if(number!=null)data.writeInt(number);
            if(text!=null)data.writeString(text);
            if(frameCall){data.writeIntArray(frame);data.writeInt(0);}
            if(!binder.transact(code,data,reply,0))throw new IOException("固件未实现背屏接口 "+code);
            reply.readException();return reply;
        }catch(Exception e){reply.recycle();throw e;}finally{data.recycle();}
    }
    private int integer(int code)throws Exception{Parcel r=call(code,null,null,null,false);try{return r.readInt();}finally{r.recycle();}}
    private String string(int code)throws Exception{Parcel r=call(code,null,null,null,false);try{return r.readString();}finally{r.recycle();}}
    private void number(int code,int value)throws Exception{call(code,value,null,null,false).recycle();}
    private void text(int code,String value)throws Exception{call(code,null,value,null,false).recycle();}
    boolean enabled()throws Exception{return integer(17)!=0;}
    void begin(int brightness,boolean lockPortrait)throws Exception{
        if(!enabled())throw new IOException("请先开启手机系统背屏开关");
        if(!saved.getBoolean("pending",false)) {
            int mode=integer(20),bright=integer(16),automatic=integer(14),rotate=integer(32);String type=string(26);
            if(!saved.edit().putBoolean("pending",true).putInt("mode",mode).putInt("brightness",bright).putInt("automatic",automatic).putInt("rotate",rotate).putString("type",type).commit())throw new IOException("无法保存背屏恢复状态");
        }
        number(13,0);number(15,Math.max(16,Math.min(255,brightness*255/8)));number(21,1);
        if(lockPortrait)number(33,0); else number(33,saved.getInt("rotate",1));
        active=true;
    }
    void push(int[]f)throws Exception {if(f==null||f.length!=391)throw new IOException("背屏帧必须为391槽");call(5,null,null,f,true).recycle();}
    void clear()throws Exception{call(5,null,null,null,true).recycle();active=false;}
    void restore(boolean forget)throws Exception{
        if(!saved.getBoolean("pending",false))return;
        clear();number(15,saved.getInt("brightness",127));number(13,saved.getInt("automatic",1));number(33,saved.getInt("rotate",1));
        text(27,saved.getString("type","agent"));number(21,saved.getInt("mode",0));
        if(integer(16)!=saved.getInt("brightness",127)||integer(14)!=saved.getInt("automatic",1)||integer(20)!=saved.getInt("mode",0)||integer(32)!=saved.getInt("rotate",1)||!saved.getString("type","agent").equals(string(26)))throw new IOException("背屏恢复状态未通过校验");
        if(forget&&!saved.edit().clear().commit())throw new IOException("背屏已恢复，但恢复记录尚未清除");
    }
    boolean pending(){return saved.getBoolean("pending",false);}
}

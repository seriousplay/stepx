package com.stepx.companion.neo;

import android.os.*;
import android.view.View;
import android.view.accessibility.AccessibilityNodeInfo;

/** Hide the duplicate overlay only while the companion's visible controls renew a short lease.
 * Leaving the Activity, expiry, another foreground package, or process recreation restores it. */
final class CompanionHostSurface {
    private static final Handler main=new Handler(Looper.getMainLooper());
    private static long expires;
    private static final Runnable check=new Runnable(){public void run(){sync(ControlService.instance);}};
    static void update(boolean visible){main.post(()->{expires=visible?SystemClock.elapsedRealtime()+5000:0;sync(ControlService.instance);});}
    static void sync(ControlService service){
        if(Looper.myLooper()!=Looper.getMainLooper()){main.post(()->sync(service));return;}
        main.removeCallbacks(check);boolean hide=false;
        if(service!=null&&SystemClock.elapsedRealtime()<expires){
            AccessibilityNodeInfo root=service.getRootInActiveWindow();
            if(root!=null){try{hide=BridgePolicy.hostControls(expires,SystemClock.elapsedRealtime(),root.getPackageName()==null?null:root.getPackageName().toString());}finally{root.recycle();}}
        }
        if(service!=null&&service.overlay!=null)service.overlay.setVisibility(hide?View.GONE:View.VISIBLE);
        if(SystemClock.elapsedRealtime()<expires)main.postDelayed(check,500);
    }
}

package com.stepx.companion.app;
import android.content.*;
/** Only restores an opted-in connection. Never starts a GUI task, game, or grants a trust decision. */
public final class ConnectionBootReceiver extends BroadcastReceiver {
    @Override public void onReceive(Context context,Intent intent){if(Intent.ACTION_LOCKED_BOOT_COMPLETED.equals(intent.getAction())||Intent.ACTION_USER_UNLOCKED.equals(intent.getAction())||Intent.ACTION_BOOT_COMPLETED.equals(intent.getAction())||Intent.ACTION_MY_PACKAGE_REPLACED.equals(intent.getAction()))AutoConnectService.resume(context);}
}

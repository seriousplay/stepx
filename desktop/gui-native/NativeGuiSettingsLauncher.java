package stepx;

import android.app.ActivityOptions;
import android.content.ActivityNotFoundException;
import android.content.ComponentName;
import android.content.Context;
import android.content.Intent;
import android.content.pm.ActivityInfo;
import android.content.pm.ApplicationInfo;
import android.content.pm.PackageManager;
import android.os.Binder;
import android.os.Build;
import android.os.Bundle;
import android.os.IBinder;
import android.os.Process;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.util.Objects;

/**
 * Research-only Settings front-door launcher for the fixed About-phone test.
 *
 * An isolated app_process host has no registered IApplicationThread. Passing
 * caller=null to the standard Android 16 activity API lets system_server use
 * the real Binder UID/PID instead. No UID, attribution, permission, background
 * launch exemption, target security flag, or system policy is overridden.
 *
 * This is deliberately narrower than Context.startActivity: only the installed
 * system Settings launcher's explicit MAIN intent, user 0, and display 0 pass.
 * It does not deep-link to About, inject any click, or report visual completion.
 * The original GUI executor must navigate and observe the result itself.
 */
public final class NativeGuiSettingsLauncher {
    private static final String HOST = "com.stepos.ability";
    private static final String SETTINGS = "com.android.settings";
    private static final int USER = 0;
    private static final int DISPLAY = 0;
    private static final String DISPLAY_KEY = "android.activity.launchDisplayId";

    private NativeGuiSettingsLauncher() {}

    /**
     * Called only by a reviewed HostApplication.startActivity override. Returns
     * START_SUCCESS (0), START_TASK_TO_FRONT (2), or START_DELIVERED_TO_TOP (3).
     * These mean the system accepted the launch, not that a GUI task completed.
     * Every other result throws, and a failed/unknown Binder call is not retried.
     */
    public static int startSettingsActivity(Context originalContext, Intent requested,
            Bundle requestedOptions) {
        try {
            verifyHost(originalContext);
            Intent intent = checkedIntent(originalContext, requested);
            Bundle options = checkedOptions(requestedOptions);
            Class<?> taskManager = Class.forName("android.app.IActivityTaskManager");
            Object service = Class.forName("android.app.ActivityTaskManager")
                    .getMethod("getService").invoke(null);
            if (service == null || !taskManager.isInstance(service))
                throw new IllegalStateException("native_settings_activity_service_missing");
            // Exact signature confirmed in this phone's Android 16 services.jar.
            Method start = taskManager.getMethod("startActivityAsUser",
                    Class.forName("android.app.IApplicationThread"), String.class,
                    String.class, Intent.class, String.class, IBinder.class,
                    String.class, int.class, int.class,
                    Class.forName("android.app.ProfilerInfo"), Bundle.class, int.class);
            Object value = start.invoke(service,
                    null, originalContext.getOpPackageName(), originalContext.getAttributionTag(),
                    intent, null, null, null, -1, 0, null, options, USER);
            if (!(value instanceof Integer))
                throw new IllegalStateException("native_settings_activity_result_invalid");
            int result = (Integer) value;
            if (result == 0 || result == 2 || result == 3) return result;
            if (result == -91 || result == -92)
                throw new ActivityNotFoundException("native_settings_activity_not_found:" + result);
            if (result == -94)
                throw new SecurityException("native_settings_activity_permission_denied");
            throw new IllegalStateException("native_settings_activity_not_started:" + result);
        } catch (InvocationTargetException failure) {
            Throwable cause = failure.getCause();
            if (cause instanceof SecurityException) throw (SecurityException) cause;
            if (cause instanceof RuntimeException) throw (RuntimeException) cause;
            // Binder failure can have an unknown side effect. Do not retry.
            throw new IllegalStateException("native_settings_activity_call_failed", cause);
        } catch (ReflectiveOperationException | PackageManager.NameNotFoundException failure) {
            throw new IllegalStateException("native_settings_activity_api_unavailable", failure);
        }
    }

    private static void verifyHost(Context context) throws PackageManager.NameNotFoundException {
        // No clearCallingIdentity here: an untrusted incoming Binder identity
        // must not be promoted to this process's system UID by the helper.
        if (Build.VERSION.SDK_INT != 36 || Process.myUid() != 1000
                || Binder.getCallingUid() != Process.myUid()
                || context == null || !HOST.equals(context.getPackageName())
                || context.getApplicationInfo().uid != Process.myUid()
                || context.getAttributionSource().getUid() != Process.myUid())
            throw new SecurityException("native_settings_host_identity_mismatch");
        String attribution = context.getAttributionSource().getPackageName();
        if (!(HOST.equals(attribution) || "android".equals(attribution))
                || !attribution.equals(context.getOpPackageName())
                || context.getPackageManager().getApplicationInfo(attribution, 0).uid != Process.myUid())
            throw new SecurityException("native_settings_attribution_mismatch");
    }

    private static Intent checkedIntent(Context context, Intent requested)
            throws PackageManager.NameNotFoundException {
        if (requested == null) throw new SecurityException("native_settings_intent_missing");
        // Take a snapshot before validation; no caller-owned Bundle/Intent is
        // forwarded to system_server after validation.
        Intent snapshot = new Intent(requested);
        ComponentName component = snapshot.getComponent();
        if (component == null || !SETTINGS.equals(component.getPackageName())
                || !(snapshot.getPackage() == null || SETTINGS.equals(snapshot.getPackage()))
                || !Intent.ACTION_MAIN.equals(snapshot.getAction())
                || snapshot.getData() != null || snapshot.getType() != null
                || snapshot.getSelector() != null || snapshot.getClipData() != null
                || snapshot.getExtras() != null || snapshot.getIdentifier() != null
                || snapshot.getSourceBounds() != null
                || (snapshot.getFlags() & ~Intent.FLAG_ACTIVITY_NEW_TASK) != 0)
            throw new SecurityException("native_settings_intent_not_allowed");
        PackageManager packages = context.getPackageManager();
        Intent launcher = packages.getLaunchIntentForPackage(SETTINGS);
        if (launcher == null || !component.equals(launcher.getComponent())
                || !Intent.ACTION_MAIN.equals(launcher.getAction())
                || !Objects.equals(snapshot.getCategories(), launcher.getCategories()))
            throw new SecurityException("native_settings_front_door_required");
        ActivityInfo activity = packages.getActivityInfo(component, 0);
        ApplicationInfo application = activity.applicationInfo;
        if (!SETTINGS.equals(activity.packageName) || !activity.exported || !activity.enabled
                || application == null || !application.enabled
                || (application.flags & (ApplicationInfo.FLAG_SYSTEM
                    | ApplicationInfo.FLAG_UPDATED_SYSTEM_APP)) == 0)
            throw new SecurityException("native_settings_system_launcher_required");
        // Construct only the validated public launcher fields. No extras,
        // grant flags, result recipient, creator token, or options are copied.
        Intent safe = new Intent(Intent.ACTION_MAIN).setComponent(component).setPackage(SETTINGS)
                .setFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        if (snapshot.getCategories() != null)
            for (String category : snapshot.getCategories()) safe.addCategory(category);
        return safe;
    }

    private static Bundle checkedOptions(Bundle requested) {
        Bundle safe = ActivityOptions.makeBasic().setLaunchDisplayId(DISPLAY).toBundle();
        if (requested == null) return safe;
        Bundle snapshot = new Bundle(requested);
        Object display = snapshot.get(DISPLAY_KEY);
        if (!(display instanceof Integer) || ((Integer) display) != DISPLAY)
            throw new SecurityException("native_settings_display_zero_required");
        // Original OpenAppAction supplies makeBasic().setLaunchDisplayId(...).
        // Accept only that bundle's runtime default values, then send our own.
        // This also rejects task tokens, BAL exceptions and launch-mode changes.
        for (String key : snapshot.keySet()) {
            if (!safe.containsKey(key) || !Objects.equals(snapshot.get(key), safe.get(key)))
                throw new SecurityException("native_settings_options_not_allowed");
        }
        return safe;
    }
}

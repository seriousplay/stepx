package stepx;

import android.content.Context;
import android.content.pm.ApplicationInfo;
import android.content.pm.PackageManager;
import android.os.Bundle;
import android.os.Looper;
import android.view.Surface;
import org.json.JSONObject;
import java.io.FileInputStream;
import java.io.InputStream;
import java.lang.reflect.InvocationTargetException;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.Locale;

/**
 * Standalone app_process preflight. No application onCreate, native GUI singleton
 * access, account access, SDK initialization, model selection, or task execution.
 */
public final class NativeGuiIsolatedProbe {
    private static final String TARGET = "com.stepos.ability";
    private static final String APK = "/system/priv-app/ability/ability.apk";
    private static final String SHA = "c9f56f3078df96d8d726422b1c622a1c19fe8c7dec8cadd8537d9a6ecf513ac4";

    public static void main(String[] args) {
        JSONObject result = new JSONObject();
        try {
            if (args.length != 0) throw new IllegalArgumentException("This fixed probe takes no arguments");
            result.put("version", 1).put("mode", "isolated_classloader_probe")
                    .put("pid", android.os.Process.myPid()).put("uid", android.os.Process.myUid())
                    .put("selinux_context", readSmall("/proc/self/attr/current"))
                    .put("gui_initialized", false).put("account_accessed", false).put("tasks_started", 0);
            if (android.os.Process.myUid() != 1000) throw new SecurityException("Probe requires the existing system UID service scope");
            String actualHash = apkHash();
            result.put("apk_sha256", actualHash);
            if (!SHA.equals(actualHash)) throw new SecurityException("Unsupported original APK");
            if (Looper.getMainLooper() == null) Looper.prepareMainLooper();
            Class<?> threadClass = Class.forName("android.app.ActivityThread");
            Object thread = threadClass.getMethod("systemMain").invoke(null);
            Context system = (Context) threadClass.getMethod("getSystemContext").invoke(thread);
            Context context = system.createPackageContext(TARGET, Context.CONTEXT_INCLUDE_CODE | Context.CONTEXT_IGNORE_SECURITY);
            ApplicationInfo info = context.getApplicationInfo();
            if (!APK.equals(info.sourceDir) || info.uid != android.os.Process.myUid())
                throw new SecurityException("Package context identity mismatch");
            result.put("system_context_package", system.getPackageName()).put("package_context", context.getPackageName())
                    .put("target_uid", info.uid).put("application_context_present", context.getApplicationContext() != null)
                    .put("current_application_present", threadClass.getMethod("currentApplication").invoke(null) != null)
                    .put("screen_ops_permission", context.checkSelfPermission("com.stepos.ability.permission.SCREEN_OPS") == PackageManager.PERMISSION_GRANTED)
                    .put("agent_service_permission", context.checkSelfPermission("com.stepos.ability.permission.ACCESS_AGENT_SERVICE") == PackageManager.PERMISSION_GRANTED);
            ClassLoader loader = context.getClassLoader();
            // initialize=false is intentional: static INSTANCE fields are not accessed.
            Class<?> bridge = Class.forName("com.stepos.taskrobot.llmapi.stepfun.SystemSdkBridge", false, loader);
            Class<?> callback = Class.forName("com.stepos.taskrobot.common.ITaskProgressCallback", false, loader);
            bridge.getMethod("startTask", String.class, String.class, String.class, Surface.class,
                    int.class, int.class, Bundle.class, callback, String.class, String.class, int.class);
            bridge.getMethod("isInitialized");
            Class<?> initializer = Class.forName("com.stepos.taskrobot.llmapi.stepfun.GuiAgentInitializer", false, loader);
            Class<?> account = Class.forName("f6.c", false, loader);
            account.getConstructor(Context.class);
            initializer.getMethod("initGuiAgent", Context.class, account);
            Class.forName("com.mdisdk.TaskManager", false, loader);
            Class.forName("stepai.aios.guiagent.config.RuntimeConfig", false, loader);
            result.put("classloader_available", true).put("start_task_abi", true)
                    .put("initializer_abi", true).put("account_constructor_abi", true)
                    .put("execution_verified", false).put("ok", true);
        } catch (Throwable error) {
            while (error instanceof InvocationTargetException && error.getCause() != null) error = error.getCause();
            try { result.put("ok", false).put("error", error.getClass().getSimpleName() + ": " + error.getMessage()); }
            catch (Exception ignored) { }
        }
        System.out.println(result.toString());
        System.out.flush();
        // Do not enter an Android event loop or leave an unregistered service behind.
        System.exit(0);
    }

    private static String readSmall(String path) throws Exception {
        try (InputStream input = new FileInputStream(path)) {
            byte[] bytes = new byte[256]; int count = input.read(bytes);
            return count <= 0 ? "" : new String(bytes, 0, count, StandardCharsets.UTF_8).replace("\u0000", "").trim();
        }
    }

    private static String apkHash() throws Exception {
        MessageDigest digest = MessageDigest.getInstance("SHA-256");
        try (InputStream input = new FileInputStream(APK)) {
            byte[] buffer = new byte[65536]; int count;
            while ((count = input.read(buffer)) != -1) digest.update(buffer, 0, count);
        }
        StringBuilder result = new StringBuilder();
        for (byte value : digest.digest()) result.append(String.format(Locale.ROOT, "%02x", value & 255));
        return result.toString();
    }
}

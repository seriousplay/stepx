package stepx;

import android.app.Application;
import android.content.pm.PackageInfo;
import android.content.pm.PackageManager;
import android.content.pm.Signature;
import android.net.LocalServerSocket;
import android.net.LocalSocket;
import android.os.Bundle;
import android.os.SystemClock;
import android.view.Surface;
import org.json.JSONArray;
import org.json.JSONObject;
import java.io.ByteArrayOutputStream;
import java.io.FileInputStream;
import java.io.InputStream;
import java.lang.reflect.Field;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.Collection;
import java.util.Locale;

/**
 * Read-only readiness probe inside one pinned original ability process.
 * It never calls init/startTask, changes a model, hooks ART, or reads account secrets.
 * Only the separately verified StepX companion UID can query or stop this socket.
 * The process owns all state; stopping it or restarting ability removes the probe.
 */
public final class NativeGuiAdapter {
    private static final String COMPANION = "com.stepx.companion.app";
    private static final String TARGET = "com.stepos.ability";
    private static final String APK_PATH = "/system/priv-app/ability/ability.apk";
    private static final String APK_SHA256 = "c9f56f3078df96d8d726422b1c622a1c19fe8c7dec8cadd8537d9a6ecf513ac4";
    private static final String BRIDGE_CLASS = "com.stepos.taskrobot.llmapi.stepfun.SystemSdkBridge";
    private static final String CALLBACK_CLASS = "com.stepos.taskrobot.common.ITaskProgressCallback";
    private static final String SOCKET = "stepx-native-gui-v1";
    private static final long MAX_IDLE_MS = 10 * 60_000L;
    private final Application app;
    private final String certificate;
    private final int uid;
    private final Object bridge;
    private final Class<?> bridgeType;
    private final Class<?> callbackType;
    private final LocalServerSocket server;
    private volatile boolean running = true;
    private volatile long lastRequest = SystemClock.elapsedRealtime();

    private NativeGuiAdapter(Application app, String certificate, int uid) throws Exception {
        this.app = app;
        this.certificate = certificate;
        this.uid = uid;
        verifyCompanion();
        if (!APK_PATH.equals(app.getApplicationInfo().sourceDir) || !APK_SHA256.equals(digestApk()))
            throw new SecurityException("Unsupported native ability APK");
        ClassLoader loader = app.getClassLoader();
        bridgeType = Class.forName(BRIDGE_CLASS, false, loader);
        callbackType = Class.forName(CALLBACK_CLASS, false, loader);
        if (!callbackType.isInterface() || !android.os.IInterface.class.isAssignableFrom(callbackType))
            throw new SecurityException("Native callback ABI mismatch");
        // Resolve exact entry points without invoking task execution or initialization.
        bridgeType.getMethod("startTask", String.class, String.class, String.class, Surface.class,
                int.class, int.class, Bundle.class, callbackType, String.class, String.class, int.class);
        bridgeType.getMethod("cancelTask", String.class, String.class);
        bridgeType.getMethod("clearTask", String.class, boolean.class, String.class);
        bridgeType.getMethod("registerProgressCallback", String.class, int.class, callbackType);
        bridgeType.getMethod("isInitialized");
        bridgeType.getMethod("getCurrentModelId");
        bridgeType.getMethod("getModelOptions");
        bridgeType.getMethod("listActiveTasks");
        bridge = bridgeType.getField("INSTANCE").get(null);
        server = new LocalServerSocket(SOCKET);
    }

    /** JNI entry point: (Ljava/lang/String;I)Ljava/lang/String;. */
    public static String run(String certificate, int uid) {
        NativeGuiAdapter adapter = null;
        try {
            if (certificate == null || !certificate.matches("[0-9a-f]{64}") || uid < 10000 || uid >= 100000)
                throw new SecurityException("Invalid companion identity");
            Class<?> at = Class.forName("android.app.ActivityThread");
            Application app = (Application) at.getMethod("currentApplication").invoke(null);
            if (app == null || !TARGET.equals(app.getPackageName()))
                throw new SecurityException("Wrong target process");
            adapter = new NativeGuiAdapter(app, certificate, uid);
            JSONObject initial = adapter.probe().put("installed", true).put("signature_verified", true);
            adapter.start();
            return initial.toString();
        } catch (Throwable error) {
            if (adapter != null) adapter.close();
            return failure(error).toString();
        }
    }

    private static String hex(byte[] bytes) {
        StringBuilder result = new StringBuilder();
        for (byte value : bytes) result.append(String.format(Locale.ROOT, "%02x", value & 255));
        return result.toString();
    }

    private static String digestApk() throws Exception {
        MessageDigest digest = MessageDigest.getInstance("SHA-256");
        byte[] buffer = new byte[64 * 1024];
        try (InputStream input = new FileInputStream(APK_PATH)) {
            int count;
            while ((count = input.read(buffer)) != -1) digest.update(buffer, 0, count);
        }
        return hex(digest.digest());
    }

    private void verifyCompanion() throws Exception {
        PackageManager pm = app.getPackageManager();
        PackageInfo info = pm.getPackageInfo(COMPANION, PackageManager.GET_SIGNING_CERTIFICATES);
        if (info.applicationInfo == null || info.applicationInfo.uid != uid || info.signingInfo == null)
            throw new SecurityException("Companion UID mismatch");
        String[] packages = pm.getPackagesForUid(uid);
        if (packages == null || packages.length != 1 || !COMPANION.equals(packages[0]))
            throw new SecurityException("Shared companion UID is not allowed");
        Signature[] signers = info.signingInfo.getApkContentsSigners();
        if (signers.length != 1 || !certificate.equals(hex(MessageDigest.getInstance("SHA-256").digest(signers[0].toByteArray()))))
            throw new SecurityException("Companion certificate mismatch");
    }

    private Object read(String method) throws Exception {
        return bridgeType.getMethod(method).invoke(bridge);
    }

    private JSONObject probe() throws Exception {
        boolean initialized = Boolean.TRUE.equals(read("isInitialized"));
        JSONArray models = new JSONArray();
        Object options = read("getModelOptions");
        if (options instanceof Collection) {
            for (Object value : (Collection<?>) options) {
                if (models.length() >= 64) break;
                // Only catalog identifiers; never serialize model configuration or credentials.
                if (value instanceof String) models.put(value);
            }
        }
        boolean runtimeInitialized = false;
        boolean llmConfigPresent = false;
        String runtimeProbe = "not_present";
        try {
            Class<?> runtime = Class.forName("ua.a", false, app.getClassLoader());
            Field field = runtime.getDeclaredField("c");
            field.setAccessible(true);
            Object runtimeInstance = field.get(null);
            runtimeInitialized = runtimeInstance != null;
            if (runtimeInstance != null) {
                // Read only presence; never expose the configuration, service URL, or tokens.
                for (Field candidate : runtimeInstance.getClass().getDeclaredFields()) {
                    if (!candidate.getType().getName().equals("stepai.aios.guiagent.config.RuntimeConfig")) continue;
                    candidate.setAccessible(true);
                    Object config = candidate.get(runtimeInstance);
                    llmConfigPresent = config != null && config.getClass().getMethod("getLlmConfig").invoke(config) != null;
                }
            }
            runtimeProbe = "available";
        } catch (ClassNotFoundException | NoSuchFieldException missing) {
            runtimeProbe = "abi_mismatch";
        }
        JSONArray callbacks = new JSONArray();
        for (Method method : callbackType.getMethods()) {
            if (method.getDeclaringClass() == Object.class) continue;
            JSONObject item = new JSONObject().put("name", method.getName()).put("returns", method.getReturnType().getName());
            JSONArray arguments = new JSONArray();
            for (Class<?> argument : method.getParameterTypes()) arguments.put(argument.getName());
            callbacks.put(item.put("parameters", arguments));
        }
        Object active = read("listActiveTasks");
        int activeCount = active instanceof Collection ? ((Collection<?>) active).size() : -1;
        return new JSONObject().put("ok", true).put("version", 1).put("socket", SOCKET)
                .put("target", TARGET).put("pid", android.os.Process.myPid()).put("uid", uid)
                .put("running", running).put("mode", "inspect_only").put("execution_enabled", false)
                .put("native_entry_blocked", true).put("apk_sha256", APK_SHA256)
                .put("sdk_initialized", initialized).put("runtime_initialized", runtimeInitialized)
                .put("llm_config_present", llmConfigPresent)
                .put("runtime_probe", runtimeProbe).put("start_task_abi", true)
                .put("active_task_count", activeCount).put("current_model", read("getCurrentModelId"))
                .put("model_ids", models).put("model_permission_verified", false)
                .put("callbacks", callbacks).put("idle_timeout_ms", MAX_IDLE_MS);
    }

    private JSONObject handle(JSONObject request) throws Exception {
        String op = request.getString("op");
        if (op.equals("shutdown")) {
            running = false;
            return new JSONObject().put("ok", true).put("version", 1).put("running", false)
                    .put("execution_enabled", false).put("rollback", "process_adapter_closed");
        }
        if (!op.equals("status") && !op.equals("native_sdk_probe"))
            throw new IllegalArgumentException("Inspect-only adapter: unsupported operation");
        return probe();
    }

    private static JSONObject failure(Throwable error) {
        while (error instanceof InvocationTargetException && error.getCause() != null) error = error.getCause();
        try {
            return new JSONObject().put("ok", false).put("version", 1)
                    .put("error", error.getClass().getSimpleName() + ": " + String.valueOf(error.getMessage()));
        } catch (Exception impossible) { return new JSONObject(); }
    }

    private void close() {
        running = false;
        try { server.close(); } catch (Exception ignored) { }
    }

    private void start() {
        Thread watchdog = new Thread(() -> {
            while (running) {
                if (SystemClock.elapsedRealtime() - lastRequest > MAX_IDLE_MS) { close(); break; }
                try { Thread.sleep(1000); } catch (InterruptedException interrupted) { close(); break; }
            }
        }, "StepXNativeGuiProbeWatchdog");
        watchdog.setDaemon(true);
        watchdog.start();
        Thread listener = new Thread(() -> {
            try {
                while (running) {
                    try (LocalSocket client = server.accept()) {
                        client.setSoTimeout(2000);
                        if (client.getPeerCredentials().getUid() != uid) continue;
                        verifyCompanion();
                        lastRequest = SystemClock.elapsedRealtime();
                        JSONObject response;
                        try {
                            ByteArrayOutputStream bytes = new ByteArrayOutputStream();
                            InputStream input = client.getInputStream();
                            int next;
                            while ((next = input.read()) != -1 && next != '\n') {
                                if (bytes.size() >= 1024) throw new IllegalArgumentException("Request too large");
                                bytes.write(next);
                            }
                            response = handle(new JSONObject(bytes.toString("UTF-8")));
                        } catch (Throwable error) { response = failure(error); }
                        client.getOutputStream().write((response.toString() + "\n").getBytes(StandardCharsets.UTF_8));
                        client.getOutputStream().flush();
                    } catch (Throwable error) {
                        // Reject a bad client without exposing another control or reopening the server.
                    }
                }
            } finally { close(); }
        }, "StepXNativeGuiProbeListener");
        listener.setDaemon(true);
        listener.start();
    }
}

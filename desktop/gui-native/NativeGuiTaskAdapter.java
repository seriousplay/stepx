package stepx;

import android.app.Application;
import android.app.KeyguardManager;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.pm.PackageInfo;
import android.content.pm.PackageManager;
import android.content.pm.Signature;
import android.net.LocalServerSocket;
import android.net.LocalSocket;
import android.os.Binder;
import android.os.IBinder;
import android.os.Parcel;
import android.os.RemoteException;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.os.PowerManager;
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
import java.lang.reflect.Proxy;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Collection;
import java.util.Locale;
import java.util.UUID;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;

/** Fixed, supervised Settings/About test through the original GUI SDK. No arbitrary task interface. */
public final class NativeGuiTaskAdapter {
    private static final String COMPANION = "com.stepx.companion.app";
    private static final String TARGET = "com.stepos.ability";
    private static final String APK = "/system/priv-app/ability/ability.apk";
    private static final String SHA = "c9f56f3078df96d8d726422b1c622a1c19fe8c7dec8cadd8537d9a6ecf513ac4";
    private static final String SOCKET = "stepx-native-gui-test-v1";
    private static final String BRIDGE = "com.stepos.taskrobot.llmapi.stepfun.SystemSdkBridge";
    private static final String CALLBACK = "com.stepos.taskrobot.common.ITaskProgressCallback";
    private static final long DEADLINE_MS = 90_000L;
    private static final long IDLE_MS = 10 * 60_000L;
    private static final String TEST_TASK = "打开系统设置，进入关于手机，查看并报告 Android 版本号。只查看信息，不要修改任何设置，不要连续点击版本号，不要打开其他应用。如果需要密码、登录或确认操作，请立即停止并说明。";
    private static volatile NativeGuiTaskAdapter hostedAdapter;
    private final Application app;
    private final String certificate;
    private final int uid;
    private final Object bridge;
    private final Class<?> bridgeType;
    private final Class<?> callbackType;
    private final Object manager;
    private final Method startTask;
    private final Handler main = new Handler(Looper.getMainLooper());
    private final LocalServerSocket server;
    private volatile boolean running = true;
    private volatile boolean shutdownRequested;
    private volatile long lastRequest = SystemClock.elapsedRealtime();
    private volatile TestState current;

    private static final class TestState {
        final String id = "stepx-settings-about-" + UUID.randomUUID().toString();
        final long started = SystemClock.elapsedRealtime();
        final ArrayList<JSONObject> events = new ArrayList<>();
        volatile String phase = "queued";
        volatile String terminal = "";
        volatile String reason = "";
        volatile boolean invocationStarted;
        volatile boolean stopRequested;
        volatile boolean cleaned;
        volatile boolean hasTaskAfterCleanup;
        volatile boolean mdiTaskAfterCleanup;
        volatile JSONObject resourceSnapshot;
        volatile int ownedDisplayId = -1;
        volatile boolean ownedDisplayPresent;
        volatile boolean clipboardRestored;
        volatile boolean clipboardPrepared;
        volatile int actionEvents;
        ClipData savedClipboard;
        ClipboardManager clipboard;
        ClipboardManager.OnPrimaryClipChangedListener clipboardListener;
        boolean sawNativeClear;
        boolean clipboardChangedAfterClear;
        Object callback;
    }

    private NativeGuiTaskAdapter(Application app, String certificate, int uid) throws Exception {
        this.app = app; this.certificate = certificate; this.uid = uid;
        verifyCompanion();
        if (!APK.equals(app.getApplicationInfo().sourceDir) || !SHA.equals(digestApk()))
            throw new SecurityException("Unsupported native ability APK");
        ClassLoader loader = app.getClassLoader();
        bridgeType = Class.forName(BRIDGE, false, loader);
        callbackType = Class.forName(CALLBACK, false, loader);
        if (!callbackType.isInterface() || !android.os.IInterface.class.isAssignableFrom(callbackType))
            throw new SecurityException("Unsupported callback ABI");
        bridge = bridgeType.getField("INSTANCE").get(null);
        startTask = bridgeType.getMethod("startTask", String.class, String.class, String.class,
                Surface.class, int.class, int.class, Bundle.class, callbackType, String.class, String.class, int.class);
        bridgeType.getMethod("cancelTask", String.class, String.class);
        bridgeType.getMethod("clearTask", String.class, boolean.class, String.class);
        bridgeType.getMethod("hasTask", String.class);
        Class<?> managerType = Class.forName("com.stepos.taskrobot.service.TaskRobotManagerImpl", false, loader);
        managerType.getMethod("pauseTask", String.class);
        managerType.getMethod("resumeTask", String.class);
        // The no-arg constructor creates only the original main-thread handler/coroutine job.
        // Do not use the Context overload, which performs additional native initialization.
        manager = managerType.getConstructor().newInstance();
        server = new LocalServerSocket(SOCKET);
    }

    /** Installs only this process endpoint and returns readiness; never starts a test. */
    public static String run(String certificate, int uid) {
        NativeGuiTaskAdapter adapter = null;
        try {
            if (certificate == null || !certificate.matches("[0-9a-f]{64}") || uid < 10000 || uid >= 100000)
                throw new SecurityException("Invalid companion identity");
            Application app = (Application) Class.forName("android.app.ActivityThread")
                    .getMethod("currentApplication").invoke(null);
            if (app == null || !TARGET.equals(app.getPackageName())) throw new SecurityException("Wrong target process");
            adapter = new NativeGuiTaskAdapter(app, certificate, uid);
            JSONObject result = adapter.status().put("installed", true).put("signature_verified", true);
            adapter.listen();
            return result.toString();
        } catch (Throwable error) {
            if (adapter != null) adapter.close();
            return failure(error).toString();
        }
    }

    /** Dedicated standalone host entry. The supplied host retains the real package identity. */
    public static synchronized String startInHost(Application app, String certificate, int uid) {
        NativeGuiTaskAdapter adapter = null;
        try {
            if (hostedAdapter != null) throw new IllegalStateException("Hosted endpoint already created");
            if (certificate == null || !certificate.matches("[0-9a-f]{64}") || uid < 10000 || uid >= 100000)
                throw new SecurityException("Invalid companion identity");
            if (app == null || !TARGET.equals(app.getPackageName()) || android.os.Process.myUid() != 1000)
                throw new SecurityException("Invalid standalone host context");
            adapter = new NativeGuiTaskAdapter(app, certificate, uid);
            JSONObject status = adapter.status().put("installed", true).put("signature_verified", true)
                    .put("standalone_host", true).put("tasks_started_automatically", 0);
            hostedAdapter = adapter;
            adapter.listen();
            return status.toString();
        } catch (Throwable error) {
            if (adapter != null) adapter.close();
            return failure(error).toString();
        }
    }

    public static String peekStatus() {
        try {
            NativeGuiTaskAdapter adapter = hostedAdapter;
            if (adapter == null) return new JSONObject().put("ok", false).put("installed", false).toString();
            return adapter.status().toString();
        } catch (Throwable error) { return failure(error).toString(); }
    }

    /** Requests normal cancellation and exact owned-task cleanup; never kills a process. */
    public static String requestShutdown() {
        try {
            NativeGuiTaskAdapter adapter = hostedAdapter;
            if (adapter == null) return new JSONObject().put("ok", true).put("installed", false).toString();
            return adapter.shutdown().toString();
        } catch (Throwable error) { return failure(error).toString(); }
    }

    /** Standard Binder control for the signed companion; no SELinux relaxation. */
    public static IBinder controlBinder() {
        final NativeGuiTaskAdapter adapter = hostedAdapter;
        if (adapter == null) throw new IllegalStateException("Host endpoint is absent");
        return new Binder() {
            { attachInterface(null, "stepx.NativeGuiTaskControl.v1"); }
            @Override protected boolean onTransact(int code, Parcel data, Parcel reply, int flags)
                    throws RemoteException {
                if (code != IBinder.FIRST_CALL_TRANSACTION) return super.onTransact(code, data, reply, flags);
                if ((flags & IBinder.FLAG_ONEWAY) != 0 || reply == null || data.dataSize() > 1024)
                    throw new SecurityException("Bounded synchronous control required");
                if (Binder.getCallingUid() != adapter.uid) throw new SecurityException("Signed companion UID required");
                data.enforceInterface("stepx.NativeGuiTaskControl.v1");
                String operation = data.readString(); data.enforceNoDataAvail();
                if (operation == null || operation.length() > 24) throw new SecurityException("Invalid operation");
                long identity = Binder.clearCallingIdentity();
                try {
                    adapter.verifyCompanion(); adapter.lastRequest = SystemClock.elapsedRealtime();
                    JSONObject response;
                    try { response = adapter.handle(new JSONObject().put("op", operation)); }
                    catch (Throwable error) { response = failure(error); }
                    reply.writeNoException(); reply.writeString(response.toString()); return true;
                } catch (Exception error) { throw new SecurityException("Companion identity verification failed"); }
                finally { Binder.restoreCallingIdentity(identity); }
            }
        };
    }

    private static String hex(byte[] bytes) {
        StringBuilder result = new StringBuilder();
        for (byte value : bytes) result.append(String.format(Locale.ROOT, "%02x", value & 255));
        return result.toString();
    }

    private static String digestApk() throws Exception {
        MessageDigest digest = MessageDigest.getInstance("SHA-256");
        try (InputStream input = new FileInputStream(APK)) {
            byte[] bytes = new byte[65536]; int count;
            while ((count = input.read(bytes)) != -1) digest.update(bytes, 0, count);
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
            throw new SecurityException("Shared UID rejected");
        Signature[] signers = info.signingInfo.getApkContentsSigners();
        if (signers.length != 1 || !certificate.equals(hex(MessageDigest.getInstance("SHA-256").digest(signers[0].toByteArray()))))
            throw new SecurityException("Companion certificate mismatch");
    }

    private Object call(String method) throws Exception { return bridgeType.getMethod(method).invoke(bridge); }
    private boolean hasTask(TestState state) throws Exception {
        return Boolean.TRUE.equals(bridgeType.getMethod("hasTask", String.class).invoke(bridge, state.id));
    }
    private int activeCount() throws Exception {
        Object tasks = call("listActiveTasks");
        if (!(tasks instanceof Collection)) throw new IllegalStateException("Native task list ABI mismatch");
        return ((Collection<?>) tasks).size();
    }

    private JSONObject readiness() throws Exception {
        boolean runtimeReady = false, llmReady = false;
        Class<?> runtimeClass = Class.forName("ua.a", false, app.getClassLoader());
        Field runtimeField = runtimeClass.getDeclaredField("c"); runtimeField.setAccessible(true);
        Object runtime = runtimeField.get(null);
        runtimeReady = runtime != null;
        if (runtime != null) {
            for (Field field : runtime.getClass().getDeclaredFields()) {
                if (!field.getType().getName().equals("stepai.aios.guiagent.config.RuntimeConfig")) continue;
                field.setAccessible(true);
                Object config = field.get(runtime);
                llmReady = config != null && config.getClass().getMethod("getLlmConfig").invoke(config) != null;
            }
        }
        PowerManager power = (PowerManager) app.getSystemService(Application.POWER_SERVICE);
        KeyguardManager keyguard = (KeyguardManager) app.getSystemService(Application.KEYGUARD_SERVICE);
        return new JSONObject().put("sdk_initialized", Boolean.TRUE.equals(call("isInitialized")))
                .put("runtime_initialized", runtimeReady).put("llm_config_present", llmReady)
                .put("interactive", power != null && power.isInteractive())
                .put("locked", keyguard == null || keyguard.isKeyguardLocked() || keyguard.isDeviceLocked())
                .put("active_task_count", activeCount()).put("current_model", call("getCurrentModelId"));
    }

    private JSONObject status() throws Exception {
        JSONObject result = readiness().put("ok", true).put("version", 1).put("socket", SOCKET)
                .put("target", TARGET).put("pid", android.os.Process.myPid()).put("uid", uid)
                .put("running", running).put("mode", "fixed_settings_about_test")
                .put("arbitrary_tasks_enabled", false).put("deadline_ms", DEADLINE_MS)
                .put("shutdown_requested", shutdownRequested).put("apk_sha256", SHA);
        result.put("cleanup_scope", "adapter_owned_sdk_task_map");
        TestState state = current;
        if (state != null) {
            JSONArray events = new JSONArray();
            synchronized (state.events) { for (JSONObject event : state.events) events.put(event); }
            result.put("test", new JSONObject().put("id", state.id).put("phase", state.phase)
                    .put("terminal", state.terminal).put("reason", state.reason)
                    .put("elapsed_ms", SystemClock.elapsedRealtime() - state.started)
                    .put("stop_requested", state.stopRequested).put("invocation_started", state.invocationStarted)
                    .put("native_task_present", hasTask(state)).put("cleaned", state.cleaned)
                    .put("has_task_after_cleanup", state.hasTaskAfterCleanup)
                    .put("mdi_task_after_cleanup", state.mdiTaskAfterCleanup)
                    .put("resource_snapshot", state.resourceSnapshot == null ? JSONObject.NULL : state.resourceSnapshot)
                    .put("owned_display_id", state.ownedDisplayId).put("owned_display_present", state.ownedDisplayPresent)
                    .put("clipboard_restored", state.clipboardRestored).put("clipboard_restore_pending", state.clipboardPrepared)
                    .put("action_events", state.actionEvents).put("events", events));
        }
        return result;
    }

    private void event(TestState state, String kind, String detail) {
        try {
            JSONObject event = new JSONObject().put("ms", SystemClock.elapsedRealtime() - state.started)
                    .put("event", kind).put("detail", diagnosticDetail(detail));
            synchronized (state.events) {
                // Keep the full UTF-8 status below the companion provider's 32 KiB cap.
                if (state.events.size() >= 24) state.events.remove(0);
                state.events.add(event);
            }
        } catch (Exception ignored) { }
    }
    private static String clip(String value, int max) { return value == null ? "" : value.substring(0, Math.min(value.length(), max)); }

    private static String diagnosticDetail(String value) {
        if (value == null) return "";
        // Screenshot callbacks carry signed object-store URLs. Keep credentials
        // and attachment links out of control status and research event logs.
        String safe = value.replaceAll("https?://[^\\s\\\"<>]+", "[attachment]");
        if (safe.startsWith("{")) {
            try {
                JSONObject source = new JSONObject(safe), selected = new JSONObject();
                java.util.Iterator<String> keys = source.keys();
                while (keys.hasNext()) {
                    String key = keys.next(); Object item = source.opt(key);
                    if (key.toLowerCase(java.util.Locale.ROOT).matches(".*(error|reason|status|state|message|summary).*" )
                            && (item instanceof String || item instanceof Number || item instanceof Boolean))
                        selected.put(key, item instanceof String ? clip((String) item, 160) : item);
                }
                safe = selected.length() > 0 ? selected.toString() : "Native task record received";
            } catch (Exception ignored) { safe = "Native task event received"; }
        }
        if (safe.toLowerCase(java.util.Locale.ROOT).matches(".*(authorization|access_token|refresh_token|x-tos-credential|password|secret).*"))
            return "Native event details omitted";
        return clip(safe, 256);
    }

    private static boolean isEmptyClip(ClipData clip) {
        if (clip == null || clip.getItemCount() == 0) return true;
        if (clip.getItemCount() != 1) return false;
        ClipData.Item item = clip.getItemAt(0);
        CharSequence text = item.getText();
        return item.getUri() == null && item.getIntent() == null && (text == null || text.length() == 0);
    }

    private void prepareClipboard(TestState state) {
        state.clipboard = (ClipboardManager) app.getSystemService(Application.CLIPBOARD_SERVICE);
        if (state.clipboard == null) throw new IllegalStateException("Clipboard preservation unavailable");
        boolean hadClip = state.clipboard.hasPrimaryClip();
        state.savedClipboard = state.clipboard.getPrimaryClip();
        if (hadClip && state.savedClipboard == null) throw new IllegalStateException("Cannot preserve clipboard");
        state.clipboardListener = () -> {
            try {
                boolean empty = isEmptyClip(state.clipboard.getPrimaryClip());
                if (!state.sawNativeClear && empty && SystemClock.elapsedRealtime() - state.started <= 15_000L)
                    state.sawNativeClear = true;
                else if (state.sawNativeClear && !empty) state.clipboardChangedAfterClear = true;
            } catch (Throwable error) { state.clipboardChangedAfterClear = true; }
        };
        state.clipboard.addPrimaryClipChangedListener(state.clipboardListener);
        state.clipboardPrepared = true;
    }

    private void restoreClipboard(TestState state) {
        if (!state.clipboardPrepared) return;
        try {
            state.clipboard.removePrimaryClipChangedListener(state.clipboardListener);
            if (state.savedClipboard != null && state.sawNativeClear && !state.clipboardChangedAfterClear
                    && isEmptyClip(state.clipboard.getPrimaryClip())) {
                state.clipboard.setPrimaryClip(state.savedClipboard);
                state.clipboardRestored = true;
            }
        } catch (Throwable error) { event(state, "clipboard_restore_error", error.getClass().getSimpleName()); }
        finally { state.savedClipboard = null; state.clipboardListener = null; state.clipboardPrepared = false; }
    }

    private String verifiedParams() throws Exception {
        JSONObject params = new JSONObject().put("app_name", "设置").put("task", TEST_TASK).put("meta", new JSONObject());
        ClassLoader loader = app.getClassLoader();
        Class<?> parser = Class.forName("com.stepos.taskrobot.llmapi.stepfun.TaskParamsParser", false, loader);
        Object parsed = parser.getMethod("parse", String.class, String.class, String.class, String.class)
                .invoke(parser.getField("INSTANCE").get(null), params.toString(), TARGET, null, null);
        String name = (String) parsed.getClass().getMethod("getOpenAppName").invoke(parsed);
        String task = (String) parsed.getClass().getMethod("getTaskName").invoke(parsed);
        Class<?> utility = Class.forName("com.stepos.taskrobot.llmapi.stepfun.PackageUtil", false, loader);
        Object packageName = utility.getMethod("getPackageNameByAppName", android.content.Context.class, String.class)
                .invoke(utility.getField("INSTANCE").get(null), app, name);
        if (!"com.android.settings".equals(packageName) || !TEST_TASK.equals(task))
            throw new SecurityException("Fixed test does not resolve to Android Settings");
        return params.toString();
    }

    private synchronized JSONObject startTest() throws Exception {
        if (shutdownRequested) throw new IllegalStateException("Adapter is shutting down");
        if (hostedAdapter == this && current != null)
            throw new IllegalStateException("The isolated research host permits one fixed test only");
        if (current != null && (!current.cleaned || current.clipboardPrepared))
            throw new IllegalStateException("A test is active or cleanup is incomplete");
        JSONObject ready = readiness();
        if (!ready.getBoolean("sdk_initialized") || !ready.getBoolean("runtime_initialized") || !ready.getBoolean("llm_config_present"))
            throw new IllegalStateException("Original GUI SDK is not initialized; initialization is not modified");
        if (!ready.getBoolean("interactive") || ready.getBoolean("locked")) throw new IllegalStateException("Unlock the phone and keep its screen on");
        if (ready.getInt("active_task_count") != 0) throw new IllegalStateException("Another native GUI task is present");
        final String params = verifiedParams();
        final TestState state = new TestState();
        current = state;
        event(state, "queued", "settings_about");
        main.post(() -> {
            if (current != state || state.stopRequested) { cleanup(state, "cancelled_before_start"); return; }
            try {
                JSONObject fresh = readiness();
                if (!fresh.getBoolean("interactive") || fresh.getBoolean("locked") || fresh.getInt("active_task_count") != 0)
                    throw new IllegalStateException("Phone readiness changed before start");
                prepareClipboard(state);
                final Binder callbackBinder = new Binder();
                state.callback = Proxy.newProxyInstance(app.getClassLoader(), new Class<?>[]{callbackType}, (proxy, method, args) -> {
                    String methodName = method.getName();
                    if (methodName.equals("asBinder")) return callbackBinder;
                    if (methodName.equals("toString")) return "StepXFixedSettingsCallback";
                    if (methodName.equals("hashCode")) return System.identityHashCode(proxy);
                    if (methodName.equals("equals")) return proxy == args[0];
                    onCallback(state, methodName, args);
                    return null;
                });
                state.invocationStarted = true;
                state.phase = "starting";
                // Type 0 sends actions to display 0; the SDK still allocates an
                // MDI virtual display and redirect registration requiring cleanup.
                startTask.invoke(bridge, null, state.id, params, null, 0, 0, null, state.callback, null, null, android.os.Process.myPid());
                event(state, "start_returned", "native_task_present=" + hasTask(state));
                if (state.terminal.isEmpty() && !state.stopRequested) state.phase = "running";
            } catch (Throwable error) {
                state.terminal = "error"; state.reason = errorMessage(error);
                event(state, "start_error", state.reason);
                cleanup(state, "start_error");
            }
        });
        return status();
    }

    private boolean callbackOwner(TestState state, Object value) {
        if (value instanceof String) return state.id.equals(value);
        if (value == null) return false;
        try { return state.id.equals(value.getClass().getMethod("getTaskId").invoke(value)); }
        catch (Throwable ignored) { return false; }
    }

    private void onCallback(TestState state, String name, Object[] args) {
        if (current != state || args == null || args.length == 0) return;
        if (state.cleaned) {
            // Runtime cancellation can acknowledge after clearTask removed the map entry.
            if (name.equals("onTaskAction") && args.length > 5 && callbackOwner(state, args[0])) {
                String action = String.valueOf(args[2]);
                if (action.equals("finish") || action.equals("error") || action.equals("cancel")) {
                    event(state, "late_native_" + action, args[5] instanceof String ? (String) args[5] : "");
                    if (state.terminal.isEmpty()) state.terminal = action;
                }
            }
            return;
        }
        if (name.equals("onPackageConflict")) {
            // This callback uniquely has conflictKind first, then the new task ID.
            if (args.length > 1 && callbackOwner(state, args[1])) {
                event(state, "onPackageConflict", "Test does not displace another task");
                requestStop(state, "package_conflict");
            }
            return;
        }
        if (!callbackOwner(state, args[0])) return;
        if (name.equals("onTaskAction") || name.equals("onTaskActionPair")) {
            String action = args.length > 2 && args[2] instanceof String ? (String) args[2] : "";
            String message = args.length > 5 && args[5] instanceof String ? (String) args[5] : "";
            event(state, action, message);
            if (action.equals("finish") || action.equals("error") || action.equals("cancel") || action.equals("app_not_found")) {
                if (state.terminal.isEmpty()) state.terminal = action;
                state.phase = "finishing";
                main.postDelayed(() -> cleanup(state, "native_" + action), 500L);
            } else if (action.equals("pause") && message.equals("true")) state.phase = "paused";
            else if (action.equals("recover_from_pause") && message.equals("true")) state.phase = "running";
            else if (action.equals("llm_takeover") || action.equals("info")) requestStop(state, "user_intervention_required");
        } else if (name.equals("onAction")) {
            state.actionEvents++;
            event(state, "action_observed", "count=" + state.actionEvents);
        } else if (name.equals("onInfoRequested") || name.equals("onRequestRelaunch")) {
            event(state, name, "Test does not auto-confirm requests");
            requestStop(state, "user_intervention_required");
        } else if (name.equals("onTaskStatusChange")) {
            try { event(state, "native_status", String.valueOf(args[0].getClass().getMethod("getCurrentStatus").invoke(args[0]))); }
            catch (Throwable ignored) { }
        }
    }

    private void requestStop(TestState state, String reason) {
        if (current != state || state.cleaned || state.stopRequested) return;
        state.stopRequested = true; state.phase = "stopping"; state.reason = reason;
        event(state, "stop_requested", reason);
        main.post(() -> {
            try {
                if (state.invocationStarted && hasTask(state)) {
                    Object accepted = bridgeType.getMethod("cancelTask", String.class, String.class).invoke(bridge, state.id, "StepX " + reason);
                    event(state, "native_cancel_requested", String.valueOf(accepted));
                }
            } catch (Throwable error) { event(state, "cancel_error", errorMessage(error)); }
            main.postDelayed(() -> cleanup(state, reason), 1500L);
        });
    }

    /** Only the main-thread handler may release the exact task allocated by this adapter. */
    private void cleanup(TestState state, String reason) {
        if (current != state || state.cleaned) return;
        if (Looper.myLooper() != Looper.getMainLooper()) { main.post(() -> cleanup(state, reason)); return; }
        state.stopRequested = true;
        try {
            hasOwnedMdiTask(state); // Preserve the exact owned display identity before native removal.
            if (state.invocationStarted && hasTask(state)) {
                bridgeType.getMethod("cancelTask", String.class, String.class).invoke(bridge, state.id, "StepX " + reason);
                // Native clearTask also ends its shared KeepAliveHelper. Defer that
                // cleanup if another task appeared since the exclusive-start check.
                if (activeCount() > 1) {
                    state.phase = "cleanup_deferred";
                    event(state, "cleanup_deferred", "Another native task is present; shared keepalive is preserved");
                    return;
                }
                bridgeType.getMethod("clearTask", String.class, boolean.class, String.class).invoke(bridge, state.id, true, "StepX " + reason);
            }
            state.hasTaskAfterCleanup = hasTask(state);
            state.mdiTaskAfterCleanup = hasOwnedMdiTask(state);
            if (state.mdiTaskAfterCleanup) state.resourceSnapshot = NativeGuiTaskResources.snapshot(app, state.id);
            android.hardware.display.DisplayManager displays = app.getSystemService(android.hardware.display.DisplayManager.class);
            if (displays == null) throw new IllegalStateException("Display verification is unavailable");
            state.ownedDisplayPresent = state.ownedDisplayId >= 0 && displays.getDisplay(state.ownedDisplayId) != null;
            state.cleaned = !state.hasTaskAfterCleanup && !state.mdiTaskAfterCleanup && !state.ownedDisplayPresent;
            state.phase = state.cleaned ? "stopped" : "cleanup_pending";
            if (state.reason.isEmpty()) state.reason = reason;
            event(state, state.cleaned ? "cleanup_verified" : "cleanup_pending",
                    "native_task_present=" + state.hasTaskAfterCleanup + ",mdi_task_present=" + state.mdiTaskAfterCleanup);
        } catch (Throwable error) {
            state.phase = "cleanup_failed";
            event(state, "cleanup_error", errorMessage(error));
        }
        if (state.cleaned) {
            // The original SDK clears the clipboard in a child coroutine. Keep the
            // preservation listener briefly after release to catch a late native clear.
            main.postDelayed(() -> {
                restoreClipboard(state);
                if (shutdownRequested && state.cleaned) close();
            }, state.clipboardPrepared ? 1500L : 0L);
        } else {
            // Native release may suspend before finishing its MDI task. Keep the
            // queue alive and wait for that exact entry; do not clear a shared map.
            main.postDelayed(() -> cleanup(state, reason), 500L);
        }
    }

    private boolean hasOwnedMdiTask(TestState state) throws Exception {
        Class<?> type = Class.forName("com.mdisdk.TaskManager", false, app.getClassLoader());
        java.lang.reflect.Field field = type.getDeclaredField("activeTasks");
        field.setAccessible(true);
        Object value = field.get(null);
        if (!(value instanceof java.util.concurrent.ConcurrentHashMap))
            throw new SecurityException("Unsupported native MDI task registry");
        Object task = ((java.util.Map<?, ?>) value).get(state.id);
        if (task == null) return false;
        Class<?> taskType = Class.forName("com.mdisdk.Task", false, app.getClassLoader());
        if (!taskType.isInstance(task) || !state.id.equals(taskType.getMethod("getName").invoke(task)))
            throw new SecurityException("Owned MDI task identity mismatch");
        int displayId = (Integer) taskType.getMethod("getDisplayId").invoke(task);
        if (displayId < 90000000 || displayId >= 100000000
                || state.ownedDisplayId >= 0 && state.ownedDisplayId != displayId)
            throw new SecurityException("Owned MDI display identity mismatch");
        state.ownedDisplayId = displayId;
        return true;
    }

    private JSONObject control(String op) throws Exception {
        TestState state = current;
        if (state == null || state.cleaned) throw new IllegalStateException("No owned active test");
        if (op.equals("stop")) requestStop(state, "user_stop");
        else {
            if (state.stopRequested) throw new IllegalStateException("Task is stopping");
            String nativeMethod = op.equals("pause") ? "pauseTask" : "resumeTask";
            main.post(() -> {
                if (current != state || state.cleaned || state.stopRequested) return;
                try {
                    if (!hasTask(state)) throw new IllegalStateException("Owned native task is absent");
                    manager.getClass().getMethod(nativeMethod, String.class).invoke(manager, state.id);
                    event(state, op + "_requested", "Awaiting original SDK callback");
                } catch (Throwable error) { event(state, op + "_error", errorMessage(error)); }
            });
        }
        return status();
    }

    private JSONObject shutdown() throws Exception {
        shutdownRequested = true;
        TestState state = current;
        if (state == null || (state.cleaned && !state.clipboardPrepared)) {
            JSONObject result = status().put("running", false).put("rollback", "adapter_closed");
            close(); return result;
        }
        if (state.cleaned) return status().put("rollback", "clipboard_restore_pending");
        state.stopRequested = true;
        if (Looper.myLooper() == Looper.getMainLooper()) {
            cleanup(state, "adapter_shutdown");
            return status().put("rollback", state.cleaned
                    ? (state.clipboardPrepared ? "clipboard_restore_pending" : "owned_task_cleared") : "cleanup_pending");
        }
        CountDownLatch finished = new CountDownLatch(1);
        main.post(() -> { try { cleanup(state, "adapter_shutdown"); } finally { finished.countDown(); } });
        if (Looper.myLooper() != Looper.getMainLooper()) finished.await(2500L, TimeUnit.MILLISECONDS);
        return status().put("rollback", state.cleaned
                ? (state.clipboardPrepared ? "clipboard_restore_pending" : "owned_task_cleared") : "cleanup_pending");
    }

    private JSONObject handle(JSONObject request) throws Exception {
        String op = request.getString("op");
        if (request.length() != 1) throw new IllegalArgumentException("This fixed test accepts only an operation name");
        if (op.equals("status")) return status();
        if (op.equals("start_test")) return startTest();
        if (op.equals("pause") || op.equals("resume") || op.equals("stop")) return control(op);
        if (op.equals("shutdown")) return shutdown();
        throw new IllegalArgumentException("Unsupported fixed-test operation");
    }

    private static String errorMessage(Throwable error) {
        while (error instanceof InvocationTargetException && error.getCause() != null) error = error.getCause();
        return diagnosticDetail(error.getClass().getSimpleName() + ": " + String.valueOf(error.getMessage()));
    }
    private static JSONObject failure(Throwable error) {
        try { return new JSONObject().put("ok", false).put("version", 1).put("error", errorMessage(error)); }
        catch (Exception ignored) { return new JSONObject(); }
    }
    private void close() { running = false; try { server.close(); } catch (Exception ignored) { } }

    private void listen() {
        Thread watchdog = new Thread(() -> {
            while (running) {
                TestState state = current;
                if (state != null && !state.cleaned && SystemClock.elapsedRealtime() - state.started >= DEADLINE_MS)
                    requestStop(state, "deadline_90s");
                if (state != null && state.stopRequested && !state.cleaned && SystemClock.elapsedRealtime() - state.started > DEADLINE_MS + 5000L)
                    main.post(() -> cleanup(state, "deadline_cleanup_retry"));
                if (SystemClock.elapsedRealtime() - lastRequest > IDLE_MS) {
                    shutdownRequested = true;
                    if (state == null || (state.cleaned && !state.clipboardPrepared)) close();
                    else if (!state.cleaned) main.post(() -> cleanup(state, "idle_shutdown"));
                }
                try { Thread.sleep(1000L); } catch (InterruptedException ignored) { break; }
            }
        }, "StepXNativeGuiTestWatchdog");
        watchdog.setDaemon(true); watchdog.start();
        Thread listener = new Thread(() -> {
            try {
                while (running) {
                    try (LocalSocket client = server.accept()) {
                        client.setSoTimeout(3000);
                        if (client.getPeerCredentials().getUid() != uid) continue;
                        verifyCompanion(); lastRequest = SystemClock.elapsedRealtime();
                        JSONObject response;
                        try {
                            ByteArrayOutputStream bytes = new ByteArrayOutputStream();
                            int next; InputStream input = client.getInputStream();
                            while ((next = input.read()) != -1 && next != '\n') {
                                if (bytes.size() >= 1024) throw new IllegalArgumentException("Request too large");
                                bytes.write(next);
                            }
                            response = handle(new JSONObject(bytes.toString("UTF-8")));
                        } catch (Throwable error) { response = failure(error); }
                        client.getOutputStream().write((response.toString() + "\n").getBytes(StandardCharsets.UTF_8));
                        client.getOutputStream().flush();
                    } catch (Throwable ignored) { }
                }
            } finally {
                TestState state = current;
                if (state != null && !state.cleaned) main.post(() -> cleanup(state, "listener_closed"));
                close();
            }
        }, "StepXNativeGuiTestListener");
        listener.setDaemon(true); listener.start();
    }
}

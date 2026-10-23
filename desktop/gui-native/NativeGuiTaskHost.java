package stepx;

import android.app.Application;
import android.content.Context;
import android.content.ContentResolver;
import android.content.ContextParams;
import android.content.SharedPreferences;
import android.content.pm.ApplicationInfo;
import android.content.pm.PackageManager;
import android.content.res.Configuration;
import android.database.DatabaseErrorHandler;
import android.database.sqlite.SQLiteDatabase;
import android.os.Handler;
import android.os.Bundle;
import android.os.Looper;
import android.os.SystemClock;
import android.os.UserHandle;
import android.system.Os;
import android.view.Display;
import org.json.JSONObject;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.lang.reflect.Field;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Proxy;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.Locale;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.atomic.AtomicBoolean;

/**
 * RESEARCH ONLY: fixed Settings/About test in a disposable original-SDK host.
 * Not included in the production installer or companion UI. Gate stays closed
 * until the parent task has reviewed the code and device baseline.
 *
 * This is storage redirection for reviewed Context calls, not a sandbox. Original
 * framework/Binder operations and model network traffic remain real. The original
 * APK, Application/onCreate, TaskRobotInit, account configuration, shared task DB,
 * SQLCipher key, and SELinux policy are not modified. Native task-memory writes
 * use a research-only in-memory Room database; they are not persisted/synced.
 *
 * Must verify on the pinned phone before release: preexisting Omni allowlist;
 * no other native tasks; unlock/interactive state; real account and model access;
 * native taps and version result; pause/resume/cancel; original task/MDI release,
 * redirect counter baseline, no remaining virtual display, original process
 * identity unchanged, and owned directory cleanup. Local cleanup is not proof of
 * global rollback. An overdue/deadlocked cleanup preserves its owned directory.
 *
 * ApiManager.init may connect BFF through its network-availability callback even
 * before start_test. BFF uses the original OAID provider and user account route.
 * The OAID provider may restore/create an identifier if missing; this host does
 * not change its settings, manufacture identifiers, or output any identifier.
 * This initialization is not a read-only readiness probe.
 *
 * The SDK can have task-memory callbacks after task release. Its in-memory DB
 * reference is deliberately retained until process exit, even after close, so a
 * late callback cannot fall through to the original SQLCipher/Keystore builder.
 * Database/preferences APIs remain fail-closed. No original trace uploader is
 * installed, and token-bearing native debug output must already be disabled.
 */
public final class NativeGuiTaskHost {
    private static final boolean TASK_HOST_AUDITED = true;
    private static final String TARGET = "com.stepos.ability";
    private static final String COMPANION = "com.stepx.companion.app";
    private static final String APK = "/system/priv-app/ability/ability.apk";
    private static final String SHA = "c9f56f3078df96d8d726422b1c622a1c19fe8c7dec8cadd8537d9a6ecf513ac4";
    private static final String SOCKET = "stepx-native-gui-test-v1";
    private static final String PREFIX = "stepx-gui-task-host-";
    private static final String DB_CLASS = "com.stepos.taskrobot.taskmemory.AppTaskDatabase";
    private static final String API_CLASS = "com.stepos.agentframework.adapter.ApiManager";
    private static final JSONObject result = new JSONObject();
    private static final AtomicBoolean finished = new AtomicBoolean();
    private static final AtomicBoolean emitted = new AtomicBoolean();
    private static final long started = SystemClock.elapsedRealtime();
    private static ClassLoader loader;
    private static HostApplication host;
    private static Object bridge;
    private static android.os.IBinder companionController;
    private static boolean controllerPublished;
    private static Object api;
    private static Object memoryDb;
    private static Field memoryInstance;
    private static Field applicationField;
    private static boolean attemptedInitialization;
    private static boolean attemptedApiInitialization;
    private static boolean adapterStarted;
    private static long settledSince;
    private static Throwable monitorFailure;

    public static void main(String[] args) {
        try {
            result.put("mode", "isolated_native_gui_fixed_test").put("version", 1)
                    .put("pid", android.os.Process.myPid()).put("host_uid", android.os.Process.myUid())
                    .put("target_package", TARGET).put("socket", SOCKET).put("apk_sha256", SHA)
                    .put("tasks_started_automatically", 0).put("production_ready", false)
                    .put("audit_gate_open", TASK_HOST_AUDITED).put("global_rollback_verified", false);
            if (args.length != 3 || !"--fixed-settings-test".equals(args[0])
                    || !args[1].matches("[0-9a-f]{64}") || !args[2].matches("[0-9]{5}"))
                throw new IllegalArgumentException("Exact fixed-test arguments required");
            int uid = Integer.parseInt(args[2]);
            if (uid < 10000 || uid >= 100000) throw new SecurityException("Invalid companion UID");
            result.put("companion_uid", uid);
            if (!TASK_HOST_AUDITED) throw new SecurityException("Fixed native task host audit gate is closed");
            if (android.os.Process.myUid() != 1000 || !"u:r:system_app:s0".equals(readSmall("/proc/self/attr/current")))
                throw new SecurityException("Unexpected service identity");
            if (!SHA.equals(apkHash())) throw new SecurityException("Unsupported original APK");
            boolean debug = (Boolean) Class.forName("android.os.SystemProperties")
                    .getMethod("getBoolean", String.class, boolean.class).invoke(null, "debug.zyql.log", false);
            if (debug) throw new SecurityException("Original task debug logging must already be disabled");
            result.put("native_sensitive_debug_disabled", true);
            if (Looper.getMainLooper() == null) Looper.prepareMainLooper();
            Class<?> activityThread = Class.forName("android.app.ActivityThread");
            Object thread = activityThread.getMethod("systemMain").invoke(null);
            Context system = (Context) activityThread.getMethod("getSystemContext").invoke(thread);
            Context context = system.createPackageContext(TARGET, Context.CONTEXT_INCLUDE_CODE | Context.CONTEXT_IGNORE_SECURITY);
            ApplicationInfo info = context.getApplicationInfo();
            if (!TARGET.equals(context.getPackageName()) || !APK.equals(info.sourceDir) || info.uid != 1000)
                throw new SecurityException("Real package identity mismatch");
            for (String permission : new String[] { "com.stepos.ability.permission.SCREEN_OPS", "com.stepos.ability.permission.ACCESS_AGENT_SERVICE" })
                if (context.checkSelfPermission(permission) != PackageManager.PERMISSION_GRANTED)
                    throw new SecurityException("Required original package permission absent");
            verifyCompanion(context, args[1], uid);
            result.put("companion_signature_verified", true);
            try {
                context.getPackageManager().getPackageInfo("stepai.android.mindx.sdk", 0);
                throw new SecurityException("Unreviewed legacy model service present");
            } catch (PackageManager.NameNotFoundException expected) {
                result.put("legacy_model_service_absent", true);
            }
            host = new HostApplication(context);
            host.providers = new NativeGuiContentResolver(host, args[1], uid);
            loader = context.getClassLoader();
            result.put("storage_root", host.root.getAbsolutePath()).put("storage_sandbox", false)
                    .put("original_application_started", false).put("memory_persistence", "research_in_memory_only");
            startDeadline();
            result.put("phase", "memory_database");
            initializeMemoryDatabase();
            result.put("phase", "cloud_logger");
            Object logger = quietLogger();
            // The BFF websocket logger is separate from ApiManager's own logger.
            Class.forName("com.stepos.agentframework.adapter.bff.LogUtilKt", true, loader)
                    .getMethod("setLogger", Class.forName("com.stepos.agentframework.adapter.netcore.ILogger", false, loader))
                    .invoke(null, logger);
            result.put("host_bff_logger_suppressed", true);
            result.put("phase", "local_account");
            Class<?> authenticator = Class.forName("com.stepos.account.oauth.Authenticator", false, loader);
            Object accountInspector = authenticator.getConstructor(Context.class, String.class).newInstance(host, "basic");
            boolean loggedIn = authenticator.getMethod("getLocalAccount").invoke(accountInspector) != null;
            result.put("local_account_present", loggedIn);
            if (!loggedIn) throw new IllegalStateException("Original local account is absent");
            Class<?> accountType = Class.forName("f6.c", false, loader);
            Object account = accountType.getConstructor(Context.class).newInstance(host);
            Class<?> initializer = Class.forName("com.stepos.taskrobot.llmapi.stepfun.GuiAgentInitializer", false, loader);
            result.put("phase", "native_gui_initialization");
            // This disposable host uses the model cell's original default prompt.
            // Custom prompt preferences belong to the original app and stay closed.
            Class<?> overrides = Class.forName("stepai.aios.guiagent.llm.config.PromptOverrideManager", true, loader);
            Object prompts = overrides.getField("INSTANCE").get(null);
            overrides.getMethod("setEnabled", boolean.class).invoke(prompts, false);
            if (!Boolean.FALSE.equals(overrides.getMethod("getEnabled").invoke(prompts))
                    || !"stepx_default_probe".equals(overrides.getMethod("getEffectivePrompt", String.class, String.class)
                            .invoke(prompts, "stepx_readiness_probe", "stepx_default_probe")))
                throw new SecurityException("Original default prompt route is unavailable");
            result.put("isolated_prompt_overrides_disabled", true);
            attemptedInitialization = true;
            initializer.getMethod("initGuiAgent", Context.class, accountType)
                    .invoke(initializer.getField("INSTANCE").get(null), host, account);
            bridge = singleton("com.stepos.taskrobot.llmapi.stepfun.SystemSdkBridge");
            boolean sdk = Boolean.TRUE.equals(bridge.getClass().getMethod("isInitialized").invoke(bridge));
            if (!sdk || activeNativeTasks() != 0) throw new IllegalStateException("SDK is not exclusively idle");
            Class<?> mmkv = Class.forName("com.tencent.mmkv.MMKV", false, loader);
            String mmkvRoot = (String) mmkv.getMethod("getRootDir").invoke(null);
            if (mmkvRoot == null || !new File(host.getFilesDir(), "mmkv").getCanonicalPath().equals(new File(mmkvRoot).getCanonicalPath()))
                throw new SecurityException("MMKV storage escaped host");
            result.put("sdk_initialized", sdk).put("mmkv_root_isolated", true);
            result.put("phase", "original_device_identifier");
            // This is the original OAID route; only the returned non-null value is
            // passed in memory. ApiManager's pinned implementation does not use
            // this parameter; BFF also obtains OAID through the same native route.
            Object deviceId = Class.forName("N9.j", false, loader).getMethod("y", Context.class).invoke(null, host);
            if (!(deviceId instanceof String) || ((String) deviceId).isEmpty())
                throw new IllegalStateException("Original device identifier is unavailable");
            api = singleton(API_CLASS);
            Class<?> logType = Class.forName("com.stepos.agentframework.adapter.netcore.ILogger", false, loader);
            Class<?> uploadType = Class.forName("com.zyql.common.utils.upload.UploadConfig", false, loader);
            attemptedApiInitialization = true;
            result.put("phase", "cloud_client_initialization");
            api.getClass().getMethod("init", Context.class, String.class, long.class, logType, uploadType)
                    .invoke(api, host, (String) deviceId, 30L, logger, null);
            deviceId = null;
            result.put("cloud_client_initialized", true).put("model_access_verified", false);
            result.put("phase", "model_observation");
            NativeGuiModelDiagnostics.install(host);
            NativeGuiEventDiagnostics.install(host);
            result.put("phase", "companion_control_endpoint");
            JSONObject adapter = new JSONObject(NativeGuiTaskAdapter.startInHost(host, args[1], uid));
            if (!adapter.optBoolean("ok") || !adapter.optBoolean("installed") || !adapter.optBoolean("signature_verified"))
                throw new IllegalStateException("Signed companion endpoint failed to initialize");
            adapterStarted = true;
            companionController = NativeGuiTaskAdapter.controlBinder();
            Bundle controllerExtras = new Bundle(); controllerExtras.putBinder("controller", companionController);
            Bundle registered = host.providers.call(android.net.Uri.parse("content://com.stepx.companion.app.native_gui_task_probe"),
                    "register_binder", null, controllerExtras);
            if (registered == null || !registered.getBoolean("ok"))
                throw new IllegalStateException("Signed companion Binder registration failed");
            controllerPublished = true; result.put("companion_binder_published", true);
            result.put("phase", "ready");
            result.put("installed", true).put("active_task_count", activeNativeTasks()).put("event", "ready").put("ok", true);
            System.out.println(result.toString()); System.out.flush();
            new Handler(Looper.getMainLooper()).postDelayed(NativeGuiTaskHost::monitor, 500L);
            runTaskLoop();
        } catch (Throwable error) {
            if (adapterStarted) NativeGuiTaskAdapter.requestShutdown();
            finish(error);
        }
    }

    private static void runTaskLoop() {
        // Native actions also post work outside the startTask call. An exception
        // there must stop the owned task, but the queue must remain alive for
        // native cancellation, MDI disposal and clipboard restoration.
        while (!finished.get()) {
            try {
                Looper.loop();
                if (!finished.get()) finish(new IllegalStateException("Native main queue exited before cleanup"));
                return;
            } catch (Throwable error) {
                if (monitorFailure == null) monitorFailure = error;
                NativeGuiTaskAdapter.requestShutdown();
                if (error instanceof VirtualMachineError || error instanceof ThreadDeath) {
                    finish(error);
                    return;
                }
                // The original monitor is still queued. Do not add another
                // recurring monitor on every action failure.
            }
        }
    }

    private static void verifyCompanion(Context context, String cert, int uid) throws Exception {
        PackageManager pm = context.getPackageManager();
        android.content.pm.PackageInfo info = pm.getPackageInfo(COMPANION, PackageManager.GET_SIGNING_CERTIFICATES);
        String[] packages = pm.getPackagesForUid(uid);
        if (info.applicationInfo == null || info.applicationInfo.uid != uid || info.signingInfo == null
                || packages == null || packages.length != 1 || !COMPANION.equals(packages[0]))
            throw new SecurityException("Companion UID mismatch");
        android.content.pm.Signature[] signers = info.signingInfo.getApkContentsSigners();
        if (signers.length != 1 || !cert.equals(hex(MessageDigest.getInstance("SHA-256").digest(signers[0].toByteArray()))))
            throw new SecurityException("Companion signature mismatch");
    }

    private static Object quietLogger() throws Exception {
        // The pinned ILogger is a class, not a Java Proxy interface. Its original
        // BFF logger guards all four log methods with its per-instance DEBUG flag.
        // This changes only this host's loaded class, never the original process.
        Class<?> logs = Class.forName("com.stepos.agentframework.adapter.bff.LogUtilKt", true, loader);
        Object logger = logs.getMethod("getLogger").invoke(null);
        java.lang.reflect.Method setDebug = logger.getClass().getMethod("setDEBUG", boolean.class);
        java.lang.reflect.Method getDebug = logger.getClass().getMethod("getDEBUG");
        setDebug.setAccessible(true); getDebug.setAccessible(true);
        setDebug.invoke(logger, false);
        if (!Boolean.FALSE.equals(getDebug.invoke(logger)))
            throw new SecurityException("Native BFF logging could not be disabled");
        return logger;
    }

    private static void initializeMemoryDatabase() throws Exception {
        Class<?> dbType = Class.forName(DB_CLASS, true, loader);
        memoryInstance = dbType.getDeclaredField("INSTANCE"); memoryInstance.setAccessible(true);
        applicationField = Class.forName("S0.b", true, loader).getDeclaredField("p"); applicationField.setAccessible(true);
        if (memoryInstance.get(null) != null || applicationField.get(null) != null
                || !Application.class.isAssignableFrom(applicationField.getType()))
            throw new SecurityException("Original process memory globals are not fresh");
        Class<?> room = Class.forName("androidx.room.Room", false, loader);
        Object builder = room.getMethod("inMemoryDatabaseBuilder", Context.class, Class.class).invoke(null, host, dbType);
        memoryDb = builder.getClass().getMethod("build").invoke(builder);
        Class<?> roomDb = Class.forName("androidx.room.RoomDatabase", false, loader);
        Object helper = roomDb.getMethod("getOpenHelper").invoke(memoryDb);
        Class<?> helperType = Class.forName("androidx.sqlite.db.SupportSQLiteOpenHelper", false, loader);
        Object sqlite = helperType.getMethod("getWritableDatabase").invoke(helper);
        Object path = Class.forName("androidx.sqlite.db.SupportSQLiteDatabase", false, loader).getMethod("getPath").invoke(sqlite);
        if (!":memory:".equals(path)) throw new SecurityException("Research task database is not in memory");
        memoryInstance.set(null, memoryDb);
        applicationField.set(null, host);
        result.put("memory_db_open", true).put("memory_db_path_verified", true);
    }

    private static void monitor() {
        if (finished.get()) return;
        try {
            JSONObject status = new JSONObject(NativeGuiTaskAdapter.peekStatus());
            if (!status.optBoolean("ok")) throw new IllegalStateException("Adapter status is unavailable");
            JSONObject test = status.optJSONObject("test");
            boolean completed = test != null && test.optBoolean("cleaned") && !test.optBoolean("clipboard_restore_pending");
            if (completed || SystemClock.elapsedRealtime() - started >= 105_000L) NativeGuiTaskAdapter.requestShutdown();
            status = new JSONObject(NativeGuiTaskAdapter.peekStatus());
            if (!status.optBoolean("running", true)) {
                if (settledSince == 0L) settledSince = SystemClock.elapsedRealtime();
                if (SystemClock.elapsedRealtime() - settledSince >= 2000L) {
                    try {
                        if (!result.has("mdi_registry_before_aux_cleanup"))
                            result.put("mdi_registry_before_aux_cleanup", NativeGuiTaskResources.registrySnapshot(host));
                        JSONObject queryCleanup = NativeGuiQueryCleanup.cleanup(host, settledSince);
                        result.put("query_cleanup_verified", queryCleanup.optBoolean("ok") && queryCleanup.optBoolean("complete"))
                                .put("mdi_factory_released", queryCleanup.optBoolean("mdi_factory_released"))
                                .put("auxiliary_cleanup", queryCleanup);
                        if (queryCleanup.optBoolean("complete")) { finish(monitorFailure); return; }
                    } catch (Throwable error) {
                        // A permanent scope/ABI mismatch is not repaired by deleting
                        // task entries. Preserve owned storage and report the guard.
                        finish(error); return;
                    }
                }
            } else settledSince = 0L;
            new Handler(Looper.getMainLooper()).postDelayed(NativeGuiTaskHost::monitor, 500L);
        } catch (Throwable error) {
            if (monitorFailure == null) monitorFailure = error;
            NativeGuiTaskAdapter.requestShutdown();
            // Give native cancellation and delayed clipboard restoration time
            // to complete; never exit merely because the first status read failed.
            new Handler(Looper.getMainLooper()).postDelayed(NativeGuiTaskHost::monitor, 500L);
        }
    }

    private static void startDeadline() {
        Thread watchdog = new Thread(() -> {
            SystemClock.sleep(Math.max(1L, 120_000L - (SystemClock.elapsedRealtime() - started)));
            if (!emitted.compareAndSet(false, true)) return;
            JSONObject timeout = new JSONObject();
            try { timeout.put("event", "finished").put("version", 1).put("ok", false).put("timed_out", true)
                    .put("pid", android.os.Process.myPid()).put("host_uid", android.os.Process.myUid())
                    .put("target_package", TARGET).put("socket", SOCKET).put("companion_uid", result.optInt("companion_uid"))
                    .put("cleanup_scope", "host_process_resources").put("cleanup_verified", false)
                    .put("global_rollback_verified", false).put("storage_root", host.root.getAbsolutePath())
                    .put("storage_preserved", host.root.exists()); }
            catch (Exception ignored) { }
            System.out.println(timeout.toString()); System.out.flush();
            System.exit(7);
        }, "stepx-native-task-host-deadline");
        watchdog.setDaemon(true); watchdog.start();
    }

    private static int activeNativeTasks() throws Exception {
        return ((Number) bridge.getClass().getMethod("getActiveTaskCount").invoke(bridge)).intValue();
    }

    private static void finish(Throwable error) {
        if (!finished.compareAndSet(false, true)) return;
        boolean cleanup = true;
        boolean safeToRelease = true;
        try {
            result.put("event", "finished").put("initialization_attempted", attemptedInitialization)
                    .put("cleanup_scope", attemptedInitialization ? "host_process_resources" : host != null ? "temporary_storage_only" : "no_initialization")
                    .put("global_rollback_verified", false)
                    .put("model_diagnostics", NativeGuiModelDiagnostics.snapshot())
                    .put("event_diagnostics", NativeGuiEventDiagnostics.snapshot());
            if (error != null) {
                Throwable cause = unwrap(error);
                result.put("error_type", cause.getClass().getSimpleName());
                String code = cause.getMessage();
                if (code != null && code.matches("native_query_[a-z_]{1,90}")) result.put("guard_code", code);
                org.json.JSONArray frames = new org.json.JSONArray();
                for (StackTraceElement frame : cause.getStackTrace()) {
                    if (frames.length() == 5) break;
                    frames.put(frame.toString());
                }
                result.put("error_frames", frames);
            }
            if (adapterStarted) {
                JSONObject status = new JSONObject(NativeGuiTaskAdapter.peekStatus());
                JSONObject test = status.optJSONObject("test");
                safeToRelease = status.optBoolean("ok") && !status.optBoolean("running", true)
                        && status.optInt("active_task_count", -1) == 0
                        && result.optBoolean("query_cleanup_verified") && result.optBoolean("mdi_factory_released")
                        && (test == null || test.optBoolean("cleaned") && !test.optBoolean("native_task_present", true)
                            && !test.optBoolean("clipboard_restore_pending", true));
                result.put("adapter_closed", !status.optBoolean("running", true));
                if (test != null) result.put("test_result", test).put("test_invocation_started", test.optBoolean("invocation_started"))
                        .put("test_terminal", test.optString("terminal")).put("test_action_events", test.optInt("action_events"))
                        .put("owned_task_cleared", test.optBoolean("cleaned"));
            }
            if (attemptedInitialization && loader != null) {
                try {
                    if (bridge == null) bridge = singleton("com.stepos.taskrobot.llmapi.stepfun.SystemSdkBridge");
                    int count = activeNativeTasks(); result.put("native_task_count_before_release", count);
                    Class<?> manager = Class.forName("com.mdisdk.TaskManager", false, loader);
                    Field field = manager.getDeclaredField("activeTasks"); field.setAccessible(true);
                    int mdiCount = ((Map<?, ?>) field.get(null)).size(); result.put("mdi_task_count_before_release", mdiCount);
                    safeToRelease &= count == 0 && mdiCount == 0;
                } catch (Throwable failure) { safeToRelease = false; result.put("task_count_error", unwrap(failure).getClass().getSimpleName()); }
            }
            cleanup &= safeToRelease;
            if (safeToRelease) {
                if (result.optBoolean("test_invocation_started") && loader != null) {
                    try {
                        Object upload = singleton("com.zyql.common.utils.upload.UploadManager");
                        upload.getClass().getMethod("stop").invoke(upload);
                        result.put("owned_upload_cancel_requested", true);
                    } catch (Throwable failure) { cleanup = false; result.put("upload_release_error", unwrap(failure).getClass().getSimpleName()); }
                }
                if (attemptedApiInitialization) {
                    try { api.getClass().getMethod("destroy", Context.class).invoke(api, host); result.put("cloud_client_released", true); }
                    catch (Throwable failure) { cleanup = false; result.put("cloud_release_error", unwrap(failure).getClass().getSimpleName()); }
                }
                if (attemptedInitialization) {
                    try {
                        Object client = singleton("stepai.android.mindx.client.StepLLMClient");
                        client.getClass().getMethod("unbindService").invoke(client);
                        boolean disconnected = !Boolean.TRUE.equals(client.getClass().getMethod("isServiceConnected").invoke(client));
                        result.put("model_service_disconnected_after_release", disconnected); cleanup &= disconnected;
                    } catch (Throwable failure) { cleanup = false; result.put("model_release_error", unwrap(failure).getClass().getSimpleName()); }
                    try {
                        if (!result.optBoolean("mdi_factory_released"))
                            Class.forName("com.mdisdk.TaskManager", false, loader).getMethod("release").invoke(null);
                        result.put("mdi_released", true);
                    }
                    catch (Throwable failure) { cleanup = false; result.put("mdi_release_error", unwrap(failure).getClass().getSimpleName()); }
                }
                if (memoryDb != null) {
                    try {
                        // Never reset INSTANCE or S0.b.p while late callbacks can run.
                        Class<?> roomDb = Class.forName("androidx.room.RoomDatabase", false, loader);
                        roomDb.getMethod("close").invoke(memoryDb);
                        boolean closed = !Boolean.TRUE.equals(roomDb.getMethod("isOpen").invoke(memoryDb));
                        result.put("memory_db_closed", closed); cleanup &= closed;
                    } catch (Throwable failure) { cleanup = false; result.put("memory_release_error", unwrap(failure).getClass().getSimpleName()); }
                }
                if (attemptedInitialization) {
                    try { Class.forName("com.tencent.mmkv.MMKV", false, loader).getMethod("onExit").invoke(null); result.put("mmkv_released", true); }
                    catch (Throwable failure) { cleanup = false; result.put("mmkv_release_error", unwrap(failure).getClass().getSimpleName()); }
                }
            }
            if (controllerPublished && safeToRelease && host != null && host.providers != null) {
                try {
                    Bundle extras = new Bundle(); extras.putBinder("controller", companionController);
                    Bundle removed = host.providers.call(android.net.Uri.parse("content://com.stepx.companion.app.native_gui_task_probe"),
                            "unregister_binder", null, extras);
                    boolean unregistered = removed != null && removed.getBoolean("ok");
                    result.put("companion_binder_unregistered", unregistered); cleanup &= unregistered;
                } catch (Throwable failure) { cleanup = false; result.put("binder_release_error", unwrap(failure).getClass().getSimpleName()); }
            }
            if (host != null && safeToRelease && host.providers != null) {
                try {
                    host.providers.close();
                    JSONObject providers = host.providers.diagnostics();
                    boolean released = providers.optBoolean("closed") && providers.optInt("outstanding", -1) == 0;
                    result.put("content_providers_released", released); cleanup &= released;
                } catch (Throwable failure) {
                    cleanup = false; result.put("provider_release_error", unwrap(failure).getClass().getSimpleName());
                }
            }
            if (host != null && host.providers != null) {
                JSONObject providers = host.providers.diagnostics();
                result.put("provider_refs_acquired", providers.optInt("providers_returned"))
                        .put("provider_refs_outstanding", providers.optInt("outstanding"))
                        .put("provider_error_category", providers.optString("last_error"));
            }
            if (host != null && cleanup) {
                try { host.removeOwnedStorage(); result.put("storage_removed", !host.root.exists()); }
                catch (Throwable failure) { cleanup = false; result.put("storage_cleanup_error", unwrap(failure).getClass().getSimpleName()); }
            }
            if (host != null) result.put("storage_preserved", host.root.exists());
            result.put("cleanup_verified", attemptedInitialization || host != null ? cleanup : JSONObject.NULL)
                    .put("ok", error == null && adapterStarted && cleanup);
        } catch (Throwable failure) {
            try { result.put("ok", false).put("cleanup_verified", false).put("finalization_error", unwrap(failure).getClass().getSimpleName()); }
            catch (Exception ignored) { }
        }
        if (!emitted.compareAndSet(false, true)) return;
        System.out.println(result.toString()); System.out.flush();
        System.exit(Boolean.TRUE.equals(result.opt("ok")) ? 0 : 2);
    }

    private static Object singleton(String name) throws Exception {
        return Class.forName(name, true, loader).getField("INSTANCE").get(null);
    }
    private static Throwable unwrap(Throwable error) {
        while (error instanceof InvocationTargetException && error.getCause() != null) error = error.getCause();
        return error;
    }
    private static String readSmall(String path) throws Exception {
        try (InputStream input = new FileInputStream(path)) {
            byte[] bytes = new byte[4096]; int count = input.read(bytes);
            return count <= 0 ? "" : new String(bytes, 0, count, StandardCharsets.UTF_8).replace("\u0000", "").trim();
        }
    }
    private static String hex(byte[] bytes) {
        StringBuilder hash = new StringBuilder();
        for (byte value : bytes) hash.append(String.format(Locale.ROOT, "%02x", value & 255));
        return hash.toString();
    }
    private static String apkHash() throws Exception {
        MessageDigest digest = MessageDigest.getInstance("SHA-256");
        try (InputStream input = new FileInputStream(APK)) {
            byte[] buffer = new byte[65536]; int count;
            while ((count = input.read(buffer)) != -1) digest.update(buffer, 0, count);
        }
        return hex(digest.digest());
    }

    /** Redirects reviewed storage calls; preserves package identity. Not a sandbox. */
    private static final class HostApplication extends Application {
        private NativeGuiContentResolver providers;
        private final File root;
        private final File files;
        private final File cache;
        private final File code;
        private final File noBackup;
        private final String owner;

        HostApplication(Context base) throws Exception {
            attachBaseContext(base);
            File parent = base.getCodeCacheDir().getCanonicalFile();
            File data = new File(base.getApplicationInfo().dataDir).getCanonicalFile();
            if (!parent.equals(new File(data, "code_cache"))) throw new SecurityException("Unexpected original code cache path");
            root = new File(parent, PREFIX + UUID.randomUUID());
            String procStat = readSmall("/proc/self/stat");
            String startTicks = procStat.substring(procStat.lastIndexOf(')') + 2).split("\\s+")[19];
            if (!startTicks.matches("[0-9]+")) throw new SecurityException("Invalid process start identity");
            owner = new JSONObject().put("pid", android.os.Process.myPid())
                    .put("start_ticks", startTicks)
                    .put("boot_id", readSmall("/proc/sys/kernel/random/boot_id"))
                    .put("root", root.getAbsolutePath()).put("nonce", UUID.randomUUID().toString()).toString();
            // mkdir, not mkdirs: a reused path or redirected parent fails before writing.
            Os.mkdir(root.getAbsolutePath(), 0700);
            try {
                Os.chmod(root.getAbsolutePath(), 0700);
                if (!root.getCanonicalFile().equals(root)) throw new SecurityException("Isolated root is redirected");
                try (FileOutputStream output = new FileOutputStream(new File(root, "owner.json"))) {
                    output.write(owner.getBytes(StandardCharsets.UTF_8));
                }
                Os.chmod(new File(root, "owner.json").getAbsolutePath(), 0600);
                files = directory("files"); cache = directory("cache"); code = directory("code_cache"); noBackup = directory("no_backup");
            } catch (Throwable failure) {
                // Constructor failure otherwise prevents the caller retaining the
                // owner object. Only this just-created, never-reused root is removed.
                try { removeTree(root); } catch (Throwable cleanupFailure) { failure.addSuppressed(cleanupFailure); }
                throw failure;
            }
        }

        private File directory(String name) throws Exception {
            File directory = new File(root, name);
            Os.mkdir(directory.getAbsolutePath(), 0700);
            return directory;
        }

        @Override public Context getApplicationContext() { return this; }
        @Override public void startActivity(android.content.Intent intent) {
            startActivity(intent, null);
        }
        @Override public void startActivity(android.content.Intent intent, Bundle options) {
            int code = NativeGuiSettingsLauncher.startSettingsActivity(this, intent, options);
            try { result.put("settings_launch_accepted", true).put("settings_launch_result", code); }
            catch (Exception impossible) { throw new IllegalStateException("Settings result could not be recorded"); }
        }
        @Override public ContentResolver getContentResolver() {
            if (providers == null) throw new IllegalStateException("Reviewed native provider access is not initialized");
            return providers;
        }
        // No derived-context call was found in the reviewed initializer, runtime,
        // MMKV, MDI or model-client sources. Reject unexpected calls instead of
        // returning an unwrapped ContextImpl with the original storage paths.
        @Override public Context createPackageContext(String name, int flags) { throw blockedStorage(); }
        @Override public Context createContextForSplit(String splitName) { throw blockedStorage(); }
        @Override public Context createConfigurationContext(Configuration config) { throw blockedStorage(); }
        @Override public Context createDisplayContext(Display display) { throw blockedStorage(); }
        @Override public Context createAttributionContext(String tag) { throw blockedStorage(); }
        @Override public Context createContext(ContextParams params) { throw blockedStorage(); }
        @Override public Context createDeviceContext(int deviceId) { throw blockedStorage(); }
        @Override public Context createWindowContext(int type, Bundle options) { throw blockedStorage(); }
        @Override public Context createWindowContext(Display display, int type, Bundle options) { throw blockedStorage(); }
        // Framework APIs present on the pinned phone but hidden from android.jar.
        public Context createContextAsUser(UserHandle user, int flags) { throw blockedStorage(); }
        public Context createPackageContextAsUser(String name, int flags, UserHandle user) { throw blockedStorage(); }
        public Context createCredentialProtectedStorageContext() { throw blockedStorage(); }
        @Override public File getDataDir() { return root; }
        @Override public File getFilesDir() { return files; }
        @Override public File getCacheDir() { return cache; }
        @Override public File getCodeCacheDir() { return code; }
        @Override public File getNoBackupFilesDir() { return noBackup; }
        @Override public ApplicationInfo getApplicationInfo() {
            ApplicationInfo info = new ApplicationInfo(super.getApplicationInfo());
            info.dataDir = root.getAbsolutePath();
            try { ApplicationInfo.class.getField("credentialProtectedDataDir").set(info, root.getAbsolutePath()); }
            catch (ReflectiveOperationException error) { throw blockedStorage(); }
            info.deviceProtectedDataDir = root.getAbsolutePath();
            return info;
        }
        @Override public File getDir(String name, int mode) {
            if (mode != MODE_PRIVATE || !name.matches("[A-Za-z0-9_.-]{1,80}")) throw blockedStorage();
            try {
                File dir = new File(root, "app_" + name);
                if (!dir.exists()) Os.mkdir(dir.getAbsolutePath(), 0700);
                if (!dir.getCanonicalFile().equals(dir)) throw blockedStorage();
                return dir;
            } catch (Exception error) { throw blockedStorage(); }
        }
        // None of these APIs is needed by the audited in-memory task path. Reject new
        // dependencies instead of silently falling back to the original app data.
        @Override public SharedPreferences getSharedPreferences(String name, int mode) { throw blockedStorage(); }
        @Override public boolean deleteSharedPreferences(String name) { throw blockedStorage(); }
        @Override public boolean moveSharedPreferencesFrom(Context source, String name) { throw blockedStorage(); }
        @Override public boolean moveDatabaseFrom(Context source, String name) { throw blockedStorage(); }
        @Override public File getDatabasePath(String name) { throw blockedStorage(); }
        @Override public SQLiteDatabase openOrCreateDatabase(String name, int mode, SQLiteDatabase.CursorFactory factory) { throw blockedStorage(); }
        @Override public SQLiteDatabase openOrCreateDatabase(String name, int mode, SQLiteDatabase.CursorFactory factory, DatabaseErrorHandler handler) { throw blockedStorage(); }
        @Override public FileInputStream openFileInput(String name) { throw blockedStorage(); }
        @Override public FileOutputStream openFileOutput(String name, int mode) { throw blockedStorage(); }
        @Override public boolean deleteFile(String name) { throw blockedStorage(); }
        @Override public File getFileStreamPath(String name) { throw blockedStorage(); }
        @Override public String[] fileList() { throw blockedStorage(); }
        @Override public boolean deleteDatabase(String name) { throw blockedStorage(); }
        @Override public String[] databaseList() { throw blockedStorage(); }
        @Override public File getExternalFilesDir(String type) { throw blockedStorage(); }
        @Override public File[] getExternalFilesDirs(String type) { throw blockedStorage(); }
        @Override public File getExternalCacheDir() { throw blockedStorage(); }
        @Override public File[] getExternalCacheDirs() { throw blockedStorage(); }
        @Override public File getObbDir() { throw blockedStorage(); }
        @Override public File[] getObbDirs() { throw blockedStorage(); }
        @Override public Context createDeviceProtectedStorageContext() { throw blockedStorage(); }

        private SecurityException blockedStorage() { return new SecurityException("Storage API outside isolated task-host scope"); }

        void removeOwnedStorage() throws Exception {
            if (!root.getName().startsWith(PREFIX) || !root.getCanonicalFile().equals(root)
                    || !owner.equals(readSmall(new File(root, "owner.json").getAbsolutePath())))
                throw new SecurityException("Isolated storage ownership proof failed");
            removeTree(root);
        }

        private void removeTree(File file) throws Exception {
            if (!file.getCanonicalFile().equals(file)) throw new SecurityException("Refusing redirected cleanup entry");
            if (file.isDirectory()) {
                File[] entries = file.listFiles();
                if (entries == null) throw new IllegalStateException("Cannot inspect owned cleanup directory");
                for (File entry : entries) removeTree(entry);
            }
            if (!file.delete()) throw new IllegalStateException("Cannot remove owned task-host entry");
        }
    }
}

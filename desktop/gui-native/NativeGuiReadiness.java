package stepx;

import android.app.Application;
import android.content.Context;
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
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.Locale;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.atomic.AtomicBoolean;

/**
 * LOCAL RESEARCH CANDIDATE. Not wired to the desktop, companion, or installer.
 *
 * Hosts the original GUI SDK in a separate process and a fresh private storage
 * directory. It never constructs the original Application, invokes onCreate,
 * accesses an account, selects a model, starts a task, or exposes an endpoint.
 * Storage redirection only covers the reviewed initialization path. This is not
 * a filesystem or Binder sandbox: raw/native file paths, PackageManager results,
 * system services, and code executing in other processes remain outside it.
 *
 * Reviewed for the pinned firmware with the legacy model-service package absent.
 * The host runner must verify the actual display-control package set already
 * contains ability. Initialization must not create a new membership.
 *
 * Pending device evidence (none is claimed by a successful local build):
 * - Correct original APK/hash, UID/domain, package permissions and writable
 *   code_cache on the currently connected phone; no original process restart.
 * - Exact displayControlPackages baseline and after-state, independently of
 *   similarly named OmniApps; no added or removed membership.
 * - StepLLMRemoteService.init side effects and matching client unregister/unbind;
 *   a connection is not proof of model entitlement or inference availability.
 * - Initialization status, zero SDK/MDI tasks, exact isolated MMKV/cache paths,
 *   no preferences/database/storage-denial failures, and no original app onCreate.
 * - Observer release, service disconnect, per-run owned directory removal and
 *   process exit. A timeout or partial cleanup is failure, never rollback proof.
 * - This no-task readiness cannot establish screenshots, taps, cancel, pause,
 *   resume, task ownership, account access, redirect cleanup, fullscreen handling,
 *   KeepAlive behavior, trace upload, clipboard restoration, or real task success.
 */
public final class NativeGuiReadiness {
    private static final boolean PLATFORM_READINESS_AUDITED = true;
    private static final String TARGET = "com.stepos.ability";
    private static final String APK = "/system/priv-app/ability/ability.apk";
    private static final String SHA = "c9f56f3078df96d8d726422b1c622a1c19fe8c7dec8cadd8537d9a6ecf513ac4";
    private static final String PREFIX = "stepx-gui-readiness-";
    private static final JSONObject result = new JSONObject();
    private static final AtomicBoolean finished = new AtomicBoolean();
    private static final AtomicBoolean emitted = new AtomicBoolean();
    private static ClassLoader loader;
    private static HostApplication host;
    private static Object bridge;
    private static Object llmClient;
    private static boolean attemptedInitialization;

    public static void main(String[] args) {
        try {
            result.put("mode", "isolated_native_gui_readiness").put("version", 1)
                    .put("pid", android.os.Process.myPid()).put("uid", android.os.Process.myUid())
                    .put("tasks_started", 0).put("account_accessed", false)
                    .put("execution_verified", false).put("model_access_verified", false)
                    .put("production_ready", false).put("audit_gate_open", PLATFORM_READINESS_AUDITED);
            if (args.length != 1 || !"--initialize-readiness".equals(args[0]))
                throw new IllegalArgumentException("Exact --initialize-readiness argument required");
            if (!PLATFORM_READINESS_AUDITED)
                throw new SecurityException("Pending model-service init audit and actual display-control baseline verification");
            if (android.os.Process.myUid() != 1000)
                throw new SecurityException("Requires existing system UID service scope");
            String domain = readSmall("/proc/self/attr/current");
            result.put("selinux_context", domain);
            if (!"u:r:system_app:s0".equals(domain))
                throw new SecurityException("Unexpected SELinux domain");
            if (!SHA.equals(apkHash())) throw new SecurityException("Unsupported original ability APK");
            result.put("apk_sha256", SHA);
            if (Looper.getMainLooper() == null) Looper.prepareMainLooper();
            Class<?> activityThread = Class.forName("android.app.ActivityThread");
            Object thread = activityThread.getMethod("systemMain").invoke(null);
            Context system = (Context) activityThread.getMethod("getSystemContext").invoke(thread);
            Context context = system.createPackageContext(TARGET, Context.CONTEXT_INCLUDE_CODE | Context.CONTEXT_IGNORE_SECURITY);
            // This obsolete client is not the default native GUI cloud route.
            // Do not bind an unreviewed service if another firmware installs it.
            try {
                context.getPackageManager().getPackageInfo("stepai.android.mindx.sdk", 0);
                throw new SecurityException("Legacy model-service package must be absent for this reviewed readiness path");
            } catch (PackageManager.NameNotFoundException expected) {
                result.put("legacy_model_service_absent", true);
            }
            ApplicationInfo info = context.getApplicationInfo();
            if (!TARGET.equals(context.getPackageName()) || !APK.equals(info.sourceDir) || info.uid != 1000)
                throw new SecurityException("Real package context identity mismatch");
            for (String permission : new String[] { "com.stepos.ability.permission.SCREEN_OPS", "com.stepos.ability.permission.ACCESS_AGENT_SERVICE" })
                if (context.checkSelfPermission(permission) != PackageManager.PERMISSION_GRANTED)
                    throw new SecurityException("Required original package permission absent");
            host = new HostApplication(context);
            loader = context.getClassLoader();
            result.put("package_context", host.getPackageName()).put("original_application_started", false)
                    .put("storage_root", host.root.getAbsolutePath()).put("application_context_is_host", host.getApplicationContext() == host)
                    .put("reviewed_storage_paths_redirected", true).put("storage_sandbox", false)
                    .put("model_config_source", "fresh_isolated_defaults");

            // Fail closed on unexpected ABI before invoking any original initializer.
            Class<?> initializer = Class.forName("com.stepos.taskrobot.llmapi.stepfun.GuiAgentInitializer", false, loader);
            Class<?> account = Class.forName("f6.c", false, loader);
            initializer.getMethod("initGuiAgent", Context.class, account);
            Class.forName("com.mdisdk.TaskManager", false, loader).getMethod("release");
            Class<?> clientType = Class.forName("stepai.android.mindx.client.StepLLMClient", false, loader);
            clientType.getMethod("unbindService");
            clientType.getMethod("isServiceConnected");
            startDeadline();
            attemptedInitialization = true;
            // Null means the initializer skips AuthTokenCache.init; no account object is constructed.
            initializer.getMethod("initGuiAgent", Context.class, account)
                    .invoke(initializer.getField("INSTANCE").get(null), host, null);
            bridge = singleton("com.stepos.taskrobot.llmapi.stepfun.SystemSdkBridge");
            llmClient = singleton("stepai.android.mindx.client.StepLLMClient");
            result.put("sdk_initialized", bridge.getClass().getMethod("isInitialized").invoke(bridge));
            Object runtime = Class.forName("ua.a", false, loader).getField("c").get(null);
            result.put("runtime_initialized", runtime != null).put("llm_config_present", hasLlmConfig(runtime))
                    .put("native_task_count", bridge.getClass().getMethod("getActiveTaskCount").invoke(bridge));
            Class<?> mmkv = Class.forName("com.tencent.mmkv.MMKV", false, loader);
            String mmkvRoot = (String) mmkv.getMethod("getRootDir").invoke(null);
            if (!new File(host.getFilesDir(), "mmkv").getCanonicalPath().equals(new File(mmkvRoot).getCanonicalPath()))
                throw new SecurityException("Native MMKV root escaped isolated storage");
            result.put("mmkv_root_isolated", true);
            // Allow only the original binding callback to settle. No task or inference is submitted.
            new Handler(Looper.getMainLooper()).postDelayed(() -> finish(null), 2500L);
            Looper.loop();
        } catch (Throwable error) {
            finish(error);
        }
    }

    private static void startDeadline() {
        Thread watchdog = new Thread(() -> {
            SystemClock.sleep(20000L);
            if (emitted.compareAndSet(false, true)) {
                JSONObject timeout = new JSONObject();
                try { timeout.put("ok", false).put("mode", "isolated_native_gui_readiness")
                        .put("timed_out", true).put("cleanup_verified", false).put("tasks_started", 0)
                        .put("cleanup_scope", "host_process_resources").put("global_rollback_verified", false)
                        .put("pid", android.os.Process.myPid()).put("storage_root", host.root.getAbsolutePath()); }
                catch (Exception ignored) { }
                System.out.println(timeout.toString());
                System.out.flush();
                // A deadlocked initializer cannot be called a successful rollback.
                System.exit(7);
            }
        }, "stepx-readiness-deadline");
        watchdog.setDaemon(true);
        watchdog.start();
    }

    private static void finish(Throwable error) {
        if (!finished.compareAndSet(false, true)) return;
        boolean cleanup = true;
        try {
            result.put("initialization_attempted", attemptedInitialization)
                    .put("cleanup_scope", attemptedInitialization ? "host_process_resources" : host != null ? "temporary_storage_only" : "no_initialization")
                    .put("global_rollback_verified", false);
            if (error != null) result.put("error_type", unwrap(error).getClass().getSimpleName());
            if (attemptedInitialization && loader != null) {
                try {
                    if (llmClient == null) llmClient = singleton("stepai.android.mindx.client.StepLLMClient");
                    result.put("model_service_connected", llmClient.getClass().getMethod("isServiceConnected").invoke(llmClient));
                    llmClient.getClass().getMethod("unbindService").invoke(llmClient);
                    boolean connected = Boolean.TRUE.equals(llmClient.getClass().getMethod("isServiceConnected").invoke(llmClient));
                    result.put("model_service_disconnected_after_release", !connected);
                    cleanup &= !connected;
                } catch (Throwable failure) {
                    result.put("model_service_release_error", unwrap(failure).getClass().getSimpleName()); cleanup = false;
                }
                try {
                    Class<?> taskManager = Class.forName("com.mdisdk.TaskManager", false, loader);
                    Field tasksField = taskManager.getDeclaredField("activeTasks");
                    tasksField.setAccessible(true);
                    int count = ((Map<?, ?>) tasksField.get(null)).size();
                    result.put("mdi_task_count_before_release", count);
                    if (count != 0) throw new SecurityException("Unexpected task in no-task readiness process");
                    taskManager.getMethod("release").invoke(null);
                    result.put("mdi_released", true);
                } catch (Throwable failure) {
                    result.put("mdi_release_error", unwrap(failure).getClass().getSimpleName()); cleanup = false;
                }
                try {
                    Class<?> mmkv = Class.forName("com.tencent.mmkv.MMKV", false, loader);
                    if (mmkv.getMethod("getRootDir").invoke(null) != null) mmkv.getMethod("onExit").invoke(null);
                    result.put("mmkv_released", true);
                } catch (Throwable failure) {
                    result.put("mmkv_release_error", unwrap(failure).getClass().getSimpleName()); cleanup = false;
                }
            }
            if (host != null) {
                try { host.removeOwnedStorage(); result.put("storage_removed", !host.root.exists()); }
                catch (Throwable failure) { result.put("storage_cleanup_error", failure.getClass().getSimpleName()); cleanup = false; }
            }
            boolean initialized = Boolean.TRUE.equals(result.opt("sdk_initialized")) && Boolean.TRUE.equals(result.opt("runtime_initialized"));
            result.put("cleanup_verified", attemptedInitialization || host != null ? cleanup : JSONObject.NULL)
                    .put("ok", error == null && initialized && cleanup);
        } catch (Throwable failure) {
            try { result.put("ok", false).put("cleanup_verified", false).put("finalization_error", failure.getClass().getSimpleName()); }
            catch (Exception ignored) { }
        }
        if (!emitted.compareAndSet(false, true)) return;
        System.out.println(result.toString());
        System.out.flush();
        System.exit(Boolean.TRUE.equals(result.opt("ok")) ? 0 : 2);
    }

    private static Object singleton(String name) throws Exception {
        return Class.forName(name, true, loader).getField("INSTANCE").get(null);
    }

    private static boolean hasLlmConfig(Object runtime) throws Exception {
        if (runtime == null) return false;
        for (Field field : runtime.getClass().getDeclaredFields()) {
            if ("stepai.aios.guiagent.config.RuntimeConfig".equals(field.getType().getName())) {
                field.setAccessible(true);
                Object config = field.get(runtime);
                return config != null && config.getClass().getMethod("getLlmConfig").invoke(config) != null;
            }
        }
        return false;
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

    private static String apkHash() throws Exception {
        MessageDigest digest = MessageDigest.getInstance("SHA-256");
        try (InputStream input = new FileInputStream(APK)) {
            byte[] buffer = new byte[65536]; int count;
            while ((count = input.read(buffer)) != -1) digest.update(buffer, 0, count);
        }
        StringBuilder hash = new StringBuilder();
        for (byte value : digest.digest()) hash.append(String.format(Locale.ROOT, "%02x", value & 255));
        return hash.toString();
    }

    /** Redirects reviewed storage calls; preserves package identity. Not a sandbox. */
    private static final class HostApplication extends Application {
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
            owner = new JSONObject().put("pid", android.os.Process.myPid())
                    .put("boot_id", readSmall("/proc/sys/kernel/random/boot_id"))
                    .put("root", root.getAbsolutePath()).put("nonce", UUID.randomUUID().toString()).toString();
            // mkdir, not mkdirs: a reused path or redirected parent fails before writing.
            Os.mkdir(root.getAbsolutePath(), 0700);
            try {
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
        // None of these APIs is needed by the audited readiness path. Reject new
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

        private SecurityException blockedStorage() { return new SecurityException("Storage API outside isolated readiness scope"); }

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
            if (!file.delete()) throw new IllegalStateException("Cannot remove owned readiness entry");
        }
    }
}

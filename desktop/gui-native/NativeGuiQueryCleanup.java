package stepx;

import android.content.Context;
import android.hardware.display.DisplayManager;
import android.hardware.display.VirtualDisplay;
import android.os.Binder;
import android.os.Build;
import android.os.Handler;
import android.os.Looper;
import android.os.Process;
import android.os.SystemClock;
import android.view.Display;
import java.lang.reflect.Field;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.util.Map;
import java.util.concurrent.atomic.AtomicInteger;
import org.json.JSONObject;

/**
 * RESEARCH ONLY. Releases the isolated SDK's known idle query and main-display
 * watcher resources using their original callbacks, never by editing task maps.
 *
 * The host must close its adapter, stop all query producers, and observe at
 * least two seconds of quiescence before calling this on its main thread.
 * The native query mutex prevents concurrent create/renew during inspection
 * and finish. It cannot invalidate a reference already held by a query caller;
 * quiescence and shutdown are mandatory, not an atomic global safety proof.
 *
 * Once helper/display cleanup is verified, the original TaskManager.release is
 * called while the same mutex is held and its map is empty. This closes only
 * this host's factory so a waiting query cannot recreate a virtual display.
 * No clearAll, redirect mutation or display-map mutation occurs here.
 * complete=true proves this process's observed resources and factory are gone;
 * external baseline verification is still needed.
 */
public final class NativeGuiQueryCleanup {
    private static final String TARGET = "com.stepos.ability";
    private static final String QUERY = "__query_task__";
    private static final String WATCH = "__main_display_watcher__";
    private static final String WATCHER = "com.stepos.taskrobot.llmapi.stepfun.MainDisplayWatcher";
    private static final String BRIDGE = "com.stepos.taskrobot.llmapi.stepfun.SystemSdkBridge";
    private static final String MANAGER = "com.mdisdk.TaskManager";
    private static final String TASK = "com.mdisdk.core.DefaultTaskImpl";
    private static Context attemptedContext;
    private static int attemptedDisplay = -1;
    private static int attemptedWatcherDisplay = -1;
    private static boolean watcherResetInvoked;
    private static String watcherError = "";
    private static boolean callbackInvoked;
    private static String callbackError = "";
    private static boolean factoryReleaseAttempted;
    private static boolean factoryReleased;
    private static String factoryError = "";

    private NativeGuiQueryCleanup() {}

    /** No waiting or sleeping on the main queue. Pending results may be rechecked. */
    public static JSONObject cleanup(Context host, long quiescentSinceElapsedMs) throws Exception {
        verifyHost(host, quiescentSinceElapsedMs);
        ClassLoader loader = host.getClassLoader();
        Class<?> bridge = Class.forName(BRIDGE, false, loader);
        Class<?> manager = Class.forName(MANAGER, false, loader);
        Object managerContext = read(manager, "applicationContext", null);
        if (bridge.getClassLoader() != loader || manager.getClassLoader() != loader
                || read(bridge, "appContext", null) != host
                || !(managerContext == host || (managerContext == null
                    && attemptedContext == host && factoryReleaseAttempted)))
            throw new SecurityException("native_query_host_ownership_mismatch");
        Map<?, ?> nativeTasks = map(read(bridge, "tasks", null));
        Map<?, ?> mdiTasks = map(read(manager, "activeTasks", null));
        if (!nativeTasks.isEmpty()) throw new IllegalStateException("native_query_native_tasks_active");
        Object mutex = read(bridge, "queryTaskMutex", null);
        if (mutex == null) throw new IllegalStateException("native_query_mutex_missing");
        Class<?> mutexInterface = field(bridge, "queryTaskMutex").getType();
        Method tryLock = mutexInterface.getMethod("tryLock", Object.class);
        Method unlock = mutexInterface.getMethod("unlock", Object.class);
        Object owner = new Object();
        if (!Boolean.TRUE.equals(tryLock.invoke(mutex, owner)))
            return pending("query_mutex_busy");
        try {
            Object watcher = read(bridge, "mainDisplayWatcher", null);
            if (watcher == null || !WATCHER.equals(watcher.getClass().getName())
                    || watcher.getClass().getClassLoader() != loader)
                throw new SecurityException("native_query_watcher_abi_mismatch");
            Object stateLock = read(watcher.getClass(), "stateLock", watcher);
            if (stateLock == null) throw new IllegalStateException("native_query_watcher_lock_missing");
            // Original watcher callbacks all use this same monitor. Hold it from
            // identity checks through reset and factory release to prevent a
            // concurrent onTaskReleased from replacing the checked watchTask.
            synchronized (stateLock) {
                Object query = read(bridge, "queryTask", null);
                Object watch = read(watcher.getClass(), "watchTask", watcher);
                verifyAdapterClosed(host);
                // After any action, subsequent calls only observe physical
                // disposal; they cannot finish a newly-created/replaced task.
                if (attemptedContext != null) {
                    if (attemptedContext != host)
                        throw new SecurityException("native_query_cleanup_context_changed");
                    return observed(host, manager, watcher, query, nativeTasks, mdiTasks);
                }
                verifyRegistry(mdiTasks, query, watch);
                if (!nativeTasks.isEmpty()) throw new IllegalStateException("native_query_native_tasks_active");
                int queryDisplay = query == null ? -1 : verifyIdleTask(host, query, QUERY, mutexInterface);
                int watchDisplay = watch == null ? -1 : verifyIdleTask(host, watch, WATCH, mutexInterface);
                if (queryDisplay > 0 && queryDisplay == watchDisplay)
                    throw new SecurityException("native_query_display_alias");
                Object handlerValue = read(bridge, "mainHandler", null);
                Object runnableValue = read(bridge, "queryTaskIdleRunnable", null);
                if (!(handlerValue instanceof Handler) || !(runnableValue instanceof Runnable))
                    throw new IllegalStateException("native_query_idle_callback_abi_mismatch");
                Handler handler = (Handler) handlerValue;
                Runnable runnable = (Runnable) runnableValue;
                if (handler.getLooper() != Looper.getMainLooper()
                        || query != null && !handler.hasCallbacks(runnable))
                    throw new IllegalStateException("native_query_idle_callback_not_pending");
                Handler watchHandler = (Handler) read(watcher.getClass(), "handler", watcher);
                if (watchHandler == null || watchHandler.getLooper() != Looper.getMainLooper())
                    throw new IllegalStateException("native_query_watcher_handler_mismatch");
                Method reset = watcher.getClass().getMethod("reset");
                verifyRegistry(mdiTasks, query, watch);
                if (read(bridge, "queryTask", null) != query
                        || read(watcher.getClass(), "watchTask", watcher) != watch || !nativeTasks.isEmpty())
                    throw new IllegalStateException("native_query_scope_changed");
                attemptedContext = host;
                attemptedDisplay = queryDisplay;
                attemptedWatcherDisplay = watchDisplay;
                if (watch != null) {
                    watcherResetInvoked = true;
                    try { reset.invoke(watcher); } // Original observer/timer teardown and Task.finish(true).
                    catch (Throwable failure) { watcherError = rootCause(failure).getClass().getSimpleName(); }
                }
                if (query != null) {
                    handler.removeCallbacks(runnable);
                    callbackInvoked = true;
                    try { runnable.run(); } // Original finish(true), map callback, queryTask=null.
                    catch (Throwable failure) { callbackError = rootCause(failure).getClass().getSimpleName(); }
                }
                return observed(host, manager, watcher, read(bridge, "queryTask", null), nativeTasks, mdiTasks);
            }
        } finally {
            unlock.invoke(mutex, owner);
        }
    }

    private static void verifyRegistry(Map<?, ?> mdi, Object query, Object watch) {
        int expected = (query == null ? 0 : 1) + (watch == null ? 0 : 1);
        if (query != null && query == watch || mdi.size() != expected
                || mdi.get(QUERY) != query || mdi.get(WATCH) != watch
                || query == null && mdi.containsKey(QUERY) || watch == null && mdi.containsKey(WATCH))
            throw new SecurityException("native_query_task_scope_mismatch");
    }

    private static int verifyIdleTask(Context host, Object task, String key, Class<?> mutexInterface) throws Exception {
        ClassLoader loader = host.getClassLoader();
        if (!TASK.equals(task.getClass().getName()) || task.getClass().getClassLoader() != loader
                || !key.equals(task.getClass().getMethod("getName").invoke(task)))
            throw new SecurityException("native_query_task_identity_mismatch");
        if (Boolean.TRUE.equals(read(task.getClass(), "isFinishing", task))
                || !map(read(task.getClass(), "executionJobs", task)).isEmpty())
            throw new IllegalStateException("native_query_task_not_idle");
        Object executions = read(task.getClass(), "activeExecutions", task);
        if (!(executions instanceof AtomicInteger) || ((AtomicInteger) executions).get() != 0)
            throw new IllegalStateException("native_query_executions_active");
        Object finishMutex = read(task.getClass(), "finishMutex", task);
        if (finishMutex == null || Boolean.TRUE.equals(mutexInterface.getMethod("isLocked").invoke(finishMutex)))
            throw new IllegalStateException("native_query_finish_in_progress");
        Object displayValue = task.getClass().getMethod("getDisplayId").invoke(task);
        if (!(displayValue instanceof Integer) || ((Integer) displayValue) < 90000000
                || ((Integer) displayValue) >= 100000000)
            throw new SecurityException("native_query_virtual_display_required");
        int displayId = (Integer) displayValue;
        DisplayManager displays = host.getSystemService(DisplayManager.class);
        Display display = displays == null ? null : displays.getDisplay(displayId);
        if (display == null || !Integer.valueOf(5).equals(Display.class.getMethod("getType").invoke(display))
                || !Integer.valueOf(Process.myUid()).equals(Display.class.getMethod("getOwnerUid").invoke(display)))
            throw new SecurityException("native_query_display_owner_mismatch");
        Object taskContext = read(task.getClass(), "taskContext", task);
        if (taskContext == null || !"com.mdisdk.core.TaskContext".equals(taskContext.getClass().getName())
                || taskContext.getClass().getClassLoader() != loader)
            throw new SecurityException("native_query_task_context_mismatch");
        // The pinned DEX has two zero-argument getDisplay methods distinguished
        // only by return type (OmniDisplay vs Android Display). getMethod(name)
        // may select either. Read the verified backing field, never guess which
        // return-type overload reflection selected.
        Field displayField = field(taskContext.getClass(), "display");
        if (!"com.mdisdk.display.OmniDisplay".equals(displayField.getType().getName())
                || displayField.getType().getClassLoader() != loader
                || read(taskContext.getClass(), "applicationContext", taskContext) != host)
            throw new SecurityException("native_query_display_field_mismatch");
        Object omni = displayField.get(taskContext);
        if (omni == null || omni.getClass() != displayField.getType()
                || omni.getClass().getClassLoader() != loader)
            throw new SecurityException("native_query_display_wrapper_mismatch");
        Object target = read(omni.getClass(), "targetDisplay", omni);
        Object virtual = read(omni.getClass(), "virtualDisplay", omni);
        if (!(target instanceof Display) || ((Display) target).getDisplayId() != displayId
                || !(virtual instanceof VirtualDisplay)
                || ((VirtualDisplay) virtual).getDisplay().getDisplayId() != displayId)
            throw new SecurityException("native_query_owned_virtual_display_mismatch");
        return displayId;
    }

    private static void verifyHost(Context host, long since) {
        long now = SystemClock.elapsedRealtime();
        if (Build.VERSION.SDK_INT != 36 || Process.myUid() != 1000
                || Binder.getCallingUid() != Process.myUid() || Looper.myLooper() != Looper.getMainLooper()
                || host == null || host.getApplicationContext() != host || !TARGET.equals(host.getPackageName())
                || host.getApplicationInfo().uid != Process.myUid())
            throw new SecurityException("native_query_host_identity_mismatch");
        if (since <= 0L || since > now || now - since < 2000L)
            throw new IllegalStateException("native_query_quiescence_required");
    }

    private static void verifyAdapterClosed(Context host) throws Exception {
        // Read fields directly. Calling readiness/listActiveTasks during teardown
        // may itself query the original display layer and create another task.
        Class<?> type = Class.forName("stepx.NativeGuiTaskAdapter", false,
                NativeGuiQueryCleanup.class.getClassLoader());
        Object adapter = read(type, "hostedAdapter", null);
        if (adapter == null || read(type, "app", adapter) != host
                || !Boolean.FALSE.equals(read(type, "running", adapter))
                || !Boolean.TRUE.equals(read(type, "shutdownRequested", adapter)))
            throw new IllegalStateException("native_query_adapter_not_closed");
    }

    private static JSONObject observed(Context host, Class<?> manager, Object watcher, Object query,
            Map<?, ?> nativeTasks, Map<?, ?> mdiTasks) throws Exception {
        DisplayManager displays = host.getSystemService(DisplayManager.class);
        boolean gone = attemptedDisplay == -1 || (displays != null
                && attemptedDisplay > 0 && displays.getDisplay(attemptedDisplay) == null);
        boolean watchGone = attemptedWatcherDisplay == -1 || (displays != null
                && attemptedWatcherDisplay > 0 && displays.getDisplay(attemptedWatcherDisplay) == null);
        boolean watchCleared = read(watcher.getClass(), "watchTask", watcher) == null
                && read(watcher.getClass(), "observerRegId", watcher) == null
                && read(watcher.getClass(), "settleTimeout", watcher) == null
                && "IDLE".equals(String.valueOf(read(watcher.getClass(), "state", watcher)));
        boolean queryReleased = callbackError.isEmpty() && watcherError.isEmpty() && query == null
                && nativeTasks.isEmpty() && mdiTasks.isEmpty() && gone && watchGone && watchCleared;
        if (queryReleased && !factoryReleaseAttempted) {
            // Still holding queryTaskMutex. Unlike a blanket fallback, this call
            // is permitted only after the exact query and physical display have
            // gone and the native+MDI maps are confirmed empty.
            factoryReleaseAttempted = true;
            try { manager.getMethod("release").invoke(null); }
            catch (Throwable failure) { factoryError = rootCause(failure).getClass().getSimpleName(); }
        }
        if (factoryReleaseAttempted)
            factoryReleased = factoryError.isEmpty() && read(manager, "applicationContext", null) == null
                    && read(manager, "displayManager", null) == null && mdiTasks.isEmpty();
        boolean complete = queryReleased && factoryReleased && nativeTasks.isEmpty();
        return new JSONObject().put("ok", complete).put("complete", complete)
                .put("scope", "isolated_sdk_idle_helpers").put("query_present", attemptedDisplay > 0)
                .put("callback_invoked", callbackInvoked).put("query_display_id", attemptedDisplay)
                .put("native_tasks_empty", nativeTasks.isEmpty()).put("mdi_tasks_empty", mdiTasks.isEmpty())
                .put("query_cleared", query == null).put("display_gone", gone)
                .put("watcher_present", attemptedWatcherDisplay > 0).put("watcher_display_id", attemptedWatcherDisplay)
                .put("watcher_reset_invoked", watcherResetInvoked).put("watcher_cleared", watchCleared)
                .put("watcher_display_gone", watchGone).put("watcher_error", watcherError)
                .put("callback_error", callbackError).put("cleanup_pending", !complete)
                .put("mdi_factory_release_attempted", factoryReleaseAttempted)
                .put("mdi_factory_released", factoryReleased).put("mdi_factory_error", factoryError);
    }

    private static JSONObject pending(String reason) throws Exception {
        return new JSONObject().put("ok", false).put("complete", false)
                .put("scope", "isolated_sdk_query_task").put("callback_invoked", false)
                .put("cleanup_pending", true).put("reason", reason);
    }

    private static Field field(Class<?> type, String name) throws Exception {
        Field field = type.getDeclaredField(name); field.setAccessible(true); return field;
    }
    private static Object read(Class<?> type, String name, Object instance) throws Exception {
        return field(type, name).get(instance);
    }
    private static Map<?, ?> map(Object value) {
        if (!(value instanceof Map)) throw new IllegalStateException("native_query_map_abi_mismatch");
        return (Map<?, ?>) value;
    }
    private static Throwable rootCause(Throwable error) {
        while (error instanceof InvocationTargetException && error.getCause() != null) error = error.getCause();
        return error;
    }
}

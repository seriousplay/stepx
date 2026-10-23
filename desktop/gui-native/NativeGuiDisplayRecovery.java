package stepx;

import android.content.Context;
import android.hardware.display.DisplayManager;
import android.os.Handler;
import android.os.Looper;
import android.os.Process;
import android.os.SystemClock;
import android.util.SparseBooleanArray;
import org.json.JSONObject;
import java.io.FileInputStream;
import java.lang.reflect.Field;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.util.Map;
import java.util.Set;
import java.util.TreeMap;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicInteger;

/**
 * ONE FAILED RESEARCH RUN ONLY: d0cf1f25c3b94bcdb88513b033bd0fd6.
 * Replays the original display-removed handler for its already-dead display.
 * No display destruction, counter setter, task movement, process killing,
 * allowlist mutation or map editing. The source deliberately has no parameters.
 *
 * The transport executes on an interrupted system_server thread. Never acquire
 * private framework locks there: post exactly once to the original WMS handler.
 * The callback checks its short deadline after locked observations, immediately
 * before the original callback. A queue timeout does not authorize a late repair.
 * This cleans a stale current mapping; its historical previous display becomes
 * 90000001 as normal onDisplayRemoved specifies. This is NOT full rollback proof.
 */
public final class NativeGuiDisplayRecovery {
    private static final int OWNED_DISPLAY = 90000001;
    private static final int EXPECTED_PREVIOUS = 90000000;
    private static final int LAUNCHER_UID = 10124;
    private static final String LAUNCHER = "com.android.launcher3";
    private static final AtomicBoolean REQUESTED = new AtomicBoolean();

    public static String run() {
        final JSONObject out = new JSONObject();
        final CountDownLatch done = new CountDownLatch(1);
        final AtomicBoolean expired = new AtomicBoolean();
        Handler handler = null;
        Runnable callback = null;
        boolean posted = false;
        try {
            out.put("version", 1).put("mode", "native_gui_owned_display_recovery")
                    .put("pid", Process.myPid()).put("uid", Process.myUid()).put("read_only", false)
                    .put("owned_display_id", OWNED_DISPLAY).put("target_uid", LAUNCHER_UID)
                    .put("target_package", LAUNCHER).put("ok", false)
                    .put("scheduled", false).put("callback_done", false)
                    .put("preconditions_verified", false).put("operation_count", 0)
                    .put("replayed_original_on_display_removed", false)
                    .put("global_rollback_verified", false);
            require(Process.myUid() == 1000 && "system_server".equals(readSmall("/proc/self/cmdline"))
                    && "u:r:system_server:s0".equals(readSmall("/proc/self/attr/current")));
            require(REQUESTED.compareAndSet(false, true));
            final Object manager = Class.forName("com.android.server.wm.OmniManagerServiceStub")
                    .getMethod("get").invoke(null);
            require(manager != null && "com.android.server.wm.StepOSOmniManagerImpl".equals(manager.getClass().getName()));
            final Object wm = field(manager, "mWm");
            final Object controller = field(manager, "mOmniDisplayController");
            require(wm != null && controller != null && "com.android.server.wm.StepOSOmniDisplayController".equals(controller.getClass().getName()));
            Object handlerValue = field(wm, "mH");
            require(handlerValue instanceof Handler);
            handler = (Handler) handlerValue;
            require(handler.getLooper() != Looper.myLooper());
            final long deadline = SystemClock.uptimeMillis() + 1800L;
            callback = new Runnable() {
                @Override public void run() {
                    try {
                        checkDeadline(expired, deadline);
                        final Context context = (Context) field(wm, "mContext");
                        final DisplayManager displays = context.getSystemService(DisplayManager.class);
                        require(displays != null && displays.getDisplay(OWNED_DISPLAY) == null);
                        require(displays.getDisplay(EXPECTED_PREVIOUS) == null);
                        require(displays.getDisplays().length == 1 && displays.getDisplays()[0].getDisplayId() == 0);
                        requireRedirectZero(manager);
                        requireOnlyOriginalControlPackage(controller);
                        requireNoInputOwnership(controller);
                        TreeMap<Integer, AppState> before = readMap(controller);
                        AppState target = before.get(LAUNCHER_UID);
                        require(target != null && LAUNCHER.equals(target.name)
                                && target.current == OWNED_DISPLAY && target.previous == EXPECTED_PREVIOUS);
                        int matching = 0;
                        for (AppState state : before.values()) if (state.current == OWNED_DISPLAY) matching++;
                        require(matching == 1);
                        checkDeadline(expired, deadline);
                        require(displays.getDisplay(OWNED_DISPLAY) == null);
                        requireRedirectZero(manager);
                        out.put("preconditions_verified", true);
                        out.put("current_before", target.current).put("previous_before", target.previous);
                        Method original = controller.getClass().getDeclaredMethod("onDisplayRemoved", int.class);
                        original.setAccessible(true);
                        checkDeadline(expired, deadline);
                        out.put("operation_count", 1);
                        original.invoke(controller, OWNED_DISPLAY);
                        out.put("replayed_original_on_display_removed", true);
                        TreeMap<Integer, AppState> after = readMap(controller);
                        AppState updated = after.get(LAUNCHER_UID);
                        require(updated != null && LAUNCHER.equals(updated.name)
                                && updated.current == -1 && updated.previous == OWNED_DISPLAY);
                        before.remove(LAUNCHER_UID); after.remove(LAUNCHER_UID);
                        boolean unchanged = before.equals(after);
                        out.put("unrelated_map_unchanged", unchanged);
                        require(unchanged);
                        requireRedirectZero(manager);
                        requireOnlyOriginalControlPackage(controller);
                        requireNoInputOwnership(controller);
                        require(displays.getDisplay(OWNED_DISPLAY) == null);
                        out.put("current_after", updated.current).put("previous_after", updated.previous)
                                .put("redirect_unchanged", true).put("control_packages_unchanged", true)
                                .put("ime_focus_unchanged", true).put("actual_display_absent", true).put("ok", true);
                    } catch (Throwable error) {
                        putError(out, error);
                    } finally {
                        try { out.put("callback_done", true); } catch (Exception ignored) { }
                        done.countDown();
                    }
                }
            };
            out.put("scheduled", true);
            require(handler.post(callback));
            posted = true;
            if (!done.await(2500L, TimeUnit.MILLISECONDS)) {
                expired.set(true);
                handler.removeCallbacks(callback);
                // Do not race JSONObject serialization against a still-running
                // callback. A timeout reports no success and requires readback.
                return new JSONObject().put("version", 1).put("mode", "native_gui_owned_display_recovery")
                        .put("pid", Process.myPid()).put("uid", Process.myUid()).put("ok", false)
                        .put("error_type", "CallbackTimeout").put("global_rollback_verified", false).toString();
            }
        } catch (Throwable error) {
            expired.set(true);
            if (handler != null && callback != null) handler.removeCallbacks(callback);
            if (posted) {
                try {
                    return new JSONObject().put("version", 1).put("mode", "native_gui_owned_display_recovery")
                            .put("pid", Process.myPid()).put("uid", Process.myUid()).put("ok", false)
                            .put("error_type", "InterruptedReadback").put("global_rollback_verified", false).toString();
                } catch (Exception ignored) { return "{\"ok\":false,\"error_type\":\"InterruptedReadback\"}"; }
            }
            putError(out, error);
        }
        return out.toString();
    }

    private static void requireRedirectZero(Object manager) throws Exception {
        Object count = field(manager, "mRedirectRefCount");
        require(count instanceof AtomicInteger && ((AtomicInteger) count).get() == 0
                && field(manager, "mRedirectActivityComponent") == null);
    }

    private static void requireOnlyOriginalControlPackage(Object controller) throws Exception {
        Object value = field(controller, "mOmniPackageNames");
        require(value instanceof Set && value.getClass().getName().equals("java.util.concurrent.ConcurrentHashMap$KeySetView"));
        Object[] names = ((Set<?>) value).toArray();
        require(names.length == 1 && "com.stepos.ability".equals(names[0]));
    }

    private static void requireNoInputOwnership(Object controller) throws Exception {
        Object imeLock = field(controller, "mOmniImeLock");
        Object focusLock = field(controller, "mOmniInputFocusLock");
        synchronized (imeLock) { require(((SparseBooleanArray) field(controller, "mOmniImeEnabledDisplays")).size() == 0); }
        synchronized (focusLock) { require(((SparseBooleanArray) field(controller, "mKeepInputFocusDisplays")).size() == 0); }
    }

    private static TreeMap<Integer, AppState> readMap(Object controller) throws Exception {
        Object value = field(controller, "mAppInfoMap");
        require(value instanceof Map);
        TreeMap<Integer, AppState> result = new TreeMap<>();
        synchronized (value) {
            Map<?, ?> map = (Map<?, ?>) value;
            require(map.size() <= 256);
            for (Map.Entry<?, ?> entry : map.entrySet()) {
                require(entry.getKey() instanceof Integer && entry.getValue() != null);
                Object info = entry.getValue();
                int uid = (Integer) field(info, "uid");
                require(uid == (Integer) entry.getKey());
                result.put(uid, new AppState((String) field(info, "packageName"),
                        (Integer) field(info, "mShowingDisplayId"), (Integer) field(info, "mPreviousDisplayId")));
            }
        }
        return result;
    }

    private static final class AppState {
        final String name; final int current; final int previous;
        AppState(String name, int current, int previous) { this.name = name; this.current = current; this.previous = previous; }
        @Override public boolean equals(Object value) {
            if (!(value instanceof AppState)) return false;
            AppState other = (AppState) value;
            return java.util.Objects.equals(name, other.name) && current == other.current && previous == other.previous;
        }
        @Override public int hashCode() { return java.util.Objects.hash(name, current, previous); }
    }

    private static Object field(Object target, String name) throws Exception {
        Class<?> type = target.getClass();
        while (type != null) {
            try { Field field = type.getDeclaredField(name); field.setAccessible(true); return field.get(target); }
            catch (NoSuchFieldException absent) { type = type.getSuperclass(); }
        }
        throw new NoSuchFieldException(name);
    }

    private static void checkDeadline(AtomicBoolean expired, long deadline) {
        require(!expired.get() && SystemClock.uptimeMillis() <= deadline);
    }
    private static void require(boolean value) { if (!value) throw new SecurityException("Recovery guard failed"); }
    private static void putError(JSONObject out, Throwable error) {
        while (error instanceof InvocationTargetException && error.getCause() != null) error = error.getCause();
        try { out.put("ok", false).put("error_type", error.getClass().getSimpleName()); } catch (Exception ignored) { }
    }
    private static String readSmall(String name) throws Exception {
        try (FileInputStream stream = new FileInputStream(name)) {
            byte[] bytes = new byte[256]; int count = stream.read(bytes);
            return count <= 0 ? "" : new String(bytes, 0, count, StandardCharsets.UTF_8).replace("\u0000", "").trim();
        }
    }
}

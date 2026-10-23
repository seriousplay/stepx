package stepx;

import android.content.ComponentName;
import android.os.Process;
import org.json.JSONArray;
import org.json.JSONObject;
import java.io.FileInputStream;
import java.lang.reflect.Field;
import java.lang.reflect.InvocationTargetException;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Set;
import java.util.concurrent.atomic.AtomicInteger;

/**
 * Fixed, read-only system_server snapshot. No setter, omniCall, listener,
 * constructor, permission mutation, display action or persistent endpoint.
 *
 * The existing WMS bootstrap calls OmniManagerServiceStub.get().setWindowManager;
 * its registry maps the implementation as SINGLETON. Require mWm to be non-null
 * so a detached/uninitialized instance is never accepted as global evidence.
 *
 * Read only AtomicInteger, volatile ComponentName, and ConcurrentHashMap key-set.
 * Do not acquire private WMS locks from an injected android.ui thread. The result
 * is a bounded observation, not an atomic multi-field transaction; compare two
 * quiescent snapshots and the independent dumpsys display/task resources.
 * The injection transport retains its small code arena until reboot; no live
 * listener or background thread is installed by this class.
 */
public final class NativeGuiDisplaySnapshot {
    private static final String STUB = "com.android.server.wm.OmniManagerServiceStub";
    private static final String IMPL = "com.android.server.wm.StepOSOmniManagerImpl";

    public static String run() {
        JSONObject out = new JSONObject();
        try {
            out.put("version", 1).put("mode", "native_gui_display_snapshot")
                    .put("pid", Process.myPid()).put("uid", Process.myUid()).put("read_only", true);
            if (Process.myUid() != 1000 || !"system_server".equals(readSmall("/proc/self/cmdline"))
                    || !"u:r:system_server:s0".equals(readSmall("/proc/self/attr/current")))
                throw new SecurityException("Wrong snapshot process");
            Object manager = Class.forName(STUB).getMethod("get").invoke(null);
            if (manager == null || !IMPL.equals(manager.getClass().getName()) || field(manager, "mWm") == null)
                throw new SecurityException("Original WMS singleton is not initialized");
            Object counter = field(manager, "mRedirectRefCount");
            if (!(counter instanceof AtomicInteger)) throw new SecurityException("Unexpected counter ABI");
            AtomicInteger count = (AtomicInteger) counter;
            int first = count.get();
            Object componentValue = field(manager, "mRedirectActivityComponent");
            if (componentValue != null && !(componentValue instanceof ComponentName))
                throw new SecurityException("Unexpected component ABI");
            String component = componentValue == null ? null : ((ComponentName) componentValue).flattenToString();
            Object controller = field(manager, "mOmniDisplayController");
            if (controller == null || !"com.android.server.wm.StepOSOmniDisplayController".equals(controller.getClass().getName()))
                throw new SecurityException("Unexpected Omni controller ABI");
            Object setValue = field(controller, "mOmniPackageNames");
            if (!(setValue instanceof Set) || !setValue.getClass().getName().equals("java.util.concurrent.ConcurrentHashMap$KeySetView"))
                throw new SecurityException("Expected concurrent control package set");
            Object[] values = ((Set<?>) setValue).toArray();
            if (values.length > 64) throw new SecurityException("Unexpected control package count");
            String[] names = new String[values.length];
            for (int i = 0; i < values.length; i++) {
                if (!(values[i] instanceof String) || !((String) values[i]).matches("[A-Za-z0-9_.]{1,200}"))
                    throw new SecurityException("Invalid control package name");
                names[i] = (String) values[i];
            }
            Arrays.sort(names);
            JSONArray packages = new JSONArray();
            for (String name : names) packages.put(name);
            int last = count.get();
            Object lastComponent = field(manager, "mRedirectActivityComponent");
            boolean stable = first == last && java.util.Objects.equals(componentValue, lastComponent);
            out.put("ok", true).put("initialized_wms_singleton", true)
                    .put("redirect_ref_count", last).put("redirect_component", component == null ? JSONObject.NULL : component)
                    .put("stable_observation", stable).put("atomic_snapshot", false)
                    .put("control_packages", packages).put("control_package_count", names.length)
                    .put("ability_control_allowed", Arrays.asList(names).contains("com.stepos.ability"));
        } catch (Throwable error) {
            while (error instanceof InvocationTargetException && error.getCause() != null) error = error.getCause();
            try { out.put("ok", false).put("error_type", error.getClass().getSimpleName()); }
            catch (Exception ignored) { }
        }
        return out.toString();
    }

    private static Object field(Object target, String name) throws Exception {
        Field field = target.getClass().getDeclaredField(name);
        field.setAccessible(true);
        return field.get(target);
    }

    private static String readSmall(String name) throws Exception {
        try (FileInputStream stream = new FileInputStream(name)) {
            byte[] bytes = new byte[256]; int count = stream.read(bytes);
            return count <= 0 ? "" : new String(bytes, 0, count, StandardCharsets.UTF_8).replace("\u0000", "").trim();
        }
    }
}

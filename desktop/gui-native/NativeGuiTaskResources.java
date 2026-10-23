package stepx;

import android.content.Context;
import android.hardware.display.DisplayManager;
import android.hardware.display.VirtualDisplay;
import android.os.Looper;
import android.os.Process;
import android.view.Display;
import org.json.JSONArray;
import org.json.JSONObject;
import java.lang.reflect.Field;
import java.lang.reflect.InvocationTargetException;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/**
 * Read-only diagnostics for an exact task already owned by the research adapter.
 * Does not finish tasks, release displays, run SDK/service initialization, mutate
 * native fields or output task names, screen content, prompts, URLs or accounts.
 * No physical-disposal/rollback claim is inferred from a map entry disappearing.
 * Use after ordinary cleanup reports a remaining MDI entry, on the host main
 * thread. Observations may change between fields; they are not an atomic proof.
 */
public final class NativeGuiTaskResources {
    private NativeGuiTaskResources() { }

    public static JSONObject snapshot(Context host, String ownedTaskId) {
        JSONObject out = new JSONObject();
        try {
            out.put("version", 1).put("mode", "native_gui_owned_task_resources")
                    .put("read_only", true).put("atomic_snapshot", false)
                    .put("pid", Process.myPid()).put("uid", Process.myUid());
            require(host != null && "com.stepos.ability".equals(host.getPackageName())
                    && Process.myUid() == 1000 && ownedTaskId != null
                    && ownedTaskId.matches("stepx-settings-about-[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}"));
            require(Looper.myLooper() == Looper.getMainLooper());
            ClassLoader loader = host.getClassLoader();
            Class<?> manager = Class.forName("com.mdisdk.TaskManager", false, loader);
            Field registryField = manager.getDeclaredField("activeTasks");
            registryField.setAccessible(true);
            Object registryValue = registryField.get(null);
            require(registryValue instanceof ConcurrentHashMap);
            Map<?, ?> registry = (Map<?, ?>) registryValue;
            Object[] entries = registry.entrySet().toArray();
            require(entries.length <= 128);
            int ownedKeyCount = 0;
            Object owned = null;
            for (Object item : entries) {
                require(item instanceof Map.Entry);
                Map.Entry<?, ?> entry = (Map.Entry<?, ?>) item;
                require(entry.getKey() instanceof String && ((String) entry.getKey()).length() <= 256
                        && entry.getValue() != null);
                if (ownedTaskId.equals(entry.getKey())) { ownedKeyCount++; owned = entry.getValue(); }
            }
            require(ownedKeyCount <= 1);
            out.put("task_key_count", entries.length).put("other_task_key_count", entries.length - ownedKeyCount)
                    .put("owned_entry_present", owned != null);
            if (owned == null) return out.put("ok", true).put("physical_display_state_known", false);
            require("com.mdisdk.core.DefaultTaskImpl".equals(owned.getClass().getName()));
            Class<?> taskType = Class.forName("com.mdisdk.Task", false, loader);
            require(taskType.isInstance(owned) && ownedTaskId.equals(taskType.getMethod("getName").invoke(owned)));
            int displayId = (Integer) taskType.getMethod("getDisplayId").invoke(owned);
            require(displayId >= 90000000 && displayId < 100000000);
            Object finishing = field(owned, "isFinishing");
            require(finishing instanceof Boolean);
            Object context = field(owned, "taskContext");
            require(context != null && "com.mdisdk.core.TaskContext".equals(context.getClass().getName()));
            Object omni = field(context, "display");
            require(omni != null && "com.mdisdk.display.OmniDisplay".equals(omni.getClass().getName()));
            Object target = field(omni, "targetDisplay");
            require(target instanceof Display && ((Display) target).getDisplayId() == displayId);
            DisplayManager displays = host.getSystemService(DisplayManager.class);
            require(displays != null);
            Display actual = displays.getDisplay(displayId);
            out.put("owned_identity_verified", true).put("display_id", displayId)
                    .put("is_finishing", finishing).put("actual_display_present", actual != null)
                    .put("target_display_valid", ((Display) target).isValid())
                    .put("actual_display_valid", actual == null ? JSONObject.NULL : actual.isValid())
                    .put("physical_display_state_known", true);
            Object virtual = field(omni, "virtualDisplay");
            require(virtual == null || virtual instanceof VirtualDisplay);
            out.put("virtual_display_object_present", virtual != null);
            if (virtual != null) {
                Display virtualTarget = ((VirtualDisplay) virtual).getDisplay();
                require(virtualTarget != null && virtualTarget.getDisplayId() == displayId);
                out.put("virtual_display_target_valid", virtualTarget.isValid());
            }
            Object pipeline = field(omni, "renderPipeline");
            require(pipeline == null || pipeline instanceof Thread
                    && "com.mdisdk.gles.EglRenderPipeline".equals(pipeline.getClass().getName()));
            out.put("egl_thread_present", pipeline != null);
            if (pipeline != null) {
                Thread thread = (Thread) pipeline;
                Object running = field(pipeline, "isRunning");
                require(running instanceof Boolean);
                out.put("egl_thread_alive", thread.isAlive()).put("egl_thread_state", thread.getState().name())
                        .put("egl_running_flag", running);
            }
            // A concurrently replaced/removed entry is an inconsistent sample.
            require(registry.get(ownedTaskId) == owned && registry.size() == entries.length);
            out.put("same_owned_entry_after_read", true).put("ok", true);
        } catch (Throwable error) {
            while (error instanceof InvocationTargetException && error.getCause() != null) error = error.getCause();
            try {
                out.put("ok", false).put("error_type", error.getClass().getSimpleName());
                JSONArray frames = new JSONArray();
                for (StackTraceElement frame : error.getStackTrace()) {
                    if (frames.length() >= 5) break;
                    String name = frame.getClassName();
                    if (name.startsWith("stepx.") || name.startsWith("com.mdisdk.")
                            || name.startsWith("android.hardware.display.") || name.startsWith("android.view.Display"))
                        frames.put(name + "." + frame.getMethodName() + ":" + frame.getLineNumber());
                }
                out.put("error_frames", frames);
            } catch (Exception ignored) { }
        }
        return out;
    }

    /** Read-only registry classification; never calls SDK query/readiness methods. */
    public static JSONObject registrySnapshot(Context host) {
        JSONObject out = new JSONObject();
        try {
            require(host != null && host.getApplicationContext() == host
                    && "com.stepos.ability".equals(host.getPackageName()) && Process.myUid() == 1000
                    && Looper.myLooper() == Looper.getMainLooper());
            ClassLoader loader = host.getClassLoader();
            Class<?> manager = Class.forName("com.mdisdk.TaskManager", false, loader);
            Class<?> bridge = Class.forName("com.stepos.taskrobot.llmapi.stepfun.SystemSdkBridge", false, loader);
            require(manager.getClassLoader() == loader && bridge.getClassLoader() == loader);
            Object context = staticField(manager, "applicationContext");
            require(context == host || context == null);
            Map<?, ?> registry = (Map<?, ?>) staticField(manager, "activeTasks");
            require(registry instanceof ConcurrentHashMap && registry.size() <= 32);
            Object query = staticField(bridge, "queryTask");
            Object watcher = staticField(bridge, "mainDisplayWatcher");
            require(watcher != null && "com.stepos.taskrobot.llmapi.stepfun.MainDisplayWatcher".equals(watcher.getClass().getName()));
            Object watch = field(watcher, "watchTask");
            Object[] entries = registry.entrySet().toArray();
            JSONArray values = new JSONArray();
            int queries = 0, watches = 0, owned = 0, others = 0;
            for (Object entryValue : entries) {
                Map.Entry<?, ?> entry = (Map.Entry<?, ?>) entryValue;
                require(entry.getKey() instanceof String && entry.getValue() != null);
                String key = (String) entry.getKey();
                String category;
                if ("__query_task__".equals(key)) { category = "query"; queries++; }
                else if ("__main_display_watcher__".equals(key)) { category = "main_display_watcher"; watches++; }
                else if (key.matches("stepx-settings-about-[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")) {
                    category = "settings_test"; owned++;
                } else { category = "other"; others++; }
                Object task = entry.getValue();
                JSONObject value = new JSONObject().put("category", category)
                        .put("same_query_reference", task == query).put("same_watcher_reference", task == watch)
                        .put("default_task_class", "com.mdisdk.core.DefaultTaskImpl".equals(task.getClass().getName()))
                        .put("class_loader_matches", task.getClass().getClassLoader() == loader);
                if (value.optBoolean("default_task_class") && value.optBoolean("class_loader_matches")) {
                    Object display = task.getClass().getMethod("getDisplayId").invoke(task);
                    require(display instanceof Integer && (Integer) display >= 0 && (Integer) display < 100000000);
                    value.put("display_id", display).put("task_name_matches_key", key.equals(task.getClass().getMethod("getName").invoke(task)))
                            .put("is_finishing", field(task, "isFinishing"))
                            .put("execution_job_count", ((Map<?, ?>) field(task, "executionJobs")).size())
                            .put("active_execution_count", ((java.util.concurrent.atomic.AtomicInteger) field(task, "activeExecutions")).get());
                    Object taskContext = field(task, "taskContext");
                    require(taskContext != null && "com.mdisdk.core.TaskContext".equals(taskContext.getClass().getName())
                            && taskContext.getClass().getClassLoader() == loader);
                    Field displayField = taskContext.getClass().getDeclaredField("display"); displayField.setAccessible(true);
                    Object omni = displayField.get(taskContext);
                    boolean declared = "com.mdisdk.display.OmniDisplay".equals(displayField.getType().getName())
                            && displayField.getType().getClassLoader() == loader;
                    boolean concrete = omni != null && omni.getClass() == displayField.getType();
                    int getterCount = 0; boolean targetGetter = false, omniGetter = false;
                    for (java.lang.reflect.Method method : taskContext.getClass().getDeclaredMethods()) {
                        if ("getDisplay".equals(method.getName()) && method.getParameterTypes().length == 0) {
                            getterCount++;
                            targetGetter |= method.getReturnType() == Display.class;
                            omniGetter |= "com.mdisdk.display.OmniDisplay".equals(method.getReturnType().getName());
                        }
                    }
                    value.put("display_provenance", "task_context_private_display_field")
                            .put("omni_field_declared_type_matches", declared).put("omni_field_value_type_matches", concrete)
                            .put("task_context_owns_host", field(taskContext, "applicationContext") == host)
                            .put("display_accessor_variant_count", getterCount)
                            .put("has_target_display_accessor", targetGetter).put("has_omni_display_accessor", omniGetter);
                    if (declared && concrete) {
                        Object target = field(omni, "targetDisplay");
                        Object virtual = field(omni, "virtualDisplay");
                        value.put("target_display_object_present", target instanceof Display)
                                .put("virtual_display_object_present", virtual instanceof VirtualDisplay);
                        if (target instanceof Display) value.put("target_display_id", ((Display) target).getDisplayId())
                                .put("target_matches_task", display.equals(((Display) target).getDisplayId()));
                        if (virtual instanceof VirtualDisplay) {
                            Display virtualTarget = ((VirtualDisplay) virtual).getDisplay();
                            value.put("virtual_target_present", virtualTarget != null);
                            if (virtualTarget != null) value.put("virtual_display_id", virtualTarget.getDisplayId())
                                    .put("virtual_matches_task", display.equals(virtualTarget.getDisplayId()));
                        }
                        DisplayManager dm = host.getSystemService(DisplayManager.class);
                        Display actual = dm == null ? null : dm.getDisplay((Integer) display);
                        value.put("actual_display_present", actual != null);
                        if (actual != null) value.put("actual_display_type", Display.class.getMethod("getType").invoke(actual))
                                .put("actual_display_owner_uid", Display.class.getMethod("getOwnerUid").invoke(actual));
                    }
                }
                require(registry.get(key) == task);
                values.put(value);
            }
            require(registry.size() == entries.length);
            out.put("ok", true).put("read_only", true).put("atomic_snapshot", false)
                    .put("task_count", entries.length).put("query_count", queries).put("watcher_count", watches)
                    .put("settings_task_count", owned).put("other_count", others)
                    .put("query_reference_present", query != null).put("watcher_reference_present", watch != null)
                    .put("watcher_observer_present", field(watcher, "observerRegId") != null)
                    .put("watcher_timer_present", field(watcher, "settleTimeout") != null)
                    .put("factory_initialized", context != null).put("tasks", values);
        } catch (Throwable error) {
            while (error instanceof InvocationTargetException && error.getCause() != null) error = error.getCause();
            try { out.put("ok", false).put("error_type", error.getClass().getSimpleName()); }
            catch (Exception ignored) { }
        }
        return out;
    }

    private static Object staticField(Class<?> type, String name) throws Exception {
        Field field = type.getDeclaredField(name); field.setAccessible(true); return field.get(null);
    }

    private static Object field(Object target, String name) throws Exception {
        Class<?> type = target.getClass();
        while (type != null) {
            try { Field field = type.getDeclaredField(name); field.setAccessible(true); return field.get(target); }
            catch (NoSuchFieldException absent) { type = type.getSuperclass(); }
        }
        throw new NoSuchFieldException(name);
    }
    private static void require(boolean value) { if (!value) throw new SecurityException("Unexpected owned task resources"); }
}

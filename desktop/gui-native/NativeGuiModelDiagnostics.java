package stepx;

import android.content.Context;
import android.os.Looper;
import android.os.Process;
import org.json.JSONArray;
import org.json.JSONObject;
import java.lang.reflect.Field;
import java.lang.reflect.InvocationHandler;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.util.ArrayDeque;
import java.util.Collection;
import java.util.Map;

/**
 * Disposable-host observation only. Does not print or retain arguments, prompts,
 * screenshots, response content, account identifiers, URLs or exception messages.
 * Uses the original methods and original Kotlin continuation context. Returned
 * values and asynchronous Kotlin Result objects pass through unchanged. Java's
 * Proxy wraps a synchronous checked exception in UndeclaredThrowableException
 * when the OEM interface declares no throws; the diagnostic records the original
 * exception first, and the wrapper retains it as cause. Unchecked exceptions and
 * Errors pass through unchanged. Proxy identity also differs. This is temporary
 * research instrumentation, not a production model replacement.
 *
 * Pinned APK: c9f56f3078df96d8d726422b1c622a1c19fe8c7dec8cadd8537d9a6ecf513ac4.
 * ModelConfigInner.resolveModelCellForTask reads modelCellMap[modelId]. Its cloud
 * resolveLlmConfigForTask reuses the base LlmService. ZyqlGuiAgentTask.start uses
 * that resolver for the task override. Both paths are checked before installation.
 * No addModel/updateModelCell/setCurrentModel calls (they can persist preferences).
 */
public final class NativeGuiModelDiagnostics {
    private static final String CONFIG = "stepai.aios.guiagent.config.RuntimeConfig";
    private static final String LLM = "stepai.aios.guiagent.config.LlmConfig";
    private static final String CELL = "stepai.aios.guiagent.llm.parser.IModelCell";
    private static final String SERVICE = "stepai.aios.guiagent.llm.LlmService";
    private static final Object LOCK = new Object();
    private static final ArrayDeque<JSONObject> events = new ArrayDeque<>();
    private static boolean installed;
    private static boolean coverageVerified;
    private static boolean removed;
    private static Object runtime, oldRuntime, newRuntime, oldCell, cellProxy, oldService, serviceProxy;
    private static Object inner;
    private static Field currentCellField, runtimeConfigField;
    private static Map<Object, Object> modelMap;
    private static String modelId;
    private static Method updateRuntime;
    private static int requestCalls, requestGenerated, serviceCalls, serviceResponses, parseCalls, errors;

    private NativeGuiModelDiagnostics() { }

    public static void install(Context host) throws Exception {
        guard(host);
        synchronized (LOCK) { require(!installed && runtime == null); }
        ClassLoader loader = host.getClassLoader();
        requireIdle(loader);
        Class<?> manager = Class.forName("ua.a", false, loader);
        Field runtimeField = field(manager, "c");
        runtime = runtimeField.get(null);
        require(runtime != null && runtime.getClass().getName().equals("stepai.aios.guiagent.runtime.a"));
        for (Field candidate : runtime.getClass().getDeclaredFields()) {
            if (!candidate.getType().getName().equals(CONFIG)) continue;
            require(runtimeConfigField == null);
            candidate.setAccessible(true); runtimeConfigField = candidate;
        }
        require(runtimeConfigField != null);
        oldRuntime = runtimeConfigField.get(runtime);
        Object llm = get(oldRuntime, "getLlmConfig");
        require(llm != null && llm.getClass().getName().equals(LLM));
        Object defaultTask = get(oldRuntime, "getTaskConfig");
        // A second default-level override would require an additional reviewed route.
        require(defaultTask != null && get(defaultTask, "getLlmConfig") == null);
        modelId = (String) get(llm, "getModelId");
        oldCell = get(llm, "getModelCell"); oldService = get(llm, "getLlmService");
        require(modelId != null && oldCell != null && oldService != null);
        require(oldService.getClass().getName().equals("com.stepos.taskrobot.llmapi.stepfun.ZyqlStepService"));
        Class<?> innerType = Class.forName("stepai.aios.guiagent.llm.cell.config.ModelConfigInner", false, loader);
        inner = innerType.getField("INSTANCE").get(null);
        Object models = Class.forName("stepai.aios.guiagent.llm.cell.config.ModelConfigs", false, loader)
                .getField("instance").get(null);
        require(models == inner && Boolean.TRUE.equals(field(innerType, "initialized").get(null)));
        Object map = field(innerType, "modelCellMap").get(null);
        require(map instanceof Map && ((Map<?, ?>) map).size() <= 128);
        @SuppressWarnings("unchecked") Map<Object, Object> checkedMap = (Map<Object, Object>) map;
        modelMap = checkedMap;
        currentCellField = field(innerType, "currentModelCell");
        require(modelId.equals(get(inner, "getCurrentModelId")) && currentCellField.get(null) == oldCell
                && modelMap.get(modelId) == oldCell);
        Class<?> cellType = Class.forName(CELL, false, loader);
        Class<?> serviceType = Class.forName(SERVICE, false, loader);
        require(cellType.isInterface() && serviceType.isInterface());
        cellProxy = observer(loader, cellType, oldCell, false);
        serviceProxy = observer(loader, serviceType, oldService, true);
        Object newLlm = copy(llm, serviceProxy, get(llm, "getCompressor"), modelId, cellProxy,
                get(llm, "getStateCompressionConfig"));
        newRuntime = copy(oldRuntime, get(oldRuntime, "getSessionConfig"), get(oldRuntime, "getMaxConcurrentTasks"),
                get(oldRuntime, "getTracerConfig"), newLlm, defaultTask, get(oldRuntime, "getMaxFailureTolerance"));
        updateRuntime = runtime.getClass().getMethod("p", oldRuntime.getClass());
        boolean changed = false;
        try {
            requireIdle(loader);
            changed = true;
            modelMap.put(modelId, cellProxy); currentCellField.set(null, cellProxy);
            invoke(updateRuntime, runtime, newRuntime);
            require(runtimeConfigField.get(runtime) == newRuntime);
            Object resolved = innerType.getMethod("resolveLlmConfigForTask", String.class, llm.getClass())
                    .invoke(inner, modelId, newLlm);
            require(get(resolved, "getModelCell") == cellProxy && get(resolved, "getLlmService") == serviceProxy);
            synchronized (LOCK) { coverageVerified = true; installed = true; }
        } catch (Throwable failure) {
            if (changed) {
                if (modelMap.get(modelId) == cellProxy) modelMap.put(modelId, oldCell);
                if (currentCellField.get(null) == cellProxy) currentCellField.set(null, oldCell);
                invoke(updateRuntime, runtime, oldRuntime);
            }
            throw asException(failure);
        }
    }

    /** Optional after all native/MDI tasks are gone. Process exit also drops every proxy. */
    public static void remove(Context host) throws Exception {
        guard(host); requireIdle(host.getClassLoader());
        synchronized (LOCK) { if (!installed || removed) return; }
        require(runtimeConfigField.get(runtime) == newRuntime && modelMap.get(modelId) == cellProxy
                && currentCellField.get(null) == cellProxy);
        invoke(updateRuntime, runtime, oldRuntime);
        modelMap.put(modelId, oldCell); currentCellField.set(null, oldCell);
        synchronized (LOCK) { removed = true; installed = false; }
    }

    /** Safe, bounded metadata only; no native getters, calls or object toString(). */
    public static JSONObject snapshot() {
        synchronized (LOCK) {
            try {
                JSONArray copy = new JSONArray();
                for (JSONObject event : events) copy.put(new JSONObject(event.toString()));
                return new JSONObject().put("version", 1).put("installed", installed).put("removed", removed)
                        .put("task_override_coverage_verified", coverageVerified)
                        .put("request_call_count", requestCalls).put("request_generated_count", requestGenerated)
                        .put("service_chat_count", serviceCalls).put("service_response_count", serviceResponses)
                        .put("parse_action_count", parseCalls).put("error_count", errors).put("events", copy);
            } catch (Exception impossible) { return new JSONObject(); }
        }
    }

    private static Object observer(ClassLoader loader, Class<?> type, Object delegate, boolean service) {
        return Proxy.newProxyInstance(loader, new Class<?>[]{type}, (proxy, method, args) -> {
            String name = method.getName();
            if (method.getDeclaringClass() == Object.class) {
                if (name.equals("equals") && args != null && args.length == 1 && args[0] == proxy) return true;
                return invoke(method, delegate, args);
            }
            String stage = service && name.equals("chat") ? "SERVICE_CHAT"
                    : !service && name.equals("generateRequestPayload") ? "REQUEST_GENERATION"
                    : !service && name.equals("parseActionResponse") ? "ACTION_PARSE" : null;
            if (stage == null) return invoke(method, delegate, args);
            countEnter(stage); record(stage + "_ENTER", null);
            Object[] forwarded = args;
            if (args != null && args.length > 0) {
                Class<?> last = method.getParameterTypes()[args.length - 1];
                if (last.getName().equals("H7.c") && last.isInterface() && args[args.length - 1] != null) {
                    forwarded = args.clone();
                    Object continuation = args[args.length - 1];
                    forwarded[args.length - 1] = Proxy.newProxyInstance(loader, new Class<?>[]{last},
                            continuationObserver(continuation, stage));
                }
            }
            try {
                Object result = invoke(method, delegate, forwarded);
                if (result != null && result.getClass().getName().equals("kotlin.coroutines.intrinsics.CoroutineSingletons")) {
                    record(stage + "_SUSPENDED", null);
                } else {
                    countReturn(stage); record(stage + "_RETURN", null);
                }
                return result;
            } catch (Throwable failure) {
                record(stage + "_ERROR", failure); throw failure;
            }
        });
    }

    private static InvocationHandler continuationObserver(Object original, String stage) {
        return (proxy, method, args) -> {
            if (method.getName().equals("resumeWith") && args != null && args.length == 1) {
                Throwable failure = resultFailure(args[0]);
                if (failure != null) record(stage + "_ASYNC_ERROR", failure);
                else { countReturn(stage); record(stage + "_ASYNC_RETURN", null); }
            }
            // getContext and resumeWith retain the caller's exact context/result.
            return invoke(method, original, args);
        };
    }

    private static Throwable resultFailure(Object result) {
        if (result == null || !result.getClass().getName().equals("kotlin.Result$Failure")) return null;
        try {
            for (Field candidate : result.getClass().getDeclaredFields()) {
                if (!Throwable.class.isAssignableFrom(candidate.getType())) continue;
                candidate.setAccessible(true); return (Throwable) candidate.get(result);
            }
        } catch (Throwable ignored) { /* Observation must not alter coroutine propagation. */ }
        return null;
    }

    private static void countEnter(String stage) {
        synchronized (LOCK) {
            if (stage.equals("REQUEST_GENERATION")) requestCalls++;
            else if (stage.equals("SERVICE_CHAT")) serviceCalls++;
            else if (stage.equals("ACTION_PARSE")) parseCalls++;
        }
    }
    private static void countReturn(String stage) {
        synchronized (LOCK) {
            if (stage.equals("REQUEST_GENERATION")) requestGenerated++;
            else if (stage.equals("SERVICE_CHAT")) serviceResponses++;
        }
    }
    private static void record(String stage, Throwable failure) {
        try {
            JSONObject event = new JSONObject().put("stage", stage);
            if (failure != null) {
                Throwable exact = unwrap(failure);
                event.put("error_class", identifier(exact.getClass().getName()));
                JSONArray frames = new JSONArray();
                for (StackTraceElement frame : exact.getStackTrace()) {
                    String cl = frame.getClassName();
                    if (cl.startsWith("java.lang.reflect.") || cl.startsWith("stepx.NativeGuiModelDiagnostics")) continue;
                    frames.put(identifier(cl) + "." + identifier(frame.getMethodName()) + ":" + frame.getLineNumber());
                    if (frames.length() == 5) break;
                }
                event.put("error_frames", frames);
            }
            synchronized (LOCK) {
                if (failure != null) errors++;
                if (events.size() == 24) events.removeFirst();
                events.addLast(event);
            }
        } catch (Throwable ignored) { /* Never replace the original result with an observer failure. */ }
    }

    private static void guard(Context host) {
        require(host != null && Process.myUid() == 1000 && "com.stepos.ability".equals(host.getPackageName())
                && "stepx.NativeGuiTaskHost$HostApplication".equals(host.getClass().getName())
                && Looper.myLooper() == Looper.getMainLooper());
    }
    private static void requireIdle(ClassLoader loader) throws Exception {
        Class<?> sdk = Class.forName("com.stepos.taskrobot.llmapi.stepfun.SystemSdkBridge", false, loader);
        Object active = sdk.getMethod("listActiveTasks").invoke(sdk.getField("INSTANCE").get(null));
        require(active instanceof Collection && ((Collection<?>) active).isEmpty());
        Object mdi = field(Class.forName("com.mdisdk.TaskManager", false, loader), "activeTasks").get(null);
        require(mdi instanceof Map && ((Map<?, ?>) mdi).isEmpty());
    }
    private static Field field(Class<?> type, String name) throws Exception {
        Field f = type.getDeclaredField(name); f.setAccessible(true); return f;
    }
    private static Object get(Object target, String getter) throws Exception {
        require(target != null); return target.getClass().getMethod(getter).invoke(target);
    }
    private static Object copy(Object source, Object... args) throws Exception {
        Method selected = null;
        for (Method method : source.getClass().getMethods()) {
            if (method.getName().equals("copy") && method.getParameterCount() == args.length) {
                require(selected == null); selected = method;
            }
        }
        require(selected != null); return invoke(selected, source, args);
    }
    private static Object invoke(Method method, Object target, Object... args) throws Exception {
        try { return method.invoke(target, args); }
        catch (InvocationTargetException e) {
            Throwable actual = e.getCause();
            if (actual instanceof Error) throw (Error) actual;
            if (actual instanceof Exception) throw (Exception) actual;
            throw new RuntimeException(actual);
        }
    }
    private static Throwable unwrap(Throwable error) {
        while (error instanceof InvocationTargetException && error.getCause() != null) error = error.getCause();
        return error;
    }
    private static Exception asException(Throwable error) {
        Throwable e = unwrap(error); if (e instanceof Exception) return (Exception) e;
        if (e instanceof Error) throw (Error) e;
        return new RuntimeException(e);
    }
    private static String identifier(String value) {
        return value != null && value.length() <= 220 && value.matches("[A-Za-z0-9_.$<>-]+") ? value : "redacted";
    }
    private static void require(boolean condition) {
        if (!condition) throw new SecurityException("Native GUI observation identity or ABI guard failed");
    }
}

package stepx;

import android.content.Context;
import android.os.Looper;
import android.os.Process;
import java.lang.reflect.Field;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.nio.ByteBuffer;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.util.ArrayDeque;
import java.util.Collection;
import java.util.Locale;
import java.util.Map;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * Research-only AgentCall metadata observer in the disposable original-SDK host.
 * Keeps ApiManager.clientMap, BffCoreAgentClient, auth and network routing intact.
 * Only its already-created AgentCall handler is wrapped; original header, body
 * and callback objects are forwarded exactly once. No body/reason text, IDs,
 * arbitrary metadata keys/values, URLs, tokens or exception messages are emitted.
 * Header frame/msg types are reported numerically, not guessed to mean success.
 * Protobuf field structure is evidence of shape, not proof of authorization.
 * Only message type 4 with the exact two-field ErrorResp shape and code 10002
 * receives an in-memory reason keyword classification. It remains a candidate;
 * code 10002 has no established authoritative meaning in the pinned client.
 * A synchronous checked Throwable can acquire Java Proxy's standard wrapper.
 */
public final class NativeGuiEventDiagnostics {
    private static final String BFF = "com.stepos.agentframework.adapter.bff.";
    private static final Object LOCK = new Object();
    private static final ArrayDeque<JSONObject> events = new ArrayDeque<>();
    private static Map<Object, Object> handlers;
    private static Object key, original, observer;
    private static boolean installed, removed;
    private static int received, forwarded, observationFailures;

    private NativeGuiEventDiagnostics() { }

    public static void install(Context host) throws Exception {
        guard(host); requireIdle(host.getClassLoader());
        require(!installed && handlers == null);
        ClassLoader loader = host.getClassLoader();
        Class<?> api = Class.forName("com.stepos.agentframework.adapter.ApiManager", false, loader);
        Object clients = field(api, "clientMap").get(null);
        require(clients instanceof Map);
        Class<?> apiType = Class.forName("com.stepos.agentframework.adapter.netcore.ApiType", false, loader);
        Object client = ((Map<?, ?>) clients).get(apiType.getField("BFF").get(null));
        require(client != null && client.getClass().getName().equals(BFF + "BffCoreAgentClient"));
        // Access the initialized lazy delegate. Never initialize a second client.
        Object lazy = field(client.getClass(), "internalClient$delegate").get(client);
        require(lazy != null);
        Method initialized = Class.forName("C7.f", false, loader).getMethod("isInitialized");
        require(Boolean.TRUE.equals(initialized.invoke(lazy)));
        Object internal = client.getClass().getMethod("getInternalClient").invoke(client);
        require(internal != null && internal.getClass().getName().equals(BFF + "BffClient"));
        Object callbacks = call(internal, "getCallbackCache");
        require(callbacks instanceof Map && ((Map<?, ?>) callbacks).isEmpty());
        Object dispatcher = field(internal.getClass(), "commandDispatcher").get(internal);
        require(dispatcher != null && dispatcher.getClass().getName().equals(BFF + "BffCommandDispatcher"));
        Object values = field(dispatcher.getClass(), "handlers").get(dispatcher);
        require(values instanceof Map && ((Map<?, ?>) values).size() <= 32);
        @SuppressWarnings("unchecked") Map<Object, Object> checked = (Map<Object, Object>) values;
        handlers = checked;
        for (Map.Entry<Object, Object> entry : handlers.entrySet()) {
            Object value = entry.getValue();
            if (value != null && value.getClass().getName().equals(BFF + "handler.AgentCallCmdHandler")) {
                require(original == null && entry.getKey() instanceof String);
                key = entry.getKey(); original = value;
            }
        }
        require(original != null && field(original.getClass(), "bffClient").get(original) == internal);
        Class<?> handler = Class.forName(BFF + "handler.IBffCommandHandler", false, loader);
        require(handler.isInterface());
        observer = makeObserver(loader, handler, original);
        requireIdle(loader);
        require(handlers.get(key) == original);
        try {
            handlers.put(key, observer);
            require(handlers.get(key) == observer && ((Map<?, ?>) clients).get(apiType.getField("BFF").get(null)) == client);
        } catch (Throwable failure) {
            if (handlers.get(key) == observer) handlers.put(key, original);
            throw failure;
        }
        synchronized (LOCK) { installed = true; }
    }

    private static Object makeObserver(ClassLoader loader, Class<?> handler, Object delegate) {
        return Proxy.newProxyInstance(loader, new Class<?>[]{handler}, (proxy, method, args) -> {
            if (method.getName().equals("handle") && args != null && args.length == 3) {
                synchronized (LOCK) { received++; }
                observe(loader, args);
                synchronized (LOCK) { forwarded++; }
            }
            try { return method.invoke(delegate, args); }
            catch (InvocationTargetException error) { throw error.getCause(); }
        });
    }

    public static void remove(Context host) throws Exception {
        guard(host); requireIdle(host.getClassLoader());
        if (!installed || removed) return;
        require(handlers.get(key) == observer); handlers.put(key, original);
        synchronized (LOCK) { installed = false; removed = true; }
    }

    public static JSONObject snapshot() {
        synchronized (LOCK) {
            try {
                JSONArray result = new JSONArray();
                for (JSONObject event : events) result.put(new JSONObject(event.toString()));
                return new JSONObject().put("version", 1).put("installed", installed).put("removed", removed)
                        .put("received_count", received).put("forwarded_count", forwarded)
                        .put("observation_failure_count", observationFailures).put("events", result);
            } catch (Exception impossible) { return new JSONObject(); }
        }
    }

    private static void observe(ClassLoader loader, Object[] args) {
        JSONObject event = new JSONObject();
        try {
            Object head = args[0], body = args[1], cache = args[2];
            require(head != null && head.getClass().getName().equals("r7.f") && body != null);
            Class<?> byteString = Class.forName("com.google.protobuf.ByteString", false, loader);
            require(byteString.isInstance(body));
            event.put("stage", "AGENT_CALL_RECEIVED").put("frame_type", call(head, "W"))
                    .put("message_type", call(head, "d0"))
                    .put("command_matches_handler", key.equals(call(head, "T")))
                    .put("module_is_client_agent", "ClientAgent".equals(call(head, "Z")))
                    .put("module_is_gui", "Gui".equals(call(head, "Z")))
                    .put("module_empty", "".equals(call(head, "Z")))
                    .put("callback_cache_present", cache != null);
            Object meta = call(head, "X");
            require(meta instanceof Map);
            event.put("header_meta_count", ((Map<?, ?>) meta).size());
            JSONObject codes = new JSONObject();
            for (String name : new String[]{"code", "error_code", "status", "status_code", "errorCode", "statusCode"}) {
                Object value = ((Map<?, ?>) meta).get(name);
                if (!(value instanceof String)) continue;
                String text = (String) value;
                if (text.matches("-?[0-9]{1,9}")) codes.put(name, Long.parseLong(text));
            }
            event.put("numeric_header_codes", codes);
            int length = ((Number) byteString.getMethod("size").invoke(body)).intValue();
            event.put("raw_body_length", length);
            if (length >= 0 && length <= 1048576) {
                byte[] data = (byte[]) byteString.getMethod("o").invoke(body);
                wireShape(data, event);
                classifyErrorCandidate(data, ((Number) call(head, "d0")).intValue(), event);
                try {
                    Class<?> completion = Class.forName("p7.b", false, loader);
                    Object parsed = completion.getMethod("H", byte[].class).invoke(null, (Object) data);
                    Object innerBody = completion.getMethod("G").invoke(parsed);
                    event.put("completion_proto_parse_ok", true)
                            .put("completion_body_length", byteString.getMethod("size").invoke(innerBody));
                } catch (Throwable parseFailure) {
                    event.put("completion_proto_parse_ok", false)
                            .put("completion_parse_error_class", errorClass(parseFailure));
                }
            } else { event.put("body_inspection_skipped", true); }
        } catch (Throwable failure) {
            synchronized (LOCK) { observationFailures++; }
            try { event.put("stage", "EVENT_OBSERVATION_ERROR").put("error_class", errorClass(failure)); }
            catch (Exception ignored) { }
        }
        synchronized (LOCK) {
            if (events.size() == 12) events.removeFirst();
            events.addLast(event);
        }
    }

    // Reads only tags and sizes. The fixed ErrorResp schema is field1:int32,
    // field2:string; expose its numeric first field as a candidate, not a verdict.
    private static void wireShape(byte[] data, JSONObject out) throws Exception {
        int[] cursor = {0}; JSONArray fields = new JSONArray(); boolean complete = true;
        Long candidateCode = null; boolean reasonField = false; int fieldCount = 0;
        while (cursor[0] < data.length && fieldCount < 16) {
            long tag = varint(data, cursor); int wire = (int) (tag & 7), field = (int) (tag >>> 3);
            require(field > 0 && field <= 536870911); fieldCount++;
            if (fields.length() < 8) fields.put(new JSONObject().put("field", field).put("wire", wire));
            if (wire == 0) {
                long value = varint(data, cursor);
                if (field == 1 && candidateCode == null) candidateCode = (long) (int) value;
            } else if (wire == 1) skip(data, cursor, 8);
            else if (wire == 2) {
                long length = varint(data, cursor); require(length >= 0 && length <= data.length);
                if (field == 2) { reasonField = true; out.put("error_reason_field_length", length); }
                skip(data, cursor, (int) length);
            } else if (wire == 5) skip(data, cursor, 4);
            else { complete = false; break; }
        }
        if (cursor[0] != data.length) complete = false;
        out.put("wire_fields", fields).put("wire_scan_complete", complete);
        if (candidateCode != null && reasonField) out.put("error_schema_candidate_code", candidateCode);
    }

    // Pinned r7.d ErrorResp: field 1 int32 code (J), field 2 UTF-8 reason (K).
    // Strictly require exactly those two fields in that order, no extra fields,
    // no duplicates, a small well-formed UTF-8 reason, and the observed error
    // message type/code. Never retain its decoded reason or quote any substring.
    private static void classifyErrorCandidate(byte[] data, int messageType, JSONObject out) throws Exception {
        if (messageType != 4 || data.length < 5 || data.length > 4112) return;
        final String reason;
        try {
            int[] cursor = {0};
            if (varint(data, cursor) != 8 || varint(data, cursor) != 10002
                    || varint(data, cursor) != 18) return;
            long length = varint(data, cursor);
            if (length < 0 || length > 4096 || length != data.length - cursor[0]) return;
            reason = StandardCharsets.UTF_8.newDecoder()
                    .onMalformedInput(CodingErrorAction.REPORT).onUnmappableCharacter(CodingErrorAction.REPORT)
                    .decode(ByteBuffer.wrap(data, cursor[0], (int) length)).toString().toLowerCase(Locale.ROOT);
        } catch (RuntimeException | java.nio.charset.CharacterCodingException invalidShape) { return; }

        boolean model = has(reason, "model", "模型");
        boolean notFound = has(reason, "not found", "not_found", "notfound", "does not exist", "不存在", "未找到");
        boolean unknownRoute = has(reason, "unknown route", "unknown_route", "route not found", "no route", "unknown method", "method not found");
        boolean invalidParameter = has(reason, "invalid parameter", "invalid_parameter", "invalid argument", "invalid_argument", "invalid request", "bad request", "参数错误", "参数无效");
        boolean unauthorized = has(reason, "unauthorized", "unauthenticated", "authentication failed", "invalid credential", "未认证", "未登录");
        boolean forbidden = has(reason, "forbidden", "permission denied", "permission_denied", "access denied", "not authorized", "not permitted", "没有权限", "无权限");
        boolean unimplemented = has(reason, "unimplemented", "not implemented");
        boolean unknownService = has(reason, "unknown service", "unknown_service", "service not found");
        boolean unavailable = has(reason, "unavailable", "not available", "不可用");
        boolean connectionRefused = has(reason, "connection refused", "connection_refused", "connect: refused");
        boolean noHealthyUpstream = has(reason, "no healthy upstream", "no_healthy_upstream");
        boolean routingTimeout = has(reason, "routing timeout", "route timeout", "upstream timeout", "deadline exceeded", "deadline_exceeded", "connection timed out", "request timed out");
        boolean rateLimited = has(reason, "rate limit", "rate_limit", "too many requests", "resource exhausted", "resource_exhausted", "限流");
        boolean serverError = has(reason, "internal server", "internal error", "server error", "bad gateway", "服务器错误");
        JSONObject keywords = new JSONObject().put("model", model).put("not_found", notFound)
                .put("unknown_route", unknownRoute).put("invalid_parameter", invalidParameter)
                .put("unauthorized", unauthorized).put("forbidden", forbidden)
                .put("unimplemented", unimplemented).put("unknown_service", unknownService)
                .put("unavailable", unavailable).put("connection_refused", connectionRefused)
                .put("no_healthy_upstream", noHealthyUpstream).put("routing_timeout", routingTimeout)
                .put("rate_limited", rateLimited).put("server_error", serverError);
        String category = unauthorized ? "unauthorized" : forbidden ? "forbidden"
                : invalidParameter ? "invalid_parameter" : rateLimited ? "rate_limited"
                : unimplemented || unknownService ? "rpc_unimplemented"
                : model && (notFound || unavailable) ? "model_unavailable"
                : unknownRoute || unavailable || connectionRefused || noHealthyUpstream || routingTimeout ? "route_unavailable"
                : serverError ? "server_error" : "unclassified";
        out.put("reason_classification_candidate", category).put("reason_keyword_candidates", keywords);

        // Deliberately weak substring evidence, NOT a claim that the missing
        // object has this type. Only fixed labels are emitted, never matched text.
        JSONObject resources = new JSONObject();
        for (String name : new String[]{"agent", "task", "session", "conversation", "handler", "config",
                "processor", "service", "endpoint", "instance", "resource", "namespace", "cluster",
                "tenant", "deployment", "app", "device", "account", "user", "permission", "token", "key", "model"}) {
            resources.put(name, reason.contains(name));
        }
        // Separate complete fixed phrases from the weak substring candidates.
        JSONObject missingPhrases = new JSONObject();
        for (String name : new String[]{"agent", "task", "session", "conversation", "handler", "config",
                "processor", "service", "endpoint", "instance", "resource", "namespace", "cluster",
                "tenant", "deployment", "app", "device", "account", "user", "permission", "token", "key", "model"}) {
            missingPhrases.put(name + "_not_found", hasWholePhrase(reason,
                    name + " not found", name + "_not_found", name + " does not exist", "no such " + name));
        }
        // These spellings are fixed public-in-APK constants, never account or
        // request identifiers. No new value is learned or emitted from reason.
        JSONObject knownConfig = new JSONObject()
                .put("mentions_default_model_id", reason.contains("step3p7-mtp3-0520-mm-reasoning-effort-fp8-aos-prod"))
                .put("mentions_gui", hasWholePhrase(reason, "gui"))
                .put("mentions_invoke", hasWholePhrase(reason, "invoke"))
                .put("mentions_client_agent", hasWholePhrase(reason, "clientagent", "client_agent", "client agent"))
                .put("mentions_agent_call", hasWholePhrase(reason, "agentcall", "agent_call", "agent call"))
                .put("grpc_not_found", hasWholePhrase(reason, "rpc error: code = notfound", "rpc error: code=notfound",
                        "grpc code = notfound", "grpc code=notfound"));
        out.put("reason_resource_candidates", resources).put("reason_missing_phrase_candidates", missingPhrases)
                .put("reason_known_config_candidates", knownConfig);
    }

    private static boolean has(String text, String... fixedPhrases) {
        for (String phrase : fixedPhrases) if (text.contains(phrase)) return true;
        return false;
    }

    private static boolean hasWholePhrase(String text, String... fixedPhrases) {
        for (String phrase : fixedPhrases) {
            int at = text.indexOf(phrase);
            while (at >= 0) {
                int end = at + phrase.length();
                if ((at == 0 || !identifierChar(text.charAt(at - 1)))
                        && (end == text.length() || !identifierChar(text.charAt(end)))) return true;
                at = text.indexOf(phrase, at + 1);
            }
        }
        return false;
    }

    private static boolean identifierChar(char c) {
        return c >= 'a' && c <= 'z' || c >= '0' && c <= '9' || c == '_';
    }
    private static long varint(byte[] bytes, int[] cursor) {
        long value = 0;
        for (int shift = 0; shift < 64; shift += 7) {
            require(cursor[0] < bytes.length); int b = bytes[cursor[0]++] & 255;
            value |= ((long) (b & 127)) << shift;
            if ((b & 128) == 0) return value;
        }
        throw new IllegalArgumentException("Invalid protobuf varint");
    }
    private static void skip(byte[] bytes, int[] cursor, int count) {
        require(count >= 0 && count <= bytes.length - cursor[0]); cursor[0] += count;
    }
    private static String errorClass(Throwable error) {
        while (error instanceof InvocationTargetException && error.getCause() != null) error = error.getCause();
        String name = error.getClass().getName();
        return name.length() <= 220 && name.matches("[A-Za-z0-9_.$]+") ? name : "redacted";
    }
    private static Object call(Object target, String method) throws Exception {
        return target.getClass().getMethod(method).invoke(target);
    }
    private static Field field(Class<?> type, String name) throws Exception {
        Field f = type.getDeclaredField(name); f.setAccessible(true); return f;
    }
    private static void guard(Context host) {
        require(host != null && Process.myUid() == 1000 && "com.stepos.ability".equals(host.getPackageName())
                && "stepx.NativeGuiTaskHost$HostApplication".equals(host.getClass().getName())
                && Looper.myLooper() == Looper.getMainLooper());
    }
    private static void requireIdle(ClassLoader loader) throws Exception {
        Class<?> sdk = Class.forName("com.stepos.taskrobot.llmapi.stepfun.SystemSdkBridge", false, loader);
        Object tasks = sdk.getMethod("listActiveTasks").invoke(sdk.getField("INSTANCE").get(null));
        require(tasks instanceof Collection && ((Collection<?>) tasks).isEmpty());
    }
    private static void require(boolean okay) {
        if (!okay) throw new SecurityException("Native GUI event observer identity or ABI guard failed");
    }
}

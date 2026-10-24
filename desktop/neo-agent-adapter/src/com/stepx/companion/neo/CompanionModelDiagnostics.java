package com.stepx.companion.neo;

import org.json.JSONArray;
import org.json.JSONObject;
import java.util.WeakHashMap;

/** Response metadata only; never stores text, reasoning, screenshots or credentials. */
final class CompanionModelDiagnostics {
    private static final WeakHashMap<ModelClient, JSONObject> replies = new WeakHashMap<>();
    static void configure(Config cfg, JSONObject body, String system, boolean visual) throws Exception {
        if (!CompanionModelPolicy.step37(cfg.endpoint, cfg.model, cfg.protocol)) return;
        body.put("reasoning_effort", CompanionModelPolicy.effort(visual));
        // The plain connection probe still requests prose. Planning/action
        // prompts explicitly require JSON and keep local schema validation.
        if (system != null && system.contains("JSON")) body.put("response_format", new JSONObject().put("type", "json_object"));
    }
    static void inspect(ModelClient client, Config cfg, JSONObject response, int limit, long beganAt) throws Exception {
        String finish = ""; JSONObject message = new JSONObject();
        if ("Anthropic".equals(cfg.protocol)) finish = response.optString("stop_reason");
        else if ("Gemini".equals(cfg.protocol)) {
            JSONArray choices = response.optJSONArray("candidates");
            if (choices != null && choices.length() > 0) finish = choices.getJSONObject(0).optString("finishReason");
        } else {
            JSONArray choices = response.optJSONArray("choices");
            if (choices != null && choices.length() > 0) {
                JSONObject choice = choices.getJSONObject(0); finish = choice.optString("finish_reason");
                if (choice.optJSONObject("message") != null) message = choice.getJSONObject("message");
            }
        }
        JSONObject stats = new JSONObject().put("requestStartedAt", beganAt).put("receivedAt", System.currentTimeMillis())
            .put("finishReason", finish.matches("[A-Za-z_]{0,40}") ? finish : "other")
            .put("maxTokens", limit).put("truncated", CompanionModelPolicy.truncated(finish))
            .put("contentCharacters", message.optString("content", "").length())
            .put("reasoningCharacters", message.optString("reasoning_content", message.optString("reasoning", "")).length());
        JSONObject usage = response.optJSONObject("usage");
        if (usage != null) for (String field : new String[]{"prompt_tokens", "completion_tokens", "total_tokens", "input_tokens", "output_tokens"})
            if (usage.opt(field) instanceof Number) stats.put(field, usage.get(field));
        synchronized (replies) { replies.put(client, stats); }
        if (CompanionModelPolicy.truncated(finish)) throw new Exception("模型输出达到 token 上限，候选动作未执行；原目标保留，请重新发起或调整模型");
        if ("content_filter".equals(finish) || "refusal".equals(finish)) throw new Exception("模型拒绝返回动作，原目标保留，未派发操作");
        if (!"Anthropic".equals(cfg.protocol) && !"Gemini".equals(cfg.protocol) && message.optString("content", "").trim().isEmpty())
            throw new Exception("模型没有返回可用动作内容，原目标保留，未派发操作");
    }
    static JSONObject snapshot(ModelClient client, long notBefore) throws Exception {
        synchronized (replies) {
            JSONObject value = replies.get(client);
            return value == null || value.optLong("requestStartedAt") < notBefore ? new JSONObject() : new JSONObject(value.toString());
        }
    }
}

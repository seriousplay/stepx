package com.stepx.companion.neo;

import org.json.JSONObject;
import java.util.WeakHashMap;

/** Bounded, in-memory diagnostics. The underlying authorization policy is unchanged. */
final class CompanionInputDiagnostics {
    private static final WeakHashMap<ControlService, Entry> entries = new WeakHashMap<>();
    private static final class Entry {
        final String run;
        JSONObject candidate = new JSONObject(), search = new JSONObject();
        Entry(String run) { this.run = run; }
    }
    private static Entry entry(ControlService owner) {
        Entry e = entries.get(owner);
        if (e == null || !e.run.equals(owner.runId)) { e = new Entry(owner.runId); entries.put(owner, e); }
        return e;
    }
    private static String safe(ControlService owner, String text, int limit) {
        String key = owner.runConfig == null ? "" : owner.runConfig.key;
        return BridgePolicy.diagnosticText(PrivateText.redact(BridgePolicy.diagnosticText(text, key, 12000)), "", limit);
    }
    static void candidate(ControlService owner, Action action) {
        final String run=owner.runId,name=action.name;final long generation=owner.generation;
        owner.main.post(()->{if(generation==owner.generation&&run.equals(owner.runId)&&owner.running)owner.controlBar.proposal(name);});
        if (!"type".equals(action.name)) return;
        try {
            String text = action.data.optString("text");
            JSONObject value = new JSONObject().put("sampledAt", System.currentTimeMillis()).put("action", "type")
                .put("text", safe(owner, text, 160)).put("characters", text.length()).put("hasNodeId", action.data.has("id"));
            synchronized (entries) { entry(owner).candidate = value; }
        } catch (Exception ignored) {}
    }
    static boolean searchAllowed(ControlService owner, String goal, String original, String target, String pkg, String text, boolean independent) {
        // Record exactly the inputs used in the real decision, not a second,
        // reconstructed policy that could disagree with the dispatch boundary.
        boolean allowed = SearchQueryPolicy.allowed(goal, original, target, pkg, text, independent);
        try {
            JSONObject value = new JSONObject().put("sampledAt", System.currentTimeMillis()).put("allowed", allowed)
                .put("text", safe(owner, text, 160)).put("characters", text == null ? -1 : text.length())
                .put("goal", safe(owner, goal, 500)).put("original", safe(owner, original, 500)).put("target", safe(owner, target, 500))
                .put("package", pkg).put("independentSearchFields", independent)
                .put("empty", text == null || text.trim().isEmpty()).put("over120", text != null && text.length() > 120)
                .put("containsControl", text != null && text.matches("(?s).*[\\p{Cntrl}\\u200B-\\u200F\\u202A-\\u202E].*"))
                .put("containsPrivateText", text != null && (PrivateText.credential(text) || !PrivateText.redact(text).equals(text)))
                .put("targetPresent", target != null && !target.trim().isEmpty())
                .put("normalizedGoalContainsQuery", text != null && SearchQueryPolicy.normalized(goal).contains(SearchQueryPolicy.normalized(text)));
            synchronized (entries) { entry(owner).search = value; }
        } catch (Exception ignored) {}
        return allowed;
    }
    static JSONObject snapshot(ControlService owner) throws Exception {
        synchronized (entries) {
            Entry e = entries.get(owner);
            if (e == null || !e.run.equals(owner.runId)) return new JSONObject();
            return new JSONObject().put("candidate", new JSONObject(e.candidate.toString())).put("searchPolicy", new JSONObject(e.search.toString()));
        }
    }
}

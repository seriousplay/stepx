package com.stepx.companion.neo;

import android.view.accessibility.AccessibilityNodeInfo;
import org.json.JSONArray;
import org.json.JSONObject;
import java.util.WeakHashMap;

/** Read-only proof taken from the exact validated observation at model finish. */
final class CompanionCompletion {
    private static final WeakHashMap<ControlService, Ticket> tickets = new WeakHashMap<>();
    private static final class Ticket {
        final String session;
        final JSONObject wanted;
        String run = "";
        long generation;
        TaskWorkflow workflow;
        int step = -1;
        JSONObject proof;
        Ticket(String session, JSONObject wanted) { this.session = session; this.wanted = wanted; }
    }
    static void begin(ControlService service, String session, JSONObject wanted) throws Exception {
        synchronized (tickets) {
            tickets.put(service, new Ticket(session, wanted == null ? null : new JSONObject(wanted.toString())));
        }
    }
    private static boolean visible(AccessibilityNodeInfo root, String text, boolean exact) {
        if(root==null)return false;
        boolean found=false;
        for(AccessibilityNodeInfo node:root.findAccessibilityNodeInfosByText(text)) {
            try {
                CharSequence label=node.getText(), description=node.getContentDescription();
                if(node.isVisibleToUser()&&!node.isPassword()&&((label!=null&&(exact?label.toString().equals(text):label.toString().contains(text)))
                    ||(description!=null&&(exact?description.toString().equals(text):description.toString().contains(text)))))found=true;
            } finally {node.recycle();}
        }
        return found;
    }
    static boolean capture(ControlService service, ControlService.Observation observation, long generation) throws Exception {
        Ticket ticket;
        synchronized (tickets) { ticket = tickets.get(service); }
        if (ticket == null || !ticket.session.equals(service.ownerSession)) return true;
        // Called on the Android main thread immediately after the executor's
        // own validateObservation, before it ends ownership or hides overlays.
        service.validateObservation(observation, generation);
        NativeWindowState state = NativeWindowState.inspect(service);
        AccessibilityNodeInfo root = state.root;
        JSONObject wanted = ticket.wanted;
        String expectedPackage = wanted == null ? service.allowed : wanted.getString("package");
        boolean frame = state.expected && !state.covered && root != null && expectedPackage.contentEquals(root.getPackageName())
            && root.getWindowId() == observation.windowId;
        JSONArray checks = new JSONArray(), texts = wanted == null ? new JSONArray() : wanted.getJSONArray("texts");
        boolean matched = frame && texts.length() > 0;
        for (int i = 0; i < texts.length(); i++) {
            String text = texts.getString(i); boolean found = false;
            if (frame) found=visible(root,text,false);
            checks.put(new JSONObject().put("text", text).put("visible", found)); matched &= found;
        }
        if(wanted!=null&&"android_version".equals(wanted.optString("preset"))&&"com.android.settings".equals(expectedPackage)) {
            String[] anchors={"Android 版本",android.os.Build.VERSION.RELEASE,"关于手机","系统版本","Android 安全补丁日期","基带版本"};
            boolean[] seen=new boolean[anchors.length];checks=new JSONArray();
            for(int i=0;i<anchors.length;i++) {seen[i]=frame&&visible(root,anchors[i],i==1);checks.put(new JSONObject().put("text",anchors[i]).put("exact",i==1).put("visible",seen[i]));}
            matched=CompanionCompletionRules.androidVersion(frame,seen[0],seen[1],seen[2],seen[3],seen[4],seen[5]);
        }
        service.validateObservation(observation, generation);
        JSONObject proof = new JSONObject().put("kind", "caller_expected_finish_screen").put("package", expectedPackage)
            .put("rootPackage", root == null || root.getPackageName() == null ? "" : root.getPackageName().toString())
            .put("windowId", root == null ? -1 : root.getWindowId()).put("observationWindowId", observation.windowId)
            .put("expected", state.expected).put("covered", state.covered).put("observation", observation.id.toString())
            .put("rootChildren", root == null ? 0 : root.getChildCount()).put("checks", checks).put("matched", matched)
            .put("sampledAt", System.currentTimeMillis()).put("generation", generation);
        synchronized (tickets) {
            if (tickets.get(service) != ticket || !ticket.session.equals(service.ownerSession)) return true;
            ticket.run = service.runId; ticket.generation = generation; ticket.workflow = service.workflow;
            ticket.step = service.workflow == null ? -1 : service.workflow.index; ticket.proof = proof;
        }
        return true;
    }
    /** Caller holds the service monitor; never performs a fresh screen read. */
    static JSONObject finished(ControlService service) throws Exception {
        synchronized (tickets) {
            Ticket ticket = tickets.get(service);
            boolean same = ticket != null && ticket.proof != null && ticket.session.equals(service.ownerSession)
                && ticket.run.equals(service.runId) && ticket.generation == service.generation && ticket.workflow == service.workflow
                && ticket.step == (service.workflow == null ? -1 : service.workflow.index)
                && !service.running && !service.paused && !service.busy() && service.ownership.state() == RunOwnership.State.ENDED;
            return same ? new JSONObject(ticket.proof.toString()).put("finishBound", true)
                : new JSONObject().put("kind", "caller_expected_finish_screen").put("matched", false).put("finishBound", false)
                    .put("reason", "本次结束时没有可用的同任务屏幕证据，不采用后续页面补判完成");
        }
    }
}

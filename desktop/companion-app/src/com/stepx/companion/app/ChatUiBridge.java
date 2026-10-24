package com.stepx.companion.app;

import android.accessibilityservice.GestureDescription;
import android.app.KeyguardManager;
import android.content.Context;
import android.content.Intent;
import android.graphics.Path;
import android.graphics.Rect;
import android.os.Bundle;
import android.os.PowerManager;
import android.os.SystemClock;
import android.util.DisplayMetrics;
import android.view.accessibility.AccessibilityNodeInfo;
import org.json.JSONObject;
import java.util.ArrayList;
import java.util.Collections;
import java.util.Comparator;
import java.util.List;
import java.util.Random;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;

/**
 * Accessibility-based chat window reader and fallback sender (neo-agent-touch style)
 * with Human-in-Use touch guard and randomized TouchPointPicker button tap.
 */
public final class ChatUiBridge {
    private static final long HUMAN_TOUCH_GUARD_MS = 10_000L;
    private static final Random RANDOM = new Random();

    private ChatUiBridge() {}

    private static final class PositionedNode {
        final String text;
        final Rect bounds;

        PositionedNode(String text, Rect bounds) {
            this.text = text;
            this.bounds = bounds;
        }
    }

    /**
     * Reads the active foreground chat window, extracts conversation title (top bar)
     * and message bubbles (middle region), and merges them into PushStore.
     */
    public static JSONObject captureActiveChatWindow(Context context) throws Exception {
        GuiAccessibilityService service = GuiAccessibilityService.instance;
        if (service == null) {
            throw new IllegalStateException("请先启用 StepX伴侣的看屏操作服务");
        }
        AccessibilityNodeInfo root = service.getRootInActiveWindow();
        if (root == null) {
            throw new IllegalStateException("无法读取当前手机屏幕窗口，请确认手机已解锁并打开聊天页面");
        }

        String pkg = root.getPackageName() != null ? root.getPackageName().toString() : "";
        DisplayMetrics dm = service.getResources().getDisplayMetrics();
        int screenWidth = Math.max(1, dm.widthPixels);
        int screenHeight = Math.max(1, dm.heightPixels);

        List<PositionedNode> allTextNodes = new ArrayList<PositionedNode>();
        collectVisibleTextNodes(root, allTextNodes, 0);

        int topBarLimit = (int) (screenHeight * 0.18);
        int bubbleTopMin = (int) (screenHeight * 0.15);
        int bubbleBottomMax = (int) (screenHeight * 0.85);

        String detectedTitle = extractTopTitle(allTextNodes, topBarLimit, screenWidth);
        if (detectedTitle.isEmpty()) {
            detectedTitle = ImAppCatalog.appDisplayName(context, pkg);
        } else {
            detectedTitle = ImAppCatalog.cleanConversationTitle(pkg, detectedTitle);
        }

        List<PositionedNode> middleNodes = new ArrayList<PositionedNode>();
        for (PositionedNode pn : allTextNodes) {
            if (pn.bounds.top >= bubbleTopMin && pn.bounds.bottom <= bubbleBottomMax) {
                if (!isUiChromeOrTimestamp(pn.text)) {
                    middleNodes.add(pn);
                }
            }
        }

        Collections.sort(middleNodes, new Comparator<PositionedNode>() {
            @Override
            public int compare(PositionedNode a, PositionedNode b) {
                int cmp = Integer.compare(a.bounds.top, b.bounds.top);
                return cmp != 0 ? cmp : Integer.compare(a.bounds.left, b.bounds.left);
            }
        });

        long baseTs = System.currentTimeMillis() - (middleNodes.size() * 10L);
        List<PushStore.CapturedBubble> bubbles = new ArrayList<PushStore.CapturedBubble>();
        for (int i = 0; i < middleNodes.size(); i++) {
            PositionedNode pn = middleNodes.get(i);
            boolean outgoing = pn.bounds.centerX() > (int) (screenWidth * 0.52);
            String sender = outgoing ? "我" : detectedTitle;
            bubbles.add(new PushStore.CapturedBubble(sender, pn.text, outgoing, baseTs + i * 10L));
        }

        String appName = ImAppCatalog.appDisplayName(context, pkg);
        return PushStore.get(context).mergeCapturedWindowMessages(pkg, appName, detectedTitle, bubbles);
    }

    public static JSONObject sendChatViaUi(
            Context context,
            String pkg,
            String targetTitle,
            String text,
            boolean forceWhenHumanActive) throws Exception {
        return sendChatViaUi(context, pkg, targetTitle, text, forceWhenHumanActive, "");
    }

    /**
     * Fallback UI sender when no RemoteInput handle is available.
     * Verifies phone is unlocked, checks Human-in-Use touch guard (10s), locates editable input,
     * sets text via ACTION_SET_TEXT, and clicks the Send button.
     */
    public static JSONObject sendChatViaUi(
            Context context,
            String pkg,
            String targetTitle,
            String text,
            boolean forceWhenHumanActive,
            String clientMsgId) throws Exception {
        if (text == null || text.trim().isEmpty()) {
            throw new IllegalArgumentException("发送内容不能为空");
        }
        String trimmedText = text.trim();

        GuiAccessibilityService service = GuiAccessibilityService.instance;
        if (service == null) {
            throw new IllegalStateException("该会话暂无通知快捷回复句柄，且看屏服务未启用。请开启看屏操作服务或等待对方发来新消息后直接秒回");
        }

        Context effectiveCtx = context != null ? context : service;
        KeyguardManager km = effectiveCtx.getSystemService(KeyguardManager.class);
        PowerManager pm = effectiveCtx.getSystemService(PowerManager.class);
        if ((km != null && km.isKeyguardLocked()) || (pm != null && !pm.isInteractive())) {
            throw new IllegalStateException("手机当前处于熄屏或锁屏状态，且该会话尚无通知快捷回复句柄。请先解锁手机屏幕，或等对方发来消息后即可熄屏秒回");
        }

        if (!forceWhenHumanActive && GuiAccessibilityService.isHumanTouchingRecently(HUMAN_TOUCH_GUARD_MS)) {
            throw new IllegalStateException("检测到您正在触屏使用手机（10秒内有操作），为避免打断当前操作已暂缓切屏发送");
        }

        GuiAccessibilityService.markSyntheticAction();

        String requestedPkg = pkg == null ? "" : pkg.trim();
        String requestedTitle = targetTitle == null ? "" : targetTitle.trim();
        if (requestedPkg.isEmpty() || requestedTitle.isEmpty()) {
            throw new IllegalStateException("请先选择明确的回复会话");
        }
        PushStore.ThreadRecord target = PushStore.get(effectiveCtx).getThreadRecord(
            ImAppCatalog.buildThreadId(requestedPkg, requestedTitle));
        if (target != null && target.lastNotificationKey != null && !target.lastNotificationKey.isEmpty()) {
            PushStore.get(effectiveCtx).openPush(effectiveCtx, target.lastNotificationKey, requestedPkg);
            SystemClock.sleep(650L);
        }
        if (("com.android.mms".equals(requestedPkg) || "com.google.android.apps.messaging".equals(requestedPkg)
                || "com.samsung.android.messaging".equals(requestedPkg) || "com.miui.mms".equals(requestedPkg))
                && requestedTitle.matches("[+0-9 ()-]{3,30}")) {
            // SMS notifications often lack a reply action and their content intent
            // may only open the inbox. Address the exact conversation explicitly.
            Intent conversation = new Intent(Intent.ACTION_SENDTO, android.net.Uri.fromParts("smsto",requestedTitle.replace(" ",""),null));
            conversation.setPackage(requestedPkg).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
            effectiveCtx.startActivity(conversation);
            SystemClock.sleep(650L);
        }
        AccessibilityNodeInfo root = service.getRootInActiveWindow();
        String activePkg = (root != null && root.getPackageName() != null)
            ? root.getPackageName().toString()
            : "";

        String desiredPkg = (pkg != null && !pkg.trim().isEmpty()) ? pkg.trim() : activePkg;
        if (!desiredPkg.isEmpty() && !desiredPkg.equals(activePkg)) {
            Intent launch = effectiveCtx.getPackageManager().getLaunchIntentForPackage(desiredPkg);
            if (launch == null) {
                throw new IllegalStateException("未找到目标聊天应用: " + desiredPkg);
            }
            launch.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
            effectiveCtx.startActivity(launch);
            SystemClock.sleep(750L);
            root = service.getRootInActiveWindow();
        }

        if (root == null) {
            throw new IllegalStateException("无法获取当前聊天窗口控件树");
        }

        String cleanTarget = ImAppCatalog.cleanConversationTitle(desiredPkg, targetTitle);

        // If editable box isn't directly visible yet and targetTitle is visible in the conversation list, tap it
        AccessibilityNodeInfo inputNode = findEditableInput(root);
        if (inputNode == null && !cleanTarget.isEmpty()) {
            AccessibilityNodeInfo convItem = findNodeByExactOrPrefixText(root, cleanTarget);
            if (convItem != null) {
                GuiAccessibilityService.markSyntheticAction();
                clickNodeOrGesture(service, convItem);
                SystemClock.sleep(550L);
                root = service.getRootInActiveWindow();
                if (root != null) {
                    inputNode = findEditableInput(root);
                }
            }
        }

        if (inputNode == null) {
            throw new IllegalStateException("当前界面未找到聊天输入框，请确保手机正停留在目标聊天对话窗口");
        }
        if (!matchesTarget(root, desiredPkg, cleanTarget, service)) {
            throw new IllegalStateException("手机当前会话与回复对象不一致，消息尚未发送，请打开目标会话后重试");
        }
        int beforeCount = messageCount(root, trimmedText);

        GuiAccessibilityService.markSyntheticAction();
        inputNode.performAction(AccessibilityNodeInfo.ACTION_FOCUS);
        inputNode.performAction(AccessibilityNodeInfo.ACTION_CLICK);

        Bundle setArgs = new Bundle();
        setArgs.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, trimmedText);
        if (!inputNode.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, setArgs)) {
            throw new IllegalStateException("聊天输入框未接受文字写入");
        }

        SystemClock.sleep(180L);
        GuiAccessibilityService.markSyntheticAction();

        AccessibilityNodeInfo refreshedRoot = service.getRootInActiveWindow();
        if (refreshedRoot == null) refreshedRoot = root;

        AccessibilityNodeInfo sendBtn = findSendButton(refreshedRoot);
        boolean sent = false;
        if (sendBtn != null) {
            sent = clickNodeOrGesture(service, sendBtn);
        }
        if (!sent) {
            // Try IME Enter action on Android 30+ (AccessibilityNodeInfo.AccessibilityAction.ACTION_IME_ENTER = 0x01020054)
            sent = inputNode.performAction(android.R.id.accessibilityActionImeEnter);
        }
        if (!sent) {
            throw new IllegalStateException("已填入文字但未找到可点击的“发送”按钮");
        }

        boolean verified = false;
        for (int attempt=0; attempt<12; attempt++) {
            SystemClock.sleep(250L);
            AccessibilityNodeInfo after = service.getRootInActiveWindow();
            AccessibilityNodeInfo draft = findEditableInput(after);
            boolean cleared = draft != null && (draft.getText() == null || !trimmedText.equals(draft.getText().toString().trim()));
            if (ChatReplyPolicy.verifiedAfterSend(beforeCount, messageCount(after,trimmedText), cleared,
                    matchesTarget(after,desiredPkg,cleanTarget,service))) { verified=true; break; }
        }
        if (!verified) throw new IllegalStateException("已点击发送，但未核实到目标会话中的新消息；请检查手机，未自动重发");
        String effectiveTitle = cleanTarget;
        return PushStore.get(effectiveCtx).recordOutgoingUiMessage(
            effectiveCtx, desiredPkg, effectiveTitle, trimmedText, clientMsgId
        );
    }

    private static void collectVisibleTextNodes(
            AccessibilityNodeInfo node,
            List<PositionedNode> out,
            int depth) {
        if (node == null || depth > 42 || out.size() >= 300) return;
        if (node.isVisibleToUser() && !node.isPassword() && !node.isEditable()) {
            CharSequence raw = node.getText();
            String text = raw != null ? raw.toString().trim() : "";
            if (!text.isEmpty()) {
                Rect bounds = new Rect();
                node.getBoundsInScreen(bounds);
                if (bounds.width() > 8 && bounds.height() > 8) {
                    out.add(new PositionedNode(text, bounds));
                }
            }
        }
        int count = node.getChildCount();
        for (int i = 0; i < count && out.size() < 300; i++) {
            collectVisibleTextNodes(node.getChild(i), out, depth + 1);
        }
    }

    private static String extractTopTitle(List<PositionedNode> nodes, int topBarLimit, int screenWidth) {
        String best = "";
        int bestScore = Integer.MIN_VALUE;
        for (PositionedNode pn : nodes) {
            if (pn.bounds.bottom <= topBarLimit && pn.bounds.top >= 0) {
                String t = pn.text.trim();
                if (isTopBarNoise(t)) continue;
                // Prefer horizontally centered or prominent title nodes below the status bar
                int centerDist = Math.abs(pn.bounds.centerX() - (screenWidth / 2));
                int score = pn.bounds.top - (centerDist / 4);
                if (score > bestScore) {
                    bestScore = score;
                    best = t;
                }
            }
        }
        return best;
    }

    private static boolean isTopBarNoise(String text) {
        if (text == null || text.isEmpty()) return true;
        if (text.length() == 1 && ("<".equals(text) || "×".equals(text) || "…".equals(text) || "+".equals(text))) {
            return true;
        }
        if ("返回".equals(text) || "更多".equals(text) || "搜索".equals(text) || "微信".equals(text)) {
            return false;
        }
        // Status bar clock or battery percentage e.g. "12:30", "98%", "5G"
        if (text.matches("^[0-9]{1,2}:[0-9]{2}$") || text.matches("^[0-9]{1,3}%$") || "5G".equals(text) || "4G".equals(text)) {
            return true;
        }
        return false;
    }

    private static boolean isUiChromeOrTimestamp(String text) {
        if (text == null || text.isEmpty()) return true;
        if (text.matches("^[0-9]{1,2}:[0-9]{2}$")
                || text.matches("^(?:昨天|今天|前天)\\s*[0-9]{1,2}:[0-9]{2}$")
                || "发送".equals(text)
                || "按住 说话".equals(text)
                || "切换到语音".equals(text)) {
            return true;
        }
        return false;
    }

    private static AccessibilityNodeInfo findEditableInput(AccessibilityNodeInfo node) {
        if (node == null) return null;
        if (node.isVisibleToUser() && node.isEditable() && !node.isPassword()) {
            String id=node.getViewIdResourceName()==null?"":node.getViewIdResourceName().toLowerCase(java.util.Locale.ROOT);
            String hint=node.getHintText()==null?"":node.getHintText().toString();
            if(!id.contains("search") && !hint.contains("搜索") && !hint.toLowerCase(java.util.Locale.ROOT).contains("search")) return node;
        }
        int count = node.getChildCount();
        for (int i = 0; i < count; i++) {
            AccessibilityNodeInfo found = findEditableInput(node.getChild(i));
            if (found != null) return found;
        }
        return null;
    }

    private static AccessibilityNodeInfo findSendButton(AccessibilityNodeInfo node) {
        if (node == null) return null;
        if (node.isVisibleToUser() && !node.isEditable()) {
            String text = node.getText() != null ? node.getText().toString().trim() : "";
            String desc = node.getContentDescription() != null ? node.getContentDescription().toString().trim() : "";
            String viewId = node.getViewIdResourceName() != null ? node.getViewIdResourceName() : "";
            if (isSendLabel(text) || isSendLabel(desc) || viewId.endsWith("/send") || viewId.endsWith("btn_send")) {
                return node;
            }
        }
        int count = node.getChildCount();
        for (int i = 0; i < count; i++) {
            AccessibilityNodeInfo found = findSendButton(node.getChild(i));
            if (found != null) return found;
        }
        return null;
    }

    private static boolean matchesTarget(AccessibilityNodeInfo root, String pkg, String title, GuiAccessibilityService service) {
        if(root == null || root.getPackageName() == null || !pkg.equals(root.getPackageName().toString())) return false;
        List<PositionedNode> nodes = new ArrayList<PositionedNode>();
        collectVisibleTextNodes(root,nodes,0);
        int limit=(int)(service.getResources().getDisplayMetrics().heightPixels*0.22);
        for(PositionedNode n:nodes) if(n.bounds.bottom<=limit && ChatReplyPolicy.sameRecipient(title,n.text)) return true;
        return false;
    }
    private static int messageCount(AccessibilityNodeInfo root,String text) {
        List<PositionedNode> nodes=new ArrayList<PositionedNode>();collectVisibleTextNodes(root,nodes,0);
        int count=0;for(PositionedNode n:nodes) if(text.equals(n.text)) count++;return count;
    }
    private static boolean isSendLabel(String s) {
        if (s == null || s.isEmpty()) return false;
        return "发送".equals(s) || "發送".equals(s) || "传送".equals(s)
            || "Send".equalsIgnoreCase(s) || s.startsWith("发送(");
    }

    private static AccessibilityNodeInfo findNodeByExactOrPrefixText(AccessibilityNodeInfo node, String target) {
        if (node == null || target == null || target.isEmpty()) return null;
        if (node.isVisibleToUser() && !node.isEditable()) {
            String text = node.getText() != null ? node.getText().toString().trim() : "";
            String clean = ImAppCatalog.cleanConversationTitle("", text);
            if (target.equals(clean) || ChatReplyPolicy.sameRecipient(target,text)) {
                return node;
            }
        }
        int count = node.getChildCount();
        for (int i = 0; i < count; i++) {
            AccessibilityNodeInfo found = findNodeByExactOrPrefixText(node.getChild(i), target);
            if (found != null) return found;
        }
        return null;
    }

    private static boolean clickNodeOrGesture(GuiAccessibilityService service, AccessibilityNodeInfo target) {
        AccessibilityNodeInfo current = target;
        for (int depth = 0; depth < 4 && current != null; depth++) {
            if (current.isClickable() && current.performAction(AccessibilityNodeInfo.ACTION_CLICK)) {
                return true;
            }
            current = current.getParent();
        }
        Rect bounds = new Rect();
        target.getBoundsInScreen(bounds);
        if (bounds.width() <= 2 || bounds.height() <= 2) return false;

        int[] pt = pickSafeTouchPoint(bounds);
        Path path = new Path();
        path.moveTo(pt[0], pt[1]);
        GestureDescription gesture = new GestureDescription.Builder()
            .addStroke(new GestureDescription.StrokeDescription(path, 0L, 55L))
            .build();
        final CountDownLatch latch = new CountDownLatch(1);
        final boolean[] ok = new boolean[] { false };
        boolean accepted = service.dispatchGesture(gesture, new GuiAccessibilityService.GestureResultCallback() {
            @Override
            public void onCompleted(GestureDescription gestureDescription) {
                ok[0] = true;
                latch.countDown();
            }

            @Override
            public void onCancelled(GestureDescription gestureDescription) {
                latch.countDown();
            }
        }, service.main);
        if (!accepted) return false;
        try {
            latch.await(1500L, TimeUnit.MILLISECONDS);
        } catch (InterruptedException ignored) {
            Thread.currentThread().interrupt();
        }
        return ok[0];
    }

    /**
     * TouchPointPicker: picks a randomized safe point inside the inner 50% of the button bounds
     * so repeated sends never tap the exact same pixel coordinate.
     */
    static int[] pickSafeTouchPoint(Rect bounds) {
        int w = Math.max(1, bounds.width());
        int h = Math.max(1, bounds.height());
        int jitterX = w >= 8 ? (RANDOM.nextInt(Math.max(2, w / 3)) - (w / 6)) : 0;
        int jitterY = h >= 8 ? (RANDOM.nextInt(Math.max(2, h / 3)) - (h / 6)) : 0;
        int x = Math.max(bounds.left + 1, Math.min(bounds.right - 1, bounds.centerX() + jitterX));
        int y = Math.max(bounds.top + 1, Math.min(bounds.bottom - 1, bounds.centerY() + jitterY));
        return new int[] { x, y };
    }
}

package com.stepx.companion;

import android.Manifest;
import android.accessibilityservice.AccessibilityServiceInfo;
import android.app.Activity;
import android.app.KeyguardManager;
import android.app.PendingIntent;
import android.app.UiAutomation;
import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.content.IntentFilter;
import android.content.pm.PackageManager;
import android.graphics.Rect;
import android.database.Cursor;
import android.net.Uri;
import android.os.Bundle;
import android.os.Handler;
import android.os.HandlerThread;
import android.os.Looper;
import android.telephony.SmsManager;
import android.telephony.SubscriptionInfo;
import android.telephony.SubscriptionManager;
import android.view.accessibility.AccessibilityNodeInfo;
import org.json.JSONArray;
import org.json.JSONObject;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.*;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;

/** Small Android API adapter, run as the already-authorized ADB shell UID. */
public final class Bridge {
    static final String PKG = "com.stepos.ailauncher";
    static final String ROOT = "/data/local/tmp/stepx-companion";
    static Context context;
    static HandlerThread callbacks;
    static UiAutomation automation;

    public static void main(String[] args) {
        try {
            Looper.prepareMainLooper();
            Class<?> at = Class.forName("android.app.ActivityThread");
            Object thread = at.getMethod("systemMain").invoke(null);
            Context system = (Context) at.getMethod("getSystemContext").invoke(thread);
            context = system.createPackageContext("com.android.shell", 0);
            callbacks = new HandlerThread("stepx-bridge");
            callbacks.start();
            String line = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8)).readLine();
            if (line == null || line.length() > 32768) throw new IllegalArgumentException("请求为空或过长");
            JSONObject request = new JSONObject(line);
            JSONObject result;
            switch (request.getString("op")) {
                case "apps_list": result = AppTools.list(request); break;
                case "app_info": result = AppTools.info(request); break;
                case "phone_accounts": result = DialTools.inspectAccounts(); break;
                case "phone_prepare": result = DialTools.prepare(request); break;
                case "phone_call": result = DialTools.call(request); break;
                case "storage_scan": result = StorageTools.scan(); break;
                case "storage_file_operation": result = FileTools.run(request); break;
                case "storage_clear_cache": result = FileTools.clearCache(); break;
                case "read_sms": result = queryRows("content://sms", new String[]{"address", "body", "date", "type", "read", "sub_id"}, "date DESC, _id DESC"); break;
                case "read_calls": result = queryRows("content://call_log/calls", new String[]{"number", "name", "type", "date", "duration"}, "date DESC, _id DESC"); break;
                case "read_contacts": result = queryRows("content://com.android.contacts/data/phones", new String[]{"display_name", "data1"}, "display_name ASC"); break;
                case "capabilities": result = capabilities(); break;
                case "amoo_context": result = amooContext(); break;
                case "amoo_snapshot": result = snapshot(request.optBoolean("debug", false)); break;
                case "amoo_sync": result = syncAmoo(request.optString("select_title", ""), request.optInt("select_index", -1)); break;
                case "amoo_new_chat": result = newChatAmoo(); break;
                case "amoo_prepare": result = prepareAmoo(request.getString("text")); break;
                case "amoo_send": result = sendAmoo(request.getString("text")); break;
                case "sms_prepare": result = prepareSms(request); break;
                case "sms_send": result = sendSms(request); break;
                case "sms_status": result = readStatus(request.getString("id")); break;
                default: throw new IllegalArgumentException("未知操作");
            }
            result.put("ok", true);
            System.out.println("STEPX_RESULT:" + result);
        } catch (Throwable error) {
            while (error.getCause() != null) error = error.getCause();
            try { System.out.println("STEPX_RESULT:" + new JSONObject().put("ok", false)
                .put("error", error.getMessage() == null ? error.getClass().getSimpleName() : error.getMessage())); }
            catch (Throwable ignored) {}
        } finally {
            if (automation != null) try { UiAutomation.class.getMethod("disconnect").invoke(automation); } catch (Throwable ignored) {}
            if (callbacks != null) callbacks.quitSafely();
            System.exit(0);
        }
    }

    // Cursor values are JSON encoded individually: message text can contain newlines,
    // commas and even strings identical to the shell `content query` row delimiters.
    static JSONObject queryRows(String uri, String[] columns, String order) throws Exception {
        return queryRows(uri, columns, order, null);
    }

    static JSONObject queryRows(String uri, String[] columns, String order, String[] args) throws Exception {
        // app_process has no registered ApplicationThread. Use the external provider
        // handle, as Android's `content` shell command does, with shell attribution.
        Uri target = Uri.parse(uri);
        Class<?> am = Class.forName("android.app.ActivityManager");
        Class<?> iam = Class.forName("android.app.IActivityManager");
        Object service = am.getMethod("getService").invoke(null);
        int user = (Integer) am.getMethod("getCurrentUser").invoke(null);
        android.os.IBinder token = new android.os.Binder();
        Object holder = iam.getMethod("getContentProviderExternal", String.class, int.class,
            android.os.IBinder.class, String.class).invoke(service, target.getAuthority(), user, token, "*cmd*");
        if (holder == null) throw new IOException("手机数据提供程序不可用");
        try {
            Object provider = holder.getClass().getField("provider").get(holder);
            Class<?> ipc = Class.forName("android.content.IContentProvider");
            Class<?> signal = Class.forName("android.os.ICancellationSignal");
            android.content.AttributionSource attribution = new android.content.AttributionSource.Builder(android.os.Process.myUid())
                .setPackageName("com.android.shell").build();
            Bundle extras = new Bundle();
            extras.putString(android.content.ContentResolver.QUERY_ARG_SQL_SORT_ORDER, order);
            if (args != null) extras.putStringArray(android.content.ContentResolver.QUERY_ARG_SQL_SELECTION_ARGS, args);
            JSONArray rows = new JSONArray();
            try (Cursor cursor = (Cursor) ipc.getMethod("query", android.content.AttributionSource.class,
                Uri.class, String[].class, Bundle.class, signal).invoke(provider, attribution, target, columns, extras, null)) {
                if (cursor == null) throw new IOException("手机数据提供程序未返回结果");
                while (cursor.moveToNext()) {
                    JSONObject row = new JSONObject();
                    for (int i = 0; i < columns.length; i++) {
                        int index = cursor.getColumnIndexOrThrow(columns[i]);
                        row.put(columns[i], cursor.isNull(index) ? JSONObject.NULL : cursor.getString(index));
                    }
                    rows.put(row);
                }
            }
            return new JSONObject().put("rows", rows);
        } finally {
            iam.getMethod("removeContentProviderExternalAsUser", String.class, android.os.IBinder.class, int.class)
                .invoke(service, target.getAuthority(), token, user);
        }
    }

    // Same public OpenId provider used by Amoo. No account token or private DB.
    static JSONObject amooContext() throws Exception {
        JSONArray rows = queryRows("content://com.stepos.secureservice.openidsdk/",
            new String[]{"value", "code"}, null, new String[]{"oaid"}).getJSONArray("rows");
        // The native SDK consumes a nonempty value even when provider code != 1000
        // (StepOS reports 1). The value, not that vendor-specific code, is the ID.
        if (rows.length() != 1)
            throw new IOException("无法读取当前手机的 Amoo 设备标识，消息未发送");
        String id = rows.getJSONObject(0).optString("value", "");
        if (id.isEmpty()) throw new IOException("Amoo 设备标识为空，消息未发送");
        return new JSONObject().put("device_id", id)
            .put("device_model", android.os.Build.MODEL)
            .put("os_version", android.os.Build.VERSION.RELEASE)
            .put("app_version", context.getPackageManager().getPackageInfo(PKG, 0).versionName)
            .put("screenLockState", context.getSystemService(KeyguardManager.class).isKeyguardLocked() ? "0" : "1");
    }

    static JSONObject capabilities() throws Exception {
        // app_process does not run SystemServer's telephony bootstrap.
        Class<?> initializer = Class.forName("android.telephony.TelephonyFrameworkInitializer");
        if (initializer.getMethod("getTelephonyServiceManager").invoke(null) == null) {
            Class<?> manager = Class.forName("android.os.TelephonyServiceManager");
            initializer.getMethod("setTelephonyServiceManager", manager)
                .invoke(null, manager.getConstructor().newInstance());
        }
        JSONObject out = new JSONObject();
        out.put("send_sms", context.checkSelfPermission(Manifest.permission.SEND_SMS) == PackageManager.PERMISSION_GRANTED);
        out.put("locked", context.getSystemService(KeyguardManager.class).isKeyguardLocked());
        out.put("default_subscription", SubscriptionManager.getDefaultSmsSubscriptionId());
        JSONArray sims = new JSONArray();
        List<SubscriptionInfo> list = context.getSystemService(SubscriptionManager.class).getActiveSubscriptionInfoList();
        if (list != null) for (SubscriptionInfo sim : list) sims.put(new JSONObject()
            .put("id", sim.getSubscriptionId()).put("slot", sim.getSimSlotIndex() + 1)
            .put("label", String.valueOf(sim.getDisplayName())));
        return out.put("subscriptions", sims).put("protocol", 1);
    }

    static void connectUi() throws Exception {
        if (automation != null) return;
        Class<?> connectionType = Class.forName("android.app.IUiAutomationConnection");
        Object connection = Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
        automation = (UiAutomation) UiAutomation.class.getConstructor(Looper.class, connectionType)
            .newInstance(callbacks.getLooper(), connection);
        UiAutomation.class.getMethod("connect", int.class).invoke(automation, 1);
        AccessibilityServiceInfo info = automation.getServiceInfo();
        info.flags |= AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS | AccessibilityServiceInfo.FLAG_RETRIEVE_INTERACTIVE_WINDOWS;
        automation.setServiceInfo(info);
    }

    static AccessibilityNodeInfo root() throws Exception {
        connectUi();
        for (int i = 0; i < 12; i++) {
            AccessibilityNodeInfo root = automation.getRootInActiveWindow();
            if (root != null && PKG.contentEquals(root.getPackageName())) return root;
            Thread.sleep(100);
        }
        throw new IllegalStateException("手机当前不在 Amoo 对话页，请点击连接 Amoo 后重试");
    }

    static AccessibilityNodeInfo find(AccessibilityNodeInfo root, String id) {
        List<AccessibilityNodeInfo> nodes = root.findAccessibilityNodeInfosByViewId(PKG + ":id/" + id);
        for (AccessibilityNodeInfo node : nodes) if (node.isVisibleToUser()) return node;
        return null;
    }

    static void collect(AccessibilityNodeInfo node, JSONArray result, int depth) throws Exception {
        if (node == null || depth > 35 || result.length() >= 1000 || node.isPassword() || !node.isVisibleToUser()) return;
        String id = node.getViewIdResourceName();
        CharSequence text = node.getText(), desc = node.getContentDescription();
        if (id != null || text != null || desc != null) result.put(new JSONObject()
            .put("id", id == null ? "" : id).put("text", text == null ? "" : text.toString())
            .put("description", desc == null ? "" : desc.toString())
            .put("class", String.valueOf(node.getClassName())).put("clickable", node.isClickable())
            .put("enabled", node.isEnabled()).put("depth", depth));
        for (int i = 0; i < node.getChildCount(); i++) collect(node.getChild(i), result, depth + 1);
    }

    static JSONObject snapshot(boolean debug) throws Exception {
        AccessibilityNodeInfo root = root();
        AccessibilityNodeInfo closeBtn = find(root, "btn_close");
        if (closeBtn != null && closeBtn.isClickable()) {
            closeBtn.performAction(AccessibilityNodeInfo.ACTION_CLICK);
            Thread.sleep(250);
            root = root();
        }
        AccessibilityNodeInfo list = find(root, "chat_list");
        if (list == null) throw new IllegalStateException("未找到 Amoo 消息列表，请打开聊天页");
        List<AccessibilityNodeInfo> children = new ArrayList<>();
        for (int i = 0; i < list.getChildCount(); i++) {
            AccessibilityNodeInfo c = list.getChild(i);
            if (c != null && c.isVisibleToUser()) children.add(c);
        }
        children.sort((a, b) -> {
            Rect ra = new Rect(), rb = new Rect();
            a.getBoundsInScreen(ra);
            b.getBoundsInScreen(rb);
            return Integer.compare(ra.top, rb.top);
        });
        JSONArray rows = new JSONArray();
        for (AccessibilityNodeInfo child : children) {
            JSONArray nodes = new JSONArray(); collect(child, nodes, 0);
            if (nodes.length() > 0) rows.put(nodes);
        }
        JSONObject result = new JSONObject().put("rows", rows);
        if (debug) { JSONArray nodes = new JSONArray(); collect(root, nodes, 0); result.put("nodes", nodes); }
        return result;
    }

    static JSONObject syncAmoo(String selectTitle, int selectIndex) throws Exception {
        AccessibilityNodeInfo root = root();
        AccessibilityNodeInfo closeBtn = find(root, "btn_close");
        if (closeBtn != null && closeBtn.isClickable()) {
            closeBtn.performAction(AccessibilityNodeInfo.ACTION_CLICK);
            Thread.sleep(250);
            root = root();
        }
        AccessibilityNodeInfo history = find(root, "rv_chat_history");
        if (history == null) {
            AccessibilityNodeInfo menu = find(root, "iv_menu");
            if (menu != null && menu.isClickable()) {
                menu.performAction(AccessibilityNodeInfo.ACTION_CLICK);
                for (int i = 0; i < 8; i++) {
                    Thread.sleep(100);
                    root = root();
                    history = find(root, "rv_chat_history");
                    if (history != null) break;
                }
            }
        }
        JSONArray sessions = new JSONArray();
        String chosenTitle = "";
        if (history != null) {
            String group = "今天";
            List<AccessibilityNodeInfo> clickTargets = new ArrayList<>();
            List<String> titles = new ArrayList<>();
            for (int i = 0; i < history.getChildCount(); i++) {
                AccessibilityNodeInfo child = history.getChild(i);
                if (child == null || !child.isVisibleToUser()) continue;
                String id = child.getViewIdResourceName();
                CharSequence directText = child.getText();
                if ("android:id/text1".equals(id) && directText != null && directText.length() > 0) {
                    group = directText.toString().trim();
                    continue;
                }
                List<AccessibilityNodeInfo> items = child.findAccessibilityNodeInfosByViewId(PKG + ":id/ll_drawer_item_content");
                if (items == null || items.isEmpty()) continue;
                AccessibilityNodeInfo itemNode = items.get(0);
                List<AccessibilityNodeInfo> titleNodes = itemNode.findAccessibilityNodeInfosByViewId("android:id/text1");
                if (titleNodes == null || titleNodes.isEmpty()) continue;
                CharSequence t = titleNodes.get(0).getText();
                String title = t == null ? "" : t.toString().trim();
                if (title.isEmpty()) continue;
                List<AccessibilityNodeInfo> statusIcons = itemNode.findAccessibilityNodeInfosByViewId(PKG + ":id/iv_session_status");
                boolean hasStatusIcon = statusIcons != null && !statusIcons.isEmpty() && statusIcons.get(0).isVisibleToUser();
                sessions.put(new JSONObject()
                    .put("title", title)
                    .put("group", group)
                    .put("has_status_icon", hasStatusIcon));
                clickTargets.add(itemNode);
                titles.add(title);
            }
            int targetIdx = -1;
            if (selectTitle != null && !selectTitle.trim().isEmpty()) {
                String wanted = selectTitle.trim();
                for (int i = 0; i < titles.size(); i++) {
                    if (titles.get(i).equals(wanted)) { targetIdx = i; break; }
                }
            }
            if (targetIdx < 0 && selectIndex >= 0 && selectIndex < clickTargets.size()) {
                targetIdx = selectIndex;
            }
            if (targetIdx < 0 && !clickTargets.isEmpty()) {
                targetIdx = 0;
            }
            if (targetIdx >= 0 && targetIdx < clickTargets.size()) {
                chosenTitle = titles.get(targetIdx);
                clickTargets.get(targetIdx).performAction(AccessibilityNodeInfo.ACTION_CLICK);
                Thread.sleep(450);
            }
        }
        JSONObject snap = snapshot(false);
        snap.put("sessions", sessions);
        snap.put("selected_title", chosenTitle);
        return snap;
    }

    static JSONObject newChatAmoo() throws Exception {
        AccessibilityNodeInfo root = root();
        AccessibilityNodeInfo drawerNew = find(root, "btn_slide_panel_new_chat");
        if (drawerNew != null && drawerNew.isClickable()) {
            drawerNew.performAction(AccessibilityNodeInfo.ACTION_CLICK);
            Thread.sleep(350);
            return snapshot(false);
        }
        AccessibilityNodeInfo topNew = find(root, "btn_new_session");
        if (topNew != null && topNew.isClickable()) {
            topNew.performAction(AccessibilityNodeInfo.ACTION_CLICK);
            Thread.sleep(350);
            return snapshot(false);
        }
        return snapshot(false);
    }

    static JSONObject prepareAmoo(String text) throws Exception {
        if (text.trim().isEmpty() || text.length() > 8000) throw new IllegalArgumentException("问题不能为空，最多 8000 字");
        AccessibilityNodeInfo root = root();
        AccessibilityNodeInfo history = find(root, "rv_chat_history");
        if (history != null) {
            List<AccessibilityNodeInfo> items = history.findAccessibilityNodeInfosByViewId(PKG + ":id/ll_drawer_item_content");
            if (items != null && !items.isEmpty()) {
                items.get(0).performAction(AccessibilityNodeInfo.ACTION_CLICK);
                Thread.sleep(350);
                root = root();
            }
        }
        AccessibilityNodeInfo input = find(root, "et_input_bar");
        if (input == null || !input.isEditable()) throw new IllegalStateException("未找到可编辑的 Amoo 输入框");
        Bundle arguments = new Bundle(); arguments.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, text);
        if (!input.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, arguments)) throw new IllegalStateException("Amoo 未接受输入，请重试");
        Thread.sleep(250);
        AccessibilityNodeInfo updated = find(root(), "et_input_bar");
        if (updated == null || !text.contentEquals(updated.getText())) throw new IllegalStateException("Amoo 输入核对失败，未发送");
        return snapshot(true).put("prepared", true);
    }

    static JSONObject sendAmoo(String text) throws Exception {
        prepareAmoo(text);
        // Resource name verified against the installed StepX build before enabling sending.
        AccessibilityNodeInfo send = find(root(), "btn_send");
        if (send == null || !send.isEnabled() || !send.isClickable()) throw new IllegalStateException("未找到可用的 Amoo 发送按钮，草稿保留在手机中");
        if (!send.performAction(AccessibilityNodeInfo.ACTION_CLICK)) throw new IllegalStateException("Amoo 未接受发送操作，未自动重试");
        return new JSONObject().put("submitted", true);
    }

    static int subscription(JSONObject request) throws Exception {
        JSONArray sims = capabilities().getJSONArray("subscriptions");
        int selected = request.optInt("subscription", -1);
        if (selected < 0) selected = SubscriptionManager.getDefaultSmsSubscriptionId();
        if (selected < 0 && sims.length() == 1) selected = sims.getJSONObject(0).getInt("id");
        for (int i = 0; i < sims.length(); i++) if (sims.getJSONObject(i).getInt("id") == selected) return selected;
        throw new IllegalStateException("请选择可用的发信 SIM 卡");
    }

    static SmsManager smsManager(int subscription) {
        return context.getSystemService(SmsManager.class).createForSubscriptionId(subscription);
    }

    static JSONObject prepareSms(JSONObject request) throws Exception {
        String number = request.getString("number").trim(), body = request.getString("body");
        if (!number.matches("\\+?[0-9][0-9 ()-]{1,39}")) throw new IllegalArgumentException("收件人号码无效");
        if (body.trim().isEmpty() || body.length() > 2000) throw new IllegalArgumentException("短信正文不能为空，最多 2000 字");
        if (!capabilities().getBoolean("send_sms")) throw new SecurityException("手机未授予调试通道 SEND_SMS 权限");
        int sub = subscription(request);
        int parts = smsManager(sub).divideMessage(body).size();
        return new JSONObject().put("subscription", sub).put("parts", parts);
    }

    static File statusFile(String id) {
        if (!id.matches("[a-f0-9]{32}")) throw new IllegalArgumentException("发送请求标识无效");
        File dir = new File(ROOT); dir.mkdirs(); dir.setReadable(false, false); dir.setWritable(false, false); dir.setExecutable(false, false);
        dir.setReadable(true, true); dir.setWritable(true, true); dir.setExecutable(true, true);
        return new File(dir, "sms-" + id + ".json");
    }

    static synchronized void writeStatus(File file, JSONObject value) throws Exception {
        File pending = new File(file.getPath() + ".pending");
        try (FileOutputStream out = new FileOutputStream(pending)) { out.write(value.toString().getBytes(StandardCharsets.UTF_8)); out.getFD().sync(); }
        pending.setReadable(false, false); pending.setWritable(false, false); pending.setReadable(true, true); pending.setWritable(true, true);
        if (!pending.renameTo(file)) throw new IOException("不能保存发送状态");
    }

    static JSONObject readStatus(String id) throws Exception {
        File file = statusFile(id);
        if (!file.exists()) return new JSONObject().put("id", id).put("state", "not_found");
        if (file.length() == 0) return new JSONObject().put("id", id).put("state", "unknown");
        return new JSONObject(new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8));
    }

    static JSONObject sendSms(JSONObject request) throws Exception {
        JSONObject plan = prepareSms(request);
        String id = request.getString("id"); File file = statusFile(id);
        if (!file.createNewFile()) return readStatus(id); // Never retransmit an existing request.
        final JSONObject state = new JSONObject().put("id", id).put("state", "sending")
            .put("parts", plan.getInt("parts")).put("completed", 0).put("updated_ms", System.currentTimeMillis());
        writeStatus(file, state);
        ArrayList<String> parts = smsManager(plan.getInt("subscription")).divideMessage(request.getString("body"));
        CountDownLatch latch = new CountDownLatch(parts.size()); AtomicInteger completed = new AtomicInteger();
        final Set<Integer> received = new HashSet<>();
        String action = "com.stepx.companion.SMS_SENT_" + id;
        BroadcastReceiver receiver = new BroadcastReceiver() {
            public void onReceive(Context unused, Intent intent) {
                synchronized (state) {
                    try {
                        int part = intent.getIntExtra("part", -1);
                        if (part < 0 || !received.add(part)) return;
                        int result = getResultCode();
                        if (result != Activity.RESULT_OK) state.put("error_code", result);
                        int count = completed.incrementAndGet(); state.put("completed", count);
                        if (count == parts.size()) state.put("state", state.has("error_code") ? "failed" : "sent");
                        state.put("updated_ms", System.currentTimeMillis()); writeStatus(file, state);
                    } catch (Exception ignored) {} finally { latch.countDown(); }
                }
            }
        };
        context.registerReceiver(receiver, new IntentFilter(action), null, new Handler(callbacks.getLooper()), Context.RECEIVER_NOT_EXPORTED);
        try {
            ArrayList<PendingIntent> intents = new ArrayList<>();
            for (int i = 0; i < parts.size(); i++) intents.add(PendingIntent.getBroadcast(context, i,
                new Intent(action).setPackage("com.android.shell").putExtra("part", i), PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_ONE_SHOT));
            smsManager(plan.getInt("subscription")).sendMultipartTextMessage(request.getString("number"), null, parts, intents, null);
            if (!latch.await(45, TimeUnit.SECONDS)) synchronized (state) {
                state.put("state", "unknown"); state.put("updated_ms", System.currentTimeMillis()); writeStatus(file, state);
            }
        } catch (Throwable error) {
            synchronized (state) { state.put("state", "unknown").put("error", error.getClass().getSimpleName()); writeStatus(file, state); }
        } finally { context.unregisterReceiver(receiver); }
        return readStatus(id);
    }
}

package com.stepx.companion.neo;

import android.content.ContentProvider;
import android.content.ContentValues;
import android.content.pm.PackageManager;
import android.database.Cursor;
import android.net.Uri;
import android.os.Binder;
import android.os.Bundle;
import android.os.ParcelFileDescriptor;
import android.util.Base64;
import org.json.JSONObject;
import java.io.ByteArrayOutputStream;
import java.io.FileNotFoundException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;

/** UID and exact companion-signature checked IPC, with no network listener. */
public final class CompanionBridgeProvider extends ContentProvider {
    private void enforceCaller() {
        int uid = Binder.getCallingUid(), own = android.os.Process.myUid();
        PackageManager pm = getContext().getPackageManager();
        if (!BridgePolicy.caller(uid, own, pm.getPackagesForUid(uid),
            pm.checkSignatures(uid, own) == PackageManager.SIGNATURE_MATCH))
            throw new SecurityException("Neo bridge caller is not authorized");
    }
    @Override public boolean onCreate() { return true; }
    @Override public Bundle call(String method, String arg, Bundle extras) {
        enforceCaller();
        int caller = Binder.getCallingUid();
        long identity = Binder.clearCallingIdentity();
        JSONObject result;
        try {
            if (!"request".equals(method)) throw new IllegalArgumentException();
            String raw;
            if (extras != null && extras.containsKey("request")) raw = extras.getString("request");
            else {
                if (arg == null || arg.length() > 48000) throw new IllegalArgumentException();
                raw = new String(Base64.decode(arg, Base64.DEFAULT), StandardCharsets.UTF_8);
            }
            if (raw == null || raw.length() > 32768) throw new IllegalArgumentException();
            JSONObject request = new JSONObject(raw);
            if ("host_ui".equals(request.optString("op")) && caller != android.os.Process.myUid())
                result = failure("host_only", "仅伴侣前台页面可管理自身控制栏");
            else if (caller == 2000 && "config".equals(request.optString("op")) && request.has("key"))
                result = failure("credential_transport", "请通过伴侣配置，或以标准输入写入 /config；密钥不能放在命令参数中");
            else result = CompanionBridgeRuntime.get(getContext()).request(request);
        } catch (Exception invalid) {
            result = failure("invalid_request", "请求无效，未执行操作");
        } finally { Binder.restoreCallingIdentity(identity); }
        Bundle answer = new Bundle();
        answer.putString("result", Base64.encodeToString(result.toString().getBytes(StandardCharsets.UTF_8), Base64.NO_WRAP));
        return answer;
    }
    /** ADB can stream configuration via stdin; no credentials in command arguments. */
    @Override public ParcelFileDescriptor openFile(Uri uri, String mode) throws FileNotFoundException {
        enforceCaller();
        if (!"/config".equals(uri.getPath()) || !"w".equals(mode)) throw new FileNotFoundException();
        try {
            ParcelFileDescriptor[] pipe = ParcelFileDescriptor.createPipe();
            new Thread(() -> {
                try (InputStream in = new ParcelFileDescriptor.AutoCloseInputStream(pipe[0])) {
                    ByteArrayOutputStream data = new ByteArrayOutputStream();
                    byte[] buffer = new byte[4096]; int n;
                    while ((n = in.read(buffer)) >= 0) {
                        if (data.size() + n > 32768) throw new IllegalArgumentException();
                        data.write(buffer, 0, n);
                    }
                    JSONObject request = new JSONObject(new String(data.toByteArray(), StandardCharsets.UTF_8));
                    if (!"config".equals(request.optString("op"))) throw new IllegalArgumentException();
                    CompanionBridgeRuntime.get(getContext()).request(request);
                } catch (Exception invalid) { /* Never write incoming bytes to a log. */ }
            }, "Neo-private-config").start();
            return pipe[1];
        } catch (Exception failure) { throw new FileNotFoundException("Unable to open private configuration pipe"); }
    }
    static JSONObject failure(String code, String message) {
        JSONObject out = new JSONObject();
        try { out.put("ok", false).put("code", code).put("error", message); } catch (Exception ignored) {}
        return out;
    }
    @Override public Cursor query(Uri u, String[] p, String s, String[] a, String o) { throw new UnsupportedOperationException(); }
    @Override public String getType(Uri u) { return null; }
    @Override public Uri insert(Uri u, ContentValues v) { throw new UnsupportedOperationException(); }
    @Override public int delete(Uri u, String s, String[] a) { throw new UnsupportedOperationException(); }
    @Override public int update(Uri u, ContentValues v, String s, String[] a) { throw new UnsupportedOperationException(); }
}

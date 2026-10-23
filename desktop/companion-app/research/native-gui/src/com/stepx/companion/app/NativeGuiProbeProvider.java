package com.stepx.companion.app;

import android.content.ContentProvider;
import android.content.ContentValues;
import android.database.Cursor;
import android.net.Credentials;
import android.net.LocalSocket;
import android.net.LocalSocketAddress;
import android.net.Uri;
import android.os.Binder;
import android.os.Bundle;
import android.os.Process;
import android.os.SystemClock;
import org.json.JSONObject;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;

/** Explicit research build only; excluded from the standard companion APK. */
public final class NativeGuiProbeProvider extends ContentProvider {
    private static final String SOCKET = "stepx-native-gui-v1";
    private static final String TARGET = "com.stepos.ability";
    private static final String APK_SHA256 = "c9f56f3078df96d8d726422b1c622a1c19fe8c7dec8cadd8537d9a6ecf513ac4";
    private static final int READ_TIMEOUT_MS = 2000;
    private static final int MAX_RESPONSE_BYTES = 32 * 1024;

    @Override public boolean onCreate() { return true; }

    @Override public Bundle call(String method, String arg, Bundle extras) {
        int caller = Binder.getCallingUid();
        if (caller != 2000 && caller != Process.myUid())
            throw new SecurityException("Native GUI probe is restricted to the ADB shell and this companion");
        if (!isOperation(method) || arg != null || (extras != null && !extras.isEmpty()))
            throw new IllegalArgumentException("Only status, native_sdk_probe and shutdown are supported without arguments");
        long identity = Binder.clearCallingIdentity();
        Bundle result = new Bundle();
        try {
            JSONObject response = exchange(method);
            result.putBoolean("ok", response.optBoolean("ok", false));
            result.putString("result", response.toString());
        } catch (Exception error) {
            String message = error.getClass().getSimpleName() + ": " + String.valueOf(error.getMessage());
            message = message.replaceAll("[\\r\\n\\t]", " ");
            if (message.length() > 240) message = message.substring(0, 240);
            result.putBoolean("ok", false);
            result.putString("error", message);
        } finally {
            Binder.restoreCallingIdentity(identity);
        }
        return result;
    }

    private static boolean isOperation(String op) {
        return "status".equals(op) || "native_sdk_probe".equals(op) || "shutdown".equals(op);
    }

    private static JSONObject exchange(String operation) throws Exception {
        try (LocalSocket socket = new LocalSocket()) {
            socket.connect(new LocalSocketAddress(SOCKET, LocalSocketAddress.Namespace.ABSTRACT));
            socket.setSoTimeout(READ_TIMEOUT_MS);
            // This is a Unix socket: Binder.getCallingUid() cannot authenticate its server.
            Credentials peer = socket.getPeerCredentials();
            if (peer == null || peer.getUid() != 1000 || peer.getPid() <= 0)
                throw new SecurityException("Native GUI probe peer is not the system process owner");
            byte[] request = (new JSONObject().put("op", operation).toString() + "\n")
                    .getBytes(StandardCharsets.UTF_8);
            socket.getOutputStream().write(request);
            socket.getOutputStream().flush();
            String raw = readResponse(socket);
            JSONObject response = new JSONObject(raw);
            if (response.optInt("version", -1) != 1 || !response.has("ok"))
                throw new IOException("Native GUI probe returned an invalid response");
            if (response.optBoolean("ok") && !"shutdown".equals(operation)) {
                if (!SOCKET.equals(response.optString("socket"))
                        || !TARGET.equals(response.optString("target"))
                        || !APK_SHA256.equals(response.optString("apk_sha256"))
                        || response.optInt("pid", -1) != peer.getPid()
                        || response.optInt("uid", -1) != Process.myUid())
                    throw new SecurityException("Native GUI probe identity or APK does not match");
            }
            return response;
        }
    }

    private static String readResponse(LocalSocket socket) throws IOException {
        long deadline = SystemClock.elapsedRealtime() + READ_TIMEOUT_MS;
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        InputStream input = socket.getInputStream();
        byte[] buffer = new byte[1024];
        while (true) {
            long remaining = deadline - SystemClock.elapsedRealtime();
            if (remaining <= 0) throw new java.net.SocketTimeoutException("Native GUI probe response timed out");
            socket.setSoTimeout((int) remaining);
            int count = input.read(buffer);
            if (count < 0) throw new IOException("Native GUI probe response ended before its delimiter");
            for (int index = 0; index < count; index++) {
                if (buffer[index] == '\n') {
                    if (bytes.size() == 0) throw new IOException("Native GUI probe returned an empty response");
                    return bytes.toString(StandardCharsets.UTF_8.name());
                }
                if (bytes.size() >= MAX_RESPONSE_BYTES)
                    throw new IOException("Native GUI probe response exceeds 32 KB");
                bytes.write(buffer[index]);
            }
        }
    }

    @Override public Cursor query(Uri u, String[] p, String s, String[] a, String o) { throw new UnsupportedOperationException(); }
    @Override public String getType(Uri u) { return null; }
    @Override public Uri insert(Uri u, ContentValues v) { throw new UnsupportedOperationException(); }
    @Override public int delete(Uri u, String s, String[] a) { throw new UnsupportedOperationException(); }
    @Override public int update(Uri u, ContentValues v, String s, String[] a) { throw new UnsupportedOperationException(); }
}

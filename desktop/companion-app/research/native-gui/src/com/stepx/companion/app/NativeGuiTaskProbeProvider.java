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
import android.os.IBinder;
import android.os.Parcel;
import android.os.Process;
import android.os.SystemClock;
import org.json.JSONObject;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.net.SocketTimeoutException;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.Callable;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.Future;
import java.util.concurrent.RejectedExecutionException;
import java.util.concurrent.SynchronousQueue;
import java.util.concurrent.ThreadPoolExecutor;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;

/** Fixed native Settings test only. Included solely by the explicit research build. */
public final class NativeGuiTaskProbeProvider extends ContentProvider {
    private static final String SOCKET = "stepx-native-gui-test-v1";
    private static final String TARGET = "com.stepos.ability";
    private static final String APK_SHA256 = "c9f56f3078df96d8d726422b1c622a1c19fe8c7dec8cadd8537d9a6ecf513ac4";
    private static final int TIMEOUT_MS = 3000;
    private static final int SHUTDOWN_TIMEOUT_MS = 4000;
    private static final int MAX_RESPONSE_BYTES = 32 * 1024;
    private static final String DESCRIPTOR = "stepx.NativeGuiTaskControl.v1";
    private static final Object CONTROLLER_LOCK = new Object();
    private static Controller controller;
    // A timed-out synchronous Binder transaction cannot be cancelled remotely.
    // Keep at most one outstanding call and no queue; never retry via the socket.
    private static final ThreadPoolExecutor BINDER_CALLS = new ThreadPoolExecutor(1, 1, 0L,
            TimeUnit.MILLISECONDS, new SynchronousQueue<Runnable>(), runnable -> {
                Thread thread = new Thread(runnable, "StepXNativeGuiProbeBinder");
                thread.setDaemon(true);
                return thread;
            }, new ThreadPoolExecutor.AbortPolicy());

    private static final class Controller implements IBinder.DeathRecipient {
        final IBinder binder;
        final int pid;
        Controller(IBinder binder, int pid) { this.binder = binder; this.pid = pid; }
        @Override public void binderDied() {
            synchronized (CONTROLLER_LOCK) { if (controller == this) controller = null; }
        }
    }

    @Override public boolean onCreate() { return true; }

    @Override public Bundle call(String method, String arg, Bundle extras) {
        int caller = Binder.getCallingUid();
        int callingPid = Binder.getCallingPid();
        boolean register = "register_binder".equals(method);
        boolean unregister = "unregister_binder".equals(method);
        if (register || unregister) {
            if (caller != 1000 || callingPid <= 1)
                throw new SecurityException("Only the system-owned native host may publish its controller");
            if (arg != null || extras == null || extras.size() != 1
                    || !extras.containsKey("controller") || extras.getBinder("controller") == null)
                throw new IllegalArgumentException("Controller publication accepts only one controller Binder");
            long identity = Binder.clearCallingIdentity();
            try {
                return register ? registerController(extras.getBinder("controller"), callingPid)
                        : unregisterController(extras.getBinder("controller"), callingPid);
            } catch (Exception error) { return errorResult(error, "binder"); }
            finally { Binder.restoreCallingIdentity(identity); }
        }
        if (caller != 2000 && caller != Process.myUid())
            throw new SecurityException("Native GUI task probe is restricted to the ADB shell and this companion");
        if (!isOperation(method) || arg != null || (extras != null && !extras.isEmpty()))
            throw new IllegalArgumentException("Only status, start_test, pause, resume, stop and shutdown are supported without arguments");

        long identity = Binder.clearCallingIdentity();
        Bundle result = new Bundle();
        Controller current;
        synchronized (CONTROLLER_LOCK) { current = controller; }
        String transport = current == null ? "socket" : "binder";
        try {
            long deadline = SystemClock.elapsedRealtime()
                    + ("shutdown".equals(method) ? SHUTDOWN_TIMEOUT_MS : TIMEOUT_MS);
            if (current != null) {
                JSONObject body = boundedBinder(() -> invokeController(current, method, true), deadline);
                result.putBoolean("ok", body.getBoolean("ok"));
                result.putString("result", body.toString());
                result.putString("transport", transport);
                return result;
            }
            // No control is sent until the fixed native adapter has identified itself.
            Reply initial = exchange("status", deadline, -1, false);
            Reply reply = initial;
            if (initial.body.getBoolean("ok") && !"status".equals(method))
                reply = exchange(method, deadline, initial.peerPid, "shutdown".equals(method));
            result.putBoolean("ok", reply.body.getBoolean("ok"));
            result.putString("result", reply.body.toString());
            result.putString("transport", transport);
        } catch (Exception error) {
            result = errorResult(error, transport);
        } finally {
            Binder.restoreCallingIdentity(identity);
        }
        return result;
    }

    private static Bundle errorResult(Exception error, String transport) {
        String message = error.getClass().getSimpleName() + ": " + String.valueOf(error.getMessage());
        message = message.replaceAll("[\\r\\n\\t]", " ");
        if (message.length() > 240) message = message.substring(0, 240);
        Bundle result = new Bundle();
        result.putBoolean("ok", false);
        result.putString("error", message);
        result.putString("transport", transport);
        return result;
    }

    private static Bundle registerController(IBinder binder, int pid) throws Exception {
        synchronized (CONTROLLER_LOCK) {
            if (controller != null && (controller.pid != pid || !controller.binder.equals(binder)))
                throw new SecurityException("A different native controller is already registered");
        }
        Controller candidate = new Controller(binder, pid);
        JSONObject status = boundedBinder(() -> {
            if (!DESCRIPTOR.equals(binder.getInterfaceDescriptor()))
                throw new SecurityException("Native GUI controller descriptor mismatch");
            JSONObject body = invokeController(candidate, "status", false);
            if (!body.getBoolean("ok"))
                throw new IOException("Native GUI controller did not confirm readiness");
            verifyIdentity(body, pid, true);
            if (!"fixed_settings_about_test".equals(body.optString("mode"))
                    || !Boolean.FALSE.equals(body.opt("arbitrary_tasks_enabled"))
                    || !Boolean.TRUE.equals(body.opt("running")))
                throw new SecurityException("Native GUI controller is not the reviewed fixed test");
            return body;
        }, SystemClock.elapsedRealtime() + TIMEOUT_MS);
        synchronized (CONTROLLER_LOCK) {
            if (controller != null && (controller.pid != pid || !controller.binder.equals(binder)))
                throw new SecurityException("Native GUI controller changed during registration");
            if (controller == null) {
                binder.linkToDeath(candidate, 0);
                controller = candidate;
            }
            if (!binder.isBinderAlive()) {
                Controller old = controller;
                controller = null;
                binder.unlinkToDeath(old, 0);
                throw new IOException("Native GUI controller exited during registration");
            }
        }
        Bundle result = new Bundle();
        result.putBoolean("ok", true);
        result.putBoolean("accepted", true);
        result.putInt("version", 1);
        result.putInt("pid", pid);
        result.putString("transport", "binder");
        result.putString("result", status.toString());
        return result;
    }

    private static Bundle unregisterController(IBinder binder, int pid) {
        synchronized (CONTROLLER_LOCK) {
            if (controller != null) {
                if (controller.pid != pid || !controller.binder.equals(binder))
                    throw new SecurityException("Only the registered native host may revoke its controller");
                Controller old = controller;
                controller = null;
                old.binder.unlinkToDeath(old, 0);
            }
        }
        Bundle result = new Bundle();
        result.putBoolean("ok", true);
        result.putBoolean("accepted", true);
        result.putBoolean("registered", false);
        result.putInt("version", 1);
        result.putString("transport", "binder");
        return result;
    }

    private static JSONObject boundedBinder(Callable<JSONObject> action, long deadline) throws Exception {
        remaining(deadline);
        Future<JSONObject> pending;
        try { pending = BINDER_CALLS.submit(action); }
        catch (RejectedExecutionException busy) {
            throw new IOException("Native GUI Binder request is still pending; no new request was sent");
        }
        try { return pending.get(remaining(deadline), TimeUnit.MILLISECONDS); }
        catch (SocketTimeoutException timeout) {
            pending.cancel(false);
            throw new SocketTimeoutException("Native GUI Binder deadline elapsed; outcome is unconfirmed and the request was not retried");
        }
        catch (TimeoutException timeout) {
            pending.cancel(false);
            throw new SocketTimeoutException("Native GUI Binder timed out; outcome is unconfirmed and the request was not retried");
        } catch (InterruptedException interrupted) {
            pending.cancel(false);
            Thread.currentThread().interrupt();
            throw new IOException("Native GUI Binder wait interrupted; outcome is unconfirmed");
        } catch (ExecutionException failed) {
            Throwable error = failed.getCause();
            if (error instanceof Exception) throw (Exception) error;
            throw new IOException("Native GUI Binder request failed");
        }
    }

    private static JSONObject invokeController(Controller target, String operation,
                                               boolean requireRegistered) throws Exception {
        if (!isOperation(operation)) throw new IllegalArgumentException("Unsupported native GUI operation");
        if (requireRegistered) synchronized (CONTROLLER_LOCK) {
            if (controller != target) throw new IOException("Native GUI controller was revoked before dispatch");
        }
        if (!target.binder.isBinderAlive()) throw new IOException("Native GUI controller is no longer alive");
        Parcel request = Parcel.obtain(), reply = Parcel.obtain();
        long identity = Binder.clearCallingIdentity();
        try {
            request.writeInterfaceToken(DESCRIPTOR);
            request.writeString(operation);
            if (!target.binder.transact(IBinder.FIRST_CALL_TRANSACTION, request, reply, 0))
                throw new IOException("Native GUI controller does not implement this protocol");
            reply.readException();
            String raw = reply.readString();
            if (raw == null || raw.length() > MAX_RESPONSE_BYTES
                    || raw.getBytes(StandardCharsets.UTF_8).length > MAX_RESPONSE_BYTES || reply.dataAvail() != 0)
                throw new IOException("Native GUI Binder returned an invalid or oversized response");
            JSONObject body = new JSONObject(raw);
            if (body.optInt("version", -1) != 1 || !(body.opt("ok") instanceof Boolean))
                throw new IOException("Native GUI Binder protocol mismatch");
            // Registration authenticated the real Binder caller PID. Successful normal
            // replies must retain every field; shutdown may omit fields after cleanup.
            verifyIdentity(body, target.pid, body.getBoolean("ok") && !"shutdown".equals(operation));
            return body;
        } finally {
            Binder.restoreCallingIdentity(identity);
            request.recycle(); reply.recycle();
        }
    }

    private static boolean isOperation(String op) {
        return "status".equals(op) || "start_test".equals(op) || "pause".equals(op)
                || "resume".equals(op) || "stop".equals(op) || "shutdown".equals(op);
    }

    private static final class Reply {
        final JSONObject body;
        final int peerPid;
        Reply(JSONObject body, int peerPid) { this.body = body; this.peerPid = peerPid; }
    }

    private static Reply exchange(String operation, long deadline, int expectedPid,
                                  boolean allowShutdownIdentityOmission) throws Exception {
        remaining(deadline);
        try (LocalSocket socket = new LocalSocket()) {
            socket.connect(new LocalSocketAddress(SOCKET, LocalSocketAddress.Namespace.ABSTRACT));
            socket.setSoTimeout(remaining(deadline));
            // SO_PEERCRED authenticates this Unix socket, independently of Binder callers.
            Credentials peer = socket.getPeerCredentials();
            if (peer == null || peer.getUid() != 1000 || peer.getPid() <= 0
                    || (expectedPid > 0 && peer.getPid() != expectedPid))
                throw new SecurityException("Native GUI task probe server identity changed or is not system-owned");
            byte[] request = (new JSONObject().put("op", operation).toString() + "\n")
                    .getBytes(StandardCharsets.UTF_8);
            socket.getOutputStream().write(request);
            socket.getOutputStream().flush();
            JSONObject response = new JSONObject(readResponse(socket, deadline));
            if (response.optInt("version", -1) != 1 || !(response.opt("ok") instanceof Boolean))
                throw new IOException("Native GUI task probe returned an invalid response");
            boolean requireAll = response.getBoolean("ok") && !allowShutdownIdentityOmission;
            verifyIdentity(response, peer.getPid(), requireAll);
            // Omitted shutdown identity is safe only after the successful status handshake
            // and matching SO_PEERCRED on this connection. All supplied fields still match.
            if (allowShutdownIdentityOmission && expectedPid <= 0)
                throw new SecurityException("Shutdown requires a verified native adapter");
            return new Reply(response, peer.getPid());
        }
    }

    private static void verifyIdentity(JSONObject response, int peerPid, boolean requireAll)
            throws Exception {
        checkString(response, "socket", SOCKET, requireAll);
        checkString(response, "target", TARGET, requireAll);
        checkString(response, "apk_sha256", APK_SHA256, requireAll);
        checkInt(response, "pid", peerPid, requireAll);
        // The adapter reports the exact companion UID it admitted, not its own UID 1000.
        checkInt(response, "uid", Process.myUid(), requireAll);
    }

    private static void checkString(JSONObject response, String key, String expected,
                                    boolean required) throws Exception {
        if ((required || response.has(key)) && !expected.equals(response.opt(key)))
            throw new SecurityException("Native GUI task probe identity mismatch: " + key);
    }

    private static void checkInt(JSONObject response, String key, int expected,
                                 boolean required) throws Exception {
        if ((required || response.has(key)) && (!(response.opt(key) instanceof Number)
                || ((Number) response.get(key)).doubleValue() != expected))
            throw new SecurityException("Native GUI task probe identity mismatch: " + key);
    }

    private static int remaining(long deadline) throws SocketTimeoutException {
        long remaining = deadline - SystemClock.elapsedRealtime();
        if (remaining <= 0) throw new SocketTimeoutException("Native GUI task probe timed out");
        return (int) remaining;
    }

    private static String readResponse(LocalSocket socket, long deadline) throws IOException {
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        InputStream input = socket.getInputStream();
        byte[] buffer = new byte[1024];
        while (true) {
            socket.setSoTimeout(remaining(deadline));
            int count = input.read(buffer);
            if (count < 0) throw new IOException("Native GUI task probe response ended before its delimiter");
            for (int index = 0; index < count; index++) {
                if (buffer[index] == '\n') {
                    if (bytes.size() == 0) throw new IOException("Native GUI task probe returned an empty response");
                    return bytes.toString(StandardCharsets.UTF_8.name());
                }
                if (bytes.size() >= MAX_RESPONSE_BYTES)
                    throw new IOException("Native GUI task probe response exceeds 32 KiB");
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

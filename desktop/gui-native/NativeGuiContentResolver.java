package stepx;

import android.content.ContentResolver;
import android.content.Context;
import android.content.IContentProvider;
import android.content.pm.ApplicationInfo;
import android.content.pm.PackageManager;
import android.content.pm.ProviderInfo;
import android.os.Binder;
import android.os.Build;
import android.os.IBinder;
import android.os.Process;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.List;
import org.json.JSONObject;

/**
 * Research host resolver for two reviewed system providers. The real UID and
 * Context attribution remain unchanged; AMS and each provider enforce access.
 *
 * The IContentProvider compile stub must never be included in an APK or DEX.
 * Hidden acquire/release signatures below match Android 16 ContentResolver.
 */
public final class NativeGuiContentResolver extends ContentResolver implements AutoCloseable {
    private static final String TARGET = "com.stepos.ability";
    private static final String PERMISSION = "android.permission.ACCESS_CONTENT_PROVIDERS_EXTERNALLY";
    private static final String SETTINGS = "settings";
    private static final String OAID = "com.stepos.secureservice.openidsdk";
    private static final String COMPANION = "com.stepx.companion.app";
    private static final String CONTROL = "com.stepx.companion.app.native_gui_task_probe";
    private final PackageManager packages;
    private final String companionSigner;
    private final int companionUid;
    private static final int MAX_REFERENCES = 32;
    private final Object activityManager;
    private final Method acquireExternal;
    private final Method releaseExternal;
    private final List<Lease> leases = new ArrayList<>();
    private boolean closed;
    private int acquireAttempts;
    private int acquired;
    private int released;
    private int denied;
    private String lastError = "";

    private static final class Lease {
        final String authority;
        final Binder token = new Binder();
        final boolean stable;
        IBinder providerBinder;
        Lease(String authority, boolean stable) { this.authority = authority; this.stable = stable; }
    }

    public NativeGuiContentResolver(Context originalContext, String signer, int uid) throws Exception {
        super(originalContext);
        // createPackageContext retains the system Context's existing android
        // attribution on this framework. Accept only that real, UID-verified
        // source or the original package source; never manufacture attribution.
        if (Build.VERSION.SDK_INT != 36 || Process.myUid() != 1000
                || originalContext == null || !TARGET.equals(originalContext.getPackageName())
                || originalContext.getApplicationInfo().uid != Process.myUid()
                || originalContext.getAttributionSource().getUid() != Process.myUid()
                || !(TARGET.equals(originalContext.getAttributionSource().getPackageName())
                    || "android".equals(originalContext.getAttributionSource().getPackageName()))
                || !originalContext.getAttributionSource().getPackageName().equals(originalContext.getOpPackageName())
                || originalContext.getPackageManager().getApplicationInfo(
                    originalContext.getAttributionSource().getPackageName(), 0).uid != Process.myUid())
            throw new SecurityException("native_provider_context_identity_mismatch");
        if (originalContext.checkSelfPermission(PERMISSION) != PackageManager.PERMISSION_GRANTED)
            throw new SecurityException("native_provider_external_permission_missing");
        if (signer == null || !signer.matches("[0-9a-f]{64}") || uid < 10000 || uid >= 100000)
            throw new SecurityException("native_provider_companion_identity_invalid");
        packages = originalContext.getPackageManager(); companionSigner = signer; companionUid = uid;
        verifyCompanion();
        Class<?> manager = Class.forName("android.app.IActivityManager");
        activityManager = Class.forName("android.app.ActivityManager").getMethod("getService").invoke(null);
        if (activityManager == null || !manager.isInstance(activityManager))
            throw new IllegalStateException("native_provider_activity_manager_missing");
        acquireExternal = manager.getMethod("getContentProviderExternal", String.class, int.class,
                IBinder.class, String.class);
        releaseExternal = manager.getMethod("removeContentProviderExternalAsUser", String.class,
                IBinder.class, int.class);
    }

    private static String expectedPackage(String authority) {
        if (SETTINGS.equals(authority)) return "com.android.providers.settings";
        if (OAID.equals(authority)) return "com.stepos.secureservice";
        if (CONTROL.equals(authority)) return COMPANION;
        return null;
    }

    private void verifyCompanion() throws Exception {
        android.content.pm.PackageInfo info = packages.getPackageInfo(COMPANION, PackageManager.GET_SIGNING_CERTIFICATES);
        String[] owners = packages.getPackagesForUid(companionUid);
        if (info.applicationInfo == null || info.applicationInfo.uid != companionUid || info.signingInfo == null
                || owners == null || owners.length != 1 || !COMPANION.equals(owners[0]))
            throw new SecurityException("native_provider_companion_owner_changed");
        android.content.pm.Signature[] signers = info.signingInfo.getApkContentsSigners();
        if (signers.length != 1) throw new SecurityException("native_provider_companion_signer_changed");
        byte[] bytes = java.security.MessageDigest.getInstance("SHA-256").digest(signers[0].toByteArray());
        StringBuilder hex = new StringBuilder();
        for (byte b : bytes) hex.append(String.format(java.util.Locale.ROOT, "%02x", b & 255));
        if (!companionSigner.equals(hex.toString())) throw new SecurityException("native_provider_companion_signer_changed");
    }

    // @Override is intentionally omitted for methods absent from public android.jar.
    protected IContentProvider acquireProvider(Context context, String authority) {
        return acquire(authority, true);
    }

    protected IContentProvider acquireExistingProvider(Context context, String authority) {
        // External acquisition may start a provider; do not present it as an existing-only lookup.
        throw failure("native_provider_existing_only_unsupported", null);
    }

    protected IContentProvider acquireUnstableProvider(Context context, String authority) {
        // AMS external references are stable. Track the resolver's stable/unstable
        // release distinction locally so each acquired external reference is released once.
        return acquire(authority, false);
    }

    private synchronized IContentProvider acquire(String authority, boolean stable) {
        if (closed) throw failure("native_provider_resolver_closed", null);
        String owner = expectedPackage(authority);
        if (owner == null) {
            denied++;
            throw failure("native_provider_authority_not_reviewed", null);
        }
        if (leases.size() >= MAX_REFERENCES)
            throw failure("native_provider_reference_limit", null);
        Lease lease = new Lease(authority, stable);
        // Keep even a failed/ambiguous acquisition until its token is released.
        // Binder death also lets AMS reclaim these external references on host exit.
        leases.add(lease);
        acquireAttempts++;
        try {
            Object holder = acquireExternal.invoke(activityManager, authority, 0, lease.token,
                    "StepXNativeGuiResearch");
            if (holder == null) {
                releaseLease(lease);
                lastError = "native_provider_unavailable";
                return null;
            }
            Object infoObject = holder.getClass().getField("info").get(holder);
            if (!(infoObject instanceof ProviderInfo))
                throw new SecurityException("native_provider_metadata_missing");
            ProviderInfo info = (ProviderInfo) infoObject;
            boolean authorityMatches = false;
            if (info.authority != null)
                for (String registered : info.authority.split(";"))
                    if (authority.equals(registered)) authorityMatches = true;
            if (!owner.equals(info.packageName) || !authorityMatches || info.applicationInfo == null
                    || (!CONTROL.equals(authority) && (info.applicationInfo.flags & (ApplicationInfo.FLAG_SYSTEM
                    | ApplicationInfo.FLAG_UPDATED_SYSTEM_APP)) == 0)
                    || (CONTROL.equals(authority) && info.applicationInfo.uid != companionUid))
                throw new SecurityException("native_provider_owner_mismatch");
            if (CONTROL.equals(authority)) verifyCompanion();
            Object providerObject = holder.getClass().getField("provider").get(holder);
            if (!(providerObject instanceof IContentProvider))
                throw new IllegalStateException("native_provider_binder_missing");
            IContentProvider provider = (IContentProvider) providerObject;
            lease.providerBinder = provider.asBinder();
            if (lease.providerBinder == null || !lease.providerBinder.isBinderAlive())
                throw new IllegalStateException("native_provider_binder_dead");
            acquired++;
            return provider;
        } catch (Throwable error) {
            try { releaseLease(lease); } catch (Throwable releaseFailure) {
                // The failed token remains in leases; close retries it and reports failure.
                lastError = "native_provider_release_pending";
                throw failure("native_provider_acquire_cleanup_pending", error);
            }
            throw failure("native_provider_acquire_failed", error);
        }
    }

    public synchronized boolean releaseProvider(IContentProvider provider) { return release(provider, true); }
    public synchronized boolean releaseUnstableProvider(IContentProvider provider) { return release(provider, false); }

    private boolean release(IContentProvider provider, boolean stable) {
        if (provider == null || provider.asBinder() == null) return false;
        IBinder binder = provider.asBinder();
        for (Lease lease : new ArrayList<>(leases)) {
            if (lease.stable == stable && binder.equals(lease.providerBinder)) {
                try { releaseLease(lease); return true; }
                catch (Throwable error) { throw failure("native_provider_release_pending", error); }
            }
        }
        return false;
    }

    private void releaseLease(Lease lease) throws Exception {
        if (!leases.contains(lease)) return;
        releaseExternal.invoke(activityManager, lease.authority, lease.token, 0);
        leases.remove(lease);
        released++;
    }

    public synchronized void unstableProviderDied(IContentProvider provider) {
        // ContentResolver releases its unstable reference in finally. We have no
        // normal AMS ContentProviderConnection, so never call unstableProviderDied
        // with an external token or discard the pending matching release.
        lastError = "native_provider_died";
    }

    public void appNotRespondingViaProvider(IContentProvider provider) {
        throw failure("native_provider_anr_reporting_unsupported", null);
    }

    /** Releases static Settings cache references as well as ordinary resolver clients. */
    @Override public synchronized void close() {
        closed = true;
        Throwable firstFailure = null;
        for (Lease lease : new ArrayList<>(leases)) {
            try { releaseLease(lease); }
            catch (Throwable error) { if (firstFailure == null) firstFailure = error; }
        }
        if (!leases.isEmpty()) throw failure("native_provider_close_incomplete", firstFailure);
    }

    /** Counters/classifications only: never provider values, account names, OAIDs or tokens. */
    public synchronized JSONObject diagnostics() {
        JSONObject value = new JSONObject();
        try {
            value.put("closed", closed).put("acquire_attempts", acquireAttempts)
                    .put("providers_returned", acquired).put("release_calls", released)
                    .put("outstanding", leases.size()).put("denied_authorities", denied)
                    .put("last_error", lastError);
        } catch (Exception impossible) { throw new IllegalStateException("native_provider_diagnostics_failed"); }
        return value;
    }

    private synchronized IllegalStateException failure(String category, Throwable error) {
        lastError = category;
        while (error instanceof InvocationTargetException && error.getCause() != null)
            error = error.getCause();
        // Remote messages may contain provider data. Retain only the exception type,
        // without attaching its cause to a later host diagnostic stack or message.
        return new IllegalStateException(category + (error == null ? "" : ":" + error.getClass().getSimpleName()));
    }
}

package stepx;

import android.content.Context;
import android.content.pm.ApplicationInfo;
import android.os.Process;
import android.system.ErrnoException;
import android.system.Os;
import android.system.OsConstants;
import android.system.StructStat;
import java.io.File;
import java.io.FileDescriptor;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.lang.reflect.Method;
import java.security.MessageDigest;
import java.util.Arrays;
import java.util.HashMap;
import java.util.HashSet;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import java.util.UUID;
import org.json.JSONObject;

/**
 * DISABLED: this native MMKV read-only-copy path crashed the disposable phone
 * host. Retained only as failure evidence; excluded from the host and builder.
 * Use the bounded pure Python metadata reader for any further investigation.
 *
 * Research only: extract the original GUI model selection from one MMKV ID.
 * Call after the isolated host initializes MMKV, passing the real package
 * Context separately. Never points MMKV at the live original files, changes
 * global rootDir, enumerates keys, reads accounts, or selects/executes a model.
 *
 * The only original files read are files/mmkv/model_config_prefs and its .crc.
 * Both are bounded, owned by UID 1000, non-symlink regular files. Their private
 * copies are opened using pinned MMKV READ_ONLY_MODE|SINGLE_PROCESS_MODE, then
 * closed and deleted. Original content is checked again after decoding; this
 * is a stable-copy check, not a lock against all future original-app writes.
 *
 * A saved ID/cell is configuration evidence only, never proof of account
 * permission, a deployed cloud instance, or correct GUI action generation.
 * No production caller or automatic task fallback is provided.
 */
public final class NativeGuiModelPreferences {
    private static final String TARGET = "com.stepos.ability";
    private static final String APK = "/system/priv-app/ability/ability.apk";
    private static final String SHA = "c9f56f3078df96d8d726422b1c622a1c19fe8c7dec8cadd8537d9a6ecf513ac4";
    private static final String ID = "model_config_prefs";
    private static final String CATALOG = "stepai.aios.guiagent.llm.cell.config.ModelCatalog";
    private NativeGuiModelPreferences() { }

    public static JSONObject snapshot(Context original, Context isolated) throws Exception {
        require(original != null && isolated != null && original != isolated && Process.myUid() == 1000);
        require(TARGET.equals(original.getPackageName()) && TARGET.equals(isolated.getPackageName()));
        require("stepx.NativeGuiTaskHost$HostApplication".equals(isolated.getClass().getName()));
        ApplicationInfo info = original.getApplicationInfo();
        require(info.uid == 1000 && APK.equals(info.sourceDir) && SHA.equals(hashFile(new File(APK))));
        File data = new File(info.dataDir).getCanonicalFile();
        require(data.equals(new File("/data/user/0/" + TARGET).getCanonicalFile()));
        File originalFiles = original.getFilesDir().getCanonicalFile();
        require(originalFiles.equals(new File(data, "files")));
        File source = new File(originalFiles, "mmkv");
        File hostRoot = isolated.getDataDir().getCanonicalFile();
        require(hostRoot.getParentFile().equals(new File(data, "code_cache"))
                && hostRoot.getName().matches("stepx-gui-task-host-[0-9a-f-]{36}"));
        require(!isolated.getFilesDir().getCanonicalFile().equals(originalFiles));
        ClassLoader loader = isolated.getClassLoader();
        Class<?> mmkv = Class.forName("com.tencent.mmkv.MMKV", false, loader);
        String globalRoot = (String) mmkv.getMethod("getRootDir").invoke(null);
        require(globalRoot != null && new File(globalRoot).getCanonicalFile()
                .equals(new File(isolated.getFilesDir(), "mmkv").getCanonicalFile()));
        require(mmkv.getField("READ_ONLY_MODE").getInt(null) == 32
                && mmkv.getField("SINGLE_PROCESS_MODE").getInt(null) == 1);
        JSONObject out = new JSONObject().put("version", 1).put("source", "original_gui_model_preferences")
                .put("source_id", ID).put("permission_verified", false).put("source_modified", false);
        File sourceData = new File(source, ID), sourceCrc = new File(source, ID + ".crc");
        if (absent(sourceData) && absent(sourceCrc)) return out.put("status", "absent").put("source_exists", false);
        require(!absent(sourceData) && !absent(sourceCrc));
        require(source.getCanonicalFile().equals(source));
        requireDirectory(source);
        byte[] content = readOwned(sourceData, 1048576), crc = readOwned(sourceCrc, 65536);
        File copy = new File(isolated.getCacheDir().getCanonicalFile(), "gui-model-snapshot-" + UUID.randomUUID());
        require(copy.getParentFile().toPath().startsWith(hostRoot.toPath()));
        Object store = null;
        boolean made = false;
        try {
            Os.mkdir(copy.getAbsolutePath(), 0700); made = true;
            requireDirectory(copy); require(copy.getCanonicalFile().equals(copy));
            writeNew(new File(copy, ID), content); writeNew(new File(copy, ID + ".crc"), crc);
            require(Boolean.TRUE.equals(mmkv.getMethod("isFileValid", String.class, String.class)
                    .invoke(null, ID, copy.getAbsolutePath())));
            store = mmkv.getMethod("mmkvWithID", String.class, int.class, String.class, String.class)
                    .invoke(null, ID, 33, null, copy.getAbsolutePath());
            require(store != null);
            Method decode = mmkv.getMethod("decodeString", String.class, String.class);
            String current = (String) decode.invoke(store, "current_model_id", null);
            String modelMap = (String) decode.invoke(store, "model_map", null);
            Map<String, String> builtins = new HashMap<>();
            Class<?> catalog = Class.forName(CATALOG, false, loader);
            Object catalogInstance = catalog.getField("INSTANCE").get(null);
            String defaultId = (String) catalog.getMethod("getDEFAULT_MODEL_ID").invoke(catalogInstance);
            Object[] entries = (Object[]) catalog.getMethod("getITEMS").invoke(catalogInstance);
            require(entries.length > 0 && entries.length <= 32);
            for (Object entry : entries) {
                String id = (String) entry.getClass().getMethod("getModelId").invoke(entry);
                String cell = (String) entry.getClass().getMethod("getModelCellType").invoke(entry);
                require(validId(id) && validCell(cell)); builtins.put(id, cell);
            }
            describeSelection(current, modelMap, defaultId, builtins, out);
            require(Arrays.equals(content, readOwned(sourceData, 1048576))
                    && Arrays.equals(crc, readOwned(sourceCrc, 65536)));
            require(Arrays.equals(content, readOwned(new File(copy, ID), 1048576))
                    && Arrays.equals(crc, readOwned(new File(copy, ID + ".crc"), 65536)));
            require(globalRoot.equals(mmkv.getMethod("getRootDir").invoke(null)));
            out.put("source_exists", true).put("stable_copy_verified", true).put("read_only_copy", true);
        } finally {
            try {
                if (store != null) mmkv.getMethod("close").invoke(store);
            } finally {
                Arrays.fill(content, (byte) 0); Arrays.fill(crc, (byte) 0);
                if (made) {
                    requireDirectory(copy);
                    String[] children = copy.list(); require(children != null && children.length <= 2);
                    for (String name : children) {
                        require(ID.equals(name) || (ID + ".crc").equals(name));
                        File owned = new File(copy, name); requireFile(owned, 1048576);
                        require(owned.delete());
                    }
                    require(copy.delete()); out.put("snapshot_removed", true);
                }
            }
        }
        return out;
    }

    // This pure filtering helper is separately exercised with hostile values.
    private static void describeSelection(String current, String modelMap, String defaultId,
            Map<String, String> builtins, JSONObject out) throws Exception {
        out.put("saved_selection_present", current != null && !current.isEmpty());
        if (current == null || current.isEmpty()) { out.put("status", "no_saved_selection"); return; }
        if (!validId(current)) { out.put("status", "invalid_selection"); return; }
        String cell = builtins.get(current);
        boolean custom = cell == null;
        if (custom && modelMap != null && !modelMap.isEmpty()) {
            require(modelMap.length() <= 131072);
            Object value = new JSONObject(modelMap).opt(current);
            if (value instanceof String && validCell((String) value)) cell = (String) value;
        }
        // Unregistered IDs are ignored by the original loader too. Do not expose
        // arbitrary strings that merely happen to occupy current_model_id.
        if (cell == null) { out.put("status", "unregistered_selection"); return; }
        out.put("status", "saved_selection").put("current_model_id", current)
                .put("model_cell_type", cell).put("matches_static_default", current.equals(defaultId))
                .put("custom_model", custom);
    }

    private static boolean validId(String value) {
        return value != null && value.matches("[A-Za-z0-9][A-Za-z0-9._/+:-]{0,159}")
                && !value.contains("://") && !value.contains("..");
    }
    private static boolean validCell(String value) {
        return value != null && new HashSet<>(Arrays.asList("StepPlanTreeModelCell", "DoubaoTarsModelCell",
                "DoubaoSeedModelCell", "GeminiModelCell", "StepGelabModelCell", "GlmAtuoModelCell",
                "GLMModelCell", "MiMoModelCell", "LocalModelCell", "LocalCloudModelCell")).contains(value);
    }
    private static boolean absent(File file) throws Exception {
        try { Os.lstat(file.getAbsolutePath()); return false; }
        catch (ErrnoException e) { if (e.errno == OsConstants.ENOENT) return true; throw e; }
    }
    private static void requireDirectory(File file) throws Exception {
        StructStat s = Os.lstat(file.getAbsolutePath());
        require(OsConstants.S_ISDIR(s.st_mode) && s.st_uid == 1000);
    }
    private static StructStat requireFile(File file, int limit) throws Exception {
        StructStat s = Os.lstat(file.getAbsolutePath());
        require(OsConstants.S_ISREG(s.st_mode) && s.st_uid == 1000 && s.st_nlink == 1
                && s.st_size >= 0 && s.st_size <= limit);
        return s;
    }
    private static byte[] readOwned(File file, int limit) throws Exception {
        StructStat before = requireFile(file, limit);
        FileDescriptor descriptor = Os.open(file.getAbsolutePath(), OsConstants.O_RDONLY | OsConstants.O_NOFOLLOW | OsConstants.O_CLOEXEC, 0);
        try (FileInputStream in = new FileInputStream(descriptor)) {
            StructStat opened = Os.fstat(descriptor);
            require(opened.st_ino == before.st_ino && opened.st_dev == before.st_dev && opened.st_size == before.st_size);
            byte[] bytes = new byte[(int) before.st_size]; int offset = 0, count;
            while (offset < bytes.length && (count = in.read(bytes, offset, bytes.length - offset)) >= 0) offset += count;
            require(offset == bytes.length && in.read() == -1);
            StructStat after = Os.fstat(descriptor);
            require(after.st_size == before.st_size && after.st_mtime == before.st_mtime && after.st_ctime == before.st_ctime);
            return bytes;
        }
    }
    private static void writeNew(File file, byte[] bytes) throws Exception {
        FileDescriptor descriptor = Os.open(file.getAbsolutePath(), OsConstants.O_WRONLY | OsConstants.O_CREAT | OsConstants.O_EXCL | OsConstants.O_NOFOLLOW | OsConstants.O_CLOEXEC, 0600);
        try (FileOutputStream out = new FileOutputStream(descriptor)) { out.write(bytes); out.getFD().sync(); }
    }
    private static String hashFile(File file) throws Exception {
        MessageDigest digest = MessageDigest.getInstance("SHA-256");
        try (FileInputStream in = new FileInputStream(file)) {
            byte[] buffer = new byte[65536]; int count;
            while ((count = in.read(buffer)) >= 0) digest.update(buffer, 0, count);
        }
        StringBuilder out = new StringBuilder();
        for (byte b : digest.digest()) out.append(String.format(Locale.ROOT, "%02x", b & 255));
        return out.toString();
    }
    private static void require(boolean value) {
        if (!value) throw new SecurityException("Native GUI model snapshot guard failed");
    }
}

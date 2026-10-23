import com.android.apksig.apk.ApkUtils;
import java.io.InputStream;
import java.nio.ByteBuffer;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;

/** Read package identity with Android's own parser, without a native aapt executable. */
public final class ApkIdentity {
    public static void main(String[] args) throws Exception {
        if (args.length != 1) throw new IllegalArgumentException("Expected one APK path");
        try (ZipFile apk = new ZipFile(args[0])) {
            ZipEntry entry = apk.getEntry("AndroidManifest.xml");
            if (entry == null) throw new IllegalArgumentException("APK manifest is missing");
            try (InputStream stream = apk.getInputStream(entry)) {
                byte[] manifest = stream.readNBytes(2 * 1024 * 1024 + 1);
                if (manifest.length > 2 * 1024 * 1024) throw new IllegalArgumentException("APK manifest is too large");
                String name = ApkUtils.getPackageNameFromBinaryAndroidManifest(ByteBuffer.wrap(manifest));
                if (name == null || !name.matches("[A-Za-z][A-Za-z0-9_]*(\\.[A-Za-z][A-Za-z0-9_]*)+"))
                    throw new IllegalArgumentException("Invalid APK package name");
                System.out.println(name);
            }
        }
    }
}

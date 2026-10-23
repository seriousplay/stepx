package com.stepx.companion.app;
import android.content.Context;
import android.security.keystore.*;
import android.util.Base64;
import java.security.*;
import java.security.spec.MGF1ParameterSpec;
import javax.crypto.*;
import javax.crypto.spec.*;
import java.nio.charset.StandardCharsets;

/** Phone-owned credentials. No plaintext file, log, MCP result or cloud skill. */
final class GuiModelVault {
    private static final String AES="stepx.gui.stepplan.v1", RSA="stepx.gui.import.v1";
    static boolean configured(Context c){return c.getSharedPreferences("gui_model",0).contains("sealed");}
    private static KeyStore store()throws Exception{KeyStore s=KeyStore.getInstance("AndroidKeyStore");s.load(null);return s;}
    static synchronized String publicKey()throws Exception{
        KeyStore s=store();
        if(!s.containsAlias(RSA)){
            KeyPairGenerator g=KeyPairGenerator.getInstance(KeyProperties.KEY_ALGORITHM_RSA,"AndroidKeyStore");
            g.initialize(new KeyGenParameterSpec.Builder(RSA,KeyProperties.PURPOSE_DECRYPT).setKeySize(3072)
                .setDigests(KeyProperties.DIGEST_SHA256,KeyProperties.DIGEST_SHA1).setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_RSA_OAEP).build());g.generateKeyPair();
        }
        return Base64.encodeToString(s.getCertificate(RSA).getPublicKey().getEncoded(),Base64.NO_WRAP);
    }
    static void importSealed(Context c,byte[] bytes)throws Exception {
        if(bytes.length!=384)throw new IllegalArgumentException("密钥密文无效");
        Cipher cipher=Cipher.getInstance("RSA/ECB/OAEPWithSHA-256AndMGF1Padding");
        cipher.init(Cipher.DECRYPT_MODE,store().getKey(RSA,null),new OAEPParameterSpec("SHA-256","MGF1",MGF1ParameterSpec.SHA1,PSource.PSpecified.DEFAULT));
        byte[] raw=cipher.doFinal(bytes);
        try{save(c,new String(raw,StandardCharsets.UTF_8));}finally{java.util.Arrays.fill(raw,(byte)0);}
    }
    static synchronized void save(Context c,String key)throws Exception {
        key=key.trim();if(key.length()<16||key.length()>512||key.matches(".*\\s.*"))throw new IllegalArgumentException("API Key 格式无效");
        KeyStore s=store();if(!s.containsAlias(AES)){
            KeyGenerator g=KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES,"AndroidKeyStore");
            g.init(new KeyGenParameterSpec.Builder(AES,KeyProperties.PURPOSE_ENCRYPT|KeyProperties.PURPOSE_DECRYPT).setBlockModes(KeyProperties.BLOCK_MODE_GCM).setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE).build());g.generateKey();
        }
        Cipher cipher=Cipher.getInstance("AES/GCM/NoPadding");cipher.init(Cipher.ENCRYPT_MODE,s.getKey(AES,null));
        if(!c.getSharedPreferences("gui_model",0).edit().putString("sealed",Base64.encodeToString(cipher.doFinal(key.getBytes(StandardCharsets.UTF_8)),Base64.NO_WRAP))
            .putString("iv",Base64.encodeToString(cipher.getIV(),Base64.NO_WRAP)).commit())throw new IllegalStateException("密钥保存失败");
    }
    static String read(Context c)throws Exception {
        android.content.SharedPreferences p=c.getSharedPreferences("gui_model",0);
        if(!configured(c))throw new IllegalStateException("请在伴侣中配置 Step Plan API Key");
        Cipher cipher=Cipher.getInstance("AES/GCM/NoPadding");cipher.init(Cipher.DECRYPT_MODE,store().getKey(AES,null),new GCMParameterSpec(128,Base64.decode(p.getString("iv",""),Base64.NO_WRAP)));
        return new String(cipher.doFinal(Base64.decode(p.getString("sealed",""),Base64.NO_WRAP)),StandardCharsets.UTF_8);
    }
}

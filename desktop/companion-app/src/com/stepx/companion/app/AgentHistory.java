package com.stepx.companion.app;
import android.content.Context;
import android.security.keystore.*;
import android.util.Base64;
import org.json.*;
import java.security.KeyStore;
import javax.crypto.*;
import javax.crypto.spec.GCMParameterSpec;
import java.nio.charset.StandardCharsets;

/** Bounded, encrypted terminal summaries. Never stores API keys, screenshots or model traces. */
final class AgentHistory {
    private static final String ALIAS="stepx.gui.history.v1";
    private static KeyStore keys()throws Exception{KeyStore k=KeyStore.getInstance("AndroidKeyStore");k.load(null);return k;}
    static synchronized JSONArray list(Context c)throws Exception{
        android.content.SharedPreferences p=c.getSharedPreferences("gui_history",0);String data=p.getString("data","");if(data.isEmpty())return new JSONArray();
        Cipher cipher=Cipher.getInstance("AES/GCM/NoPadding");cipher.init(Cipher.DECRYPT_MODE,keys().getKey(ALIAS,null),new GCMParameterSpec(128,Base64.decode(p.getString("iv",""),Base64.NO_WRAP)));
        return new JSONArray(new String(cipher.doFinal(Base64.decode(data,Base64.NO_WRAP)),StandardCharsets.UTF_8));
    }
    static synchronized void observe(Context c,JSONObject s){
        if(!s.optBoolean("terminal")||s.optString("task").isEmpty()||s.optLong("createdAt")<=c.getSharedPreferences("gui_history",0).getLong("clearedAt",0))return;
        try{
            JSONArray old=list(c),next=new JSONArray();JSONObject entry=new JSONObject();
            for(String field:new String[]{"task","goal","phase","progress","createdAt","actions","modelCalls","screenshots","result","model"})if(s.has(field))entry.put(field,s.opt(field));
            for(int i=0;i<old.length();i++)if(old.getJSONObject(i).optString("task").equals(entry.optString("task"))&&old.getJSONObject(i).toString().equals(entry.toString()))return;
            next.put(entry);for(int i=0;i<old.length()&&next.length()<20;i++)if(!old.getJSONObject(i).optString("task").equals(entry.optString("task")))next.put(old.get(i));
            KeyStore k=keys();if(!k.containsAlias(ALIAS)){KeyGenerator g=KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES,"AndroidKeyStore");g.init(new KeyGenParameterSpec.Builder(ALIAS,KeyProperties.PURPOSE_ENCRYPT|KeyProperties.PURPOSE_DECRYPT).setBlockModes(KeyProperties.BLOCK_MODE_GCM).setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE).build());g.generateKey();}
            Cipher cipher=Cipher.getInstance("AES/GCM/NoPadding");cipher.init(Cipher.ENCRYPT_MODE,k.getKey(ALIAS,null));
            c.getSharedPreferences("gui_history",0).edit().putString("data",Base64.encodeToString(cipher.doFinal(next.toString().getBytes(StandardCharsets.UTF_8)),Base64.NO_WRAP)).putString("iv",Base64.encodeToString(cipher.getIV(),Base64.NO_WRAP)).commit();
        }catch(Exception ignored){/* History failure must not change execution or completion evidence. */}
    }
    static synchronized void clear(Context c){c.getSharedPreferences("gui_history",0).edit().clear().putLong("clearedAt",System.currentTimeMillis()).commit();}
}

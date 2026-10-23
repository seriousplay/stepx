package com.stepx.companion.app;
import android.content.Context;
import org.json.*;
import java.io.*;
import java.nio.file.*;

/** Same phone-owned queue for native MCP, local phone UI and authorized desktop. */
final class GuiDelegation {
    static JSONObject desktop(Context c,JSONObject q)throws Exception {
        String op=q.getString("agentOp");
        if(op.equals("config_info"))return PhoneGuiAgent.status(c).put("publicKey",GuiModelVault.publicKey());
        if(op.equals("install_key")){
            File f=new File(c.getExternalFilesDir(null),"gui-key.sealed");
            try{if(!Files.isRegularFile(f.toPath(),LinkOption.NOFOLLOW_LINKS)||f.length()!=384)throw new IOException("密钥密文文件无效");GuiModelVault.importSealed(c,Files.readAllBytes(f.toPath()));}
            finally{f.delete();}
            return PhoneGuiAgent.status(c);
        }
        return PhoneGuiAgent.request(c,android.os.Process.myUid(),new JSONObject(q.toString()).put("op",op),true);
    }
    static JSONObject mcp(Context c,int uid,JSONObject q)throws Exception {return PhoneGuiAgent.request(c,uid,q,false);}
}

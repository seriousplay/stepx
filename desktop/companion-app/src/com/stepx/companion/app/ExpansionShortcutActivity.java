package com.stepx.companion.app;
import android.app.Activity;
import android.content.Intent;
import android.os.Bundle;
import android.widget.Toast;
import java.io.*;
import org.json.JSONObject;
/** The shortcut contains an opaque journal ID, never a trusted filesystem path. */
public final class ExpansionShortcutActivity extends Activity {
 public void onCreate(Bundle b){super.onCreate(b);new Thread(()->{try{if(getIntent().getData()==null)throw new IOException("文件入口无效");byte[] bytes;try(InputStream in=getContentResolver().openInputStream(getIntent().getData())){if(in==null)throw new IOException("无法读取入口");bytes=in.readNBytes(32769);}if(bytes.length>32768)throw new IOException("文件入口无效");JSONObject shortcut=new JSONObject(new String(bytes,java.nio.charset.StandardCharsets.UTF_8));if(!shortcut.optString("type").equals("stepx-storage-shortcut"))throw new IOException("不是存储扩容文件入口");JSONObject record=new ExpansionStore(this).get(shortcut.getString("id"));if(!record.getString("desktop").equals(shortcut.getString("desktopId")))throw new IOException("文件入口与原电脑不匹配");runOnUiThread(()->{startActivity(new Intent(this,ExpansionActivity.class).putExtra("fileId",record.optString("id")));finish();});}catch(Exception e){runOnUiThread(()->{Toast.makeText(this,e.getMessage(),Toast.LENGTH_LONG).show();finish();});}},"expansion-shortcut").start();}
}

package com.stepx.companion.app;

import android.content.*;
import android.database.Cursor;
import android.graphics.*;
import android.net.Uri;
import android.os.Environment;
import android.provider.*;
import org.json.JSONObject;
import java.io.*;
import java.nio.file.*;
import java.security.MessageDigest;
import java.util.Locale;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicReference;
import java.util.regex.*;

/** Real MediaStore previews, not fake original files or vendor-private cloud records. */
final class ExpansionGallery {
    private static final String ALBUM="Pictures/StepX Companion";
    private static final Pattern NAME=Pattern.compile("SC_([a-f0-9]{32})\\.jpg");
    private static File path(String id)throws Exception {
        if(!ExpansionStore.id(id))throw new IOException("无效文件编号");
        File root=Environment.getExternalStorageDirectory(),dir=new File(root,ALBUM);
        if(Files.isSymbolicLink(new File(root,"Pictures").toPath())||Files.isSymbolicLink(dir.toPath()))throw new IOException("相册目录不可用");
        if(!dir.isDirectory()&&!dir.mkdirs())throw new IOException("无法创建相册缩略图目录");
        File f=new File(dir,"SC_"+id+".jpg");if(Files.isSymbolicLink(f.toPath()))throw new IOException("相册缩略图路径不可用");return f;
    }
    static void publish(Context c,ExpansionStore store,JSONObject j) {
        if((!j.optString("state").equals("archived")&&!j.optString("state").equals("releasing"))||(!j.optString("kind").equals("photos")&&!j.optString("kind").equals("videos")))return;
        try {
            File f=path(j.getString("id"));
            if(j.has("galleryUri")&&j.optString("galleryError").isEmpty()&&j.has("gallerySha256")&&f.isFile()&&ExpansionStore.digest(f).equals(j.getString("gallerySha256")))return;
            Bitmap source=BitmapFactory.decodeFile(store.thumbnail(j.getString("id")).getPath());
            if(source==null)throw new IOException("此格式暂时无法生成相册预览，原件已安全保存在电脑");
            int w=480,h=Math.max(160,Math.min(720,Math.round(w*source.getHeight()/(float)source.getWidth())));
            Bitmap output=Bitmap.createBitmap(w,h+96,Bitmap.Config.ARGB_8888);Canvas canvas=new Canvas(output);canvas.drawColor(0xff18181b);
            Paint p=new Paint(Paint.ANTI_ALIAS_FLAG|Paint.FILTER_BITMAP_FLAG);float scale=Math.min(w/(float)source.getWidth(),h/(float)source.getHeight());float bw=source.getWidth()*scale,bh=source.getHeight()*scale;
            canvas.drawBitmap(source,null,new RectF((w-bw)/2,(h-bh)/2,(w+bw)/2,(h+bh)/2),p);
            p.setColor(Color.WHITE);p.setTextSize(23);p.setTypeface(Typeface.create("sans-serif-medium",0));canvas.drawText(j.optString("kind").equals("videos")?"视频原件在电脑 · 此图仅为预览":"原图在电脑 · 此图仅为预览",20,h+34,p);
            p.setTextSize(19);p.setColor(0xffb8b8c1);canvas.drawText("相册分享 → 取回电脑原图",20,h+69,p);
            ByteArrayOutputStream bytes=new ByteArrayOutputStream();output.compress(Bitmap.CompressFormat.JPEG,82,bytes);source.recycle();output.recycle();
            byte[] data=bytes.toByteArray();String hash=hash(data);
            // Never overwrite a thumbnail that the user has edited, or any unrelated file.
            if(f.exists()){if(!ExpansionStore.digest(f).equals(hash))throw new IOException("相册预览已被修改，保留现有图片");}
            else {try(OutputStream out=Files.newOutputStream(f.toPath(),StandardOpenOption.CREATE_NEW,StandardOpenOption.WRITE)){out.write(data);}try(java.nio.channels.FileChannel out=java.nio.channels.FileChannel.open(f.toPath(),StandardOpenOption.WRITE)){out.force(true);}}
            f.setLastModified(j.getLong("modified"));j.put("galleryPath",f.getPath()).put("gallerySha256",hash).put("galleryBytes",f.length()).put("galleryError","");store.put(j);
            CountDownLatch done=new CountDownLatch(1);AtomicReference<Uri> indexed=new AtomicReference<>();
            android.media.MediaScannerConnection.scanFile(c,new String[]{f.getPath()},new String[]{"image/jpeg"},(p2,u)->{indexed.set(u);done.countDown();});
            if(done.await(8,TimeUnit.SECONDS)&&indexed.get()!=null){Uri uri=indexed.get();ContentValues values=new ContentValues();values.put(MediaStore.Images.Media.DATE_TAKEN,j.getLong("modified"));values.put(MediaStore.Images.Media.DESCRIPTION,"StepX Companion 电脑扩容预览 · "+j.optString("name"));c.getContentResolver().update(uri,values,null,null);j.put("galleryUri",uri.toString());store.put(j);}
            else throw new IOException("相册正在索引预览，可稍后刷新相册");
        }catch(Exception e){try{j.put("galleryError",e.getMessage());store.put(j);}catch(Exception ignored){}}
    }
    static JSONObject resolve(Context c,ExpansionStore store,Uri uri)throws Exception {
        if(uri==null||!"content".equals(uri.getScheme()))throw new IOException("请从系统相册分享扩容缩略图");
        String name="";try(Cursor cursor=c.getContentResolver().query(uri,new String[]{OpenableColumns.DISPLAY_NAME},null,null,null)){if(cursor!=null&&cursor.moveToFirst())name=cursor.getString(0);}
        Matcher match=NAME.matcher(name==null?"":name);
        if(!match.matches())throw new IOException("这不是电脑扩容缩略图，无需取回。请选择 StepX Companion 相册中的图片。");
        JSONObject j=store.get(match.group(1));if(!j.optString("state").equals("archived")&&!j.optString("state").equals("restored"))throw new IOException("此文件没有可取回的电脑原件");
        ByteArrayOutputStream bytes=new ByteArrayOutputStream();try(InputStream in=c.getContentResolver().openInputStream(uri)){if(in==null)throw new IOException("无法读取这张缩略图");byte[] b=new byte[16384];int n;while((n=in.read(b))!=-1){if(bytes.size()+n>2*1024*1024)throw new IOException("此图片不是原始扩容缩略图");bytes.write(b,0,n);}}
        if(!hash(bytes.toByteArray()).equals(j.optString("gallerySha256")))throw new IOException("缩略图已被编辑，无法核对原件。请从原始扩容缩略图取回。");
        return j;
    }
    static void remove(Context c,JSONObject j)throws Exception {
        if(!j.has("gallerySha256"))return;File f=path(j.getString("id"));
        if(f.isFile()&&ExpansionStore.digest(f).equals(j.getString("gallerySha256"))){Files.delete(f.toPath());android.media.MediaScannerConnection.scanFile(c,new String[]{f.getPath()},null,null);}
    }
    private static String hash(byte[] b)throws Exception {StringBuilder s=new StringBuilder();for(byte n:MessageDigest.getInstance("SHA-256").digest(b))s.append(String.format(Locale.ROOT,"%02x",n&255));return s.toString();}
}

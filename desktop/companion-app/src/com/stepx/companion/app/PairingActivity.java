package com.stepx.companion.app;

import android.Manifest;
import android.app.*;
import android.content.*;
import android.content.pm.PackageManager;
import android.graphics.*;
import android.graphics.drawable.GradientDrawable;
import android.hardware.camera2.*;
import android.hardware.camera2.params.StreamConfigurationMap;
import android.media.ImageReader;
import android.net.*;
import android.os.*;
import android.provider.Settings;
import android.util.Size;
import android.view.*;
import android.widget.*;
import com.google.zxing.*;
import com.google.zxing.common.HybridBinarizer;
import com.google.zxing.qrcode.QRCodeReader;
import java.io.*;
import java.nio.ByteBuffer;
import java.nio.file.Files;
import java.nio.file.StandardCopyOption;
import java.nio.charset.StandardCharsets;
import java.util.*;
import java.util.concurrent.atomic.AtomicBoolean;
import org.json.JSONObject;

/** Local camera / photo QR decoder feeding Android's authenticated pairing handshake. */
public final class PairingActivity extends Activity implements TextureView.SurfaceTextureListener {
    private final Handler ui=new Handler(Looper.getMainLooper());
    private HandlerThread cameraThread; private Handler cameraHandler;
    private CameraDevice camera; private CameraCaptureSession capture; private ImageReader reader;
    private TextureView texture; private TextView status; private Button scan,enable,cancel;
    private PairingController controller;
    private boolean resumed,waitingNetwork,selectingImage,scannerRequested,galleryRequested,destroyed,openingCamera;
    private final AtomicBoolean decoding=new AtomicBoolean(false);
    private long lastDecode;
    private String diagnosticId="";
    private long diagnosticDeadline;
    private boolean diagnosticPending,diagnosticSubmitted;
    private int dp(float n){return Math.round(n*getResources().getDisplayMetrics().density);}
    private TextView label(LinearLayout p,String value,int size){TextView v=new TextView(this);v.setText(value);v.setTextSize(size);v.setTextColor(Color.rgb(30,30,36));v.setPadding(0,dp(10),0,dp(10));p.addView(v,new LinearLayout.LayoutParams(-1,-2));return v;}
    private Button button(LinearLayout p,String text,Runnable action){Button b=new Button(this);b.setText(text);b.setAllCaps(false);b.setMinHeight(dp(50));p.addView(b,new LinearLayout.LayoutParams(-1,-2));b.setOnClickListener(v->action.run());return b;}
    @Override public void onCreate(Bundle state){super.onCreate(state);
        LinearLayout root=new LinearLayout(this);root.setOrientation(1);root.setPadding(dp(22),dp(22),dp(22),dp(20));root.setBackgroundColor(Color.rgb(248,248,251));
        root.setOnApplyWindowInsetsListener((v,insets)->{Insets bars=insets.getInsets(WindowInsets.Type.systemBars());v.setPadding(dp(22)+bars.left,dp(14)+bars.top,dp(22)+bars.right,dp(16)+bars.bottom);return insets;});
        button(root,"‹ 返回 StepX伴侣",this::finish);
        label(root,"扫码连接电脑",26).setTypeface(null,Typeface.BOLD);
        label(root,"手机与电脑连接同一 Wi-Fi。在 desktop 打开『连接设备』，让二维码完整进入取景框。",15);
        texture=new TextureView(this);texture.setSurfaceTextureListener(this);root.addView(texture,new LinearLayout.LayoutParams(-1,dp(260)));
        status=label(root,"正在检查扫码能力…",15);
        scan=button(root,"扫描 desktop 二维码",this::requestScan);
        button(root,"从相册选择二维码",this::requestGallery);
        enable=button(root,"开启无线连接",this::confirmEnable);
        cancel=button(root,"取消本次配对",()->{waitingNetwork=false;scannerRequested=false;galleryRequested=false;closeCamera();controller.cancel();});
        button(root,"管理已配对电脑",()->{try{startActivity(new Intent("android.settings.WIRELESS_DEBUGGING_SETTINGS"));}catch(Exception e){startActivity(new Intent(Settings.ACTION_APPLICATION_DEVELOPMENT_SETTINGS));}});
        label(root,"二维码只在手机本地识别。无线连接使用 Android 安全配对；口令不会上传或保存在文件中。",12);
        ScrollView scroll=new ScrollView(this);scroll.addView(root);setContentView(scroll);root.requestApplyInsets();
        controller=new PairingController(this,(s,m)->{status.setText(m);cancel.setEnabled(controller!=null&&controller.isPairing());if(diagnosticSubmitted){diagnostic(s,m);if("paired".equals(s)||"error".equals(s)||"cancelled".equals(s))diagnosticSubmitted=false;}if("paired".equals(s)||"error".equals(s)){scannerRequested=false;closeCamera();}});
        updateReadiness();
        acceptDiagnostic(getIntent());
    }
    @Override protected void onNewIntent(Intent intent){super.onNewIntent(intent);setIntent(intent);acceptDiagnostic(intent);}
    private void acceptDiagnostic(Intent intent){
        // This Activity is DUMP-protected. The test can only read this one app-owned file,
        // cannot enable wireless/trust a network, and uses the same real QR decoder.
        if(intent==null||!intent.getBooleanExtra("test_mode",false)||!"incoming-qr.png".equals(intent.getStringExtra("test_qr_file")))return;
        String request=intent.getStringExtra("test_request_id");
        if(request==null||!request.matches("[0-9a-f]{16}"))return;
        ui.removeCallbacks(diagnosticWait);diagnosticSubmitted=false;diagnosticPending=true;diagnosticId=request;diagnosticDeadline=SystemClock.elapsedRealtime()+15000;
        waitingNetwork=scannerRequested=galleryRequested=false;closeCamera();
        if(controller.isPairing())controller.cancel();
        diagnostic("received","");ui.post(diagnosticWait);
    }
    private final Runnable diagnosticWait=new Runnable(){@Override public void run(){
        if(destroyed||!diagnosticPending)return;
        if(SystemClock.elapsedRealtime()>=diagnosticDeadline){diagnosticPending=false;String reason=!resumed?"扫码页未处于前台，请先解锁手机。":!controller.ready()?"安全扫码服务尚未就绪。":!controller.supported()?"手机未确认安全二维码配对能力。":"请先开启无线调试并完成系统 Wi-Fi 信任确认。";diagnostic("error",reason);controller.update("error",reason);return;}
        if(!resumed||!controller.ready()||!controller.supported()||!controller.wirelessEnabled()||controller.port()<=0){diagnostic("waiting_service","");controller.refresh();ui.postDelayed(this,250);return;}
        diagnosticPending=false;
        try{
            File directory=getExternalFilesDir(null);if(directory==null)throw new IOException("测试二维码目录不可用");
            File file=new File(directory,"incoming-qr.png");
            if(!Files.isRegularFile(file.toPath(),java.nio.file.LinkOption.NOFOLLOW_LINKS)||file.length()>8*1024*1024)throw new IOException("测试二维码文件无效");
            diagnostic("decoding","");BitmapFactory.Options bounds=new BitmapFactory.Options();bounds.inJustDecodeBounds=true;
            try(InputStream in=new FileInputStream(file)){BitmapFactory.decodeStream(in,null,bounds);}
            if(bounds.outWidth<=0||bounds.outHeight<=0||(long)bounds.outWidth*bounds.outHeight>4000000)throw new IOException("测试二维码图片尺寸无效");
            PairingCode code;try(InputStream in=new FileInputStream(file)){code=PairingCode.parse(decodeBitmap(BitmapFactory.decodeStream(in)));}
            diagnosticSubmitted=true;diagnostic("submitted","");controller.begin(code);
        }catch(Exception e){diagnostic("error",e.getMessage());showError(e);}
    }};
    private void diagnostic(String phase,String error){
        if(diagnosticId.isEmpty())return;
        try{JSONObject value=new JSONObject().put("protocol",1).put("requestId",diagnosticId).put("phase",phase).put("updatedAtMs",System.currentTimeMillis());
            if("error".equals(phase))value.put("error",error==null?"测试未完成":error.substring(0,Math.min(200,error.length())));
            File directory=getExternalFilesDir(null);if(directory==null)return;directory.mkdirs();File temporary=new File(directory,"pairing-test.json.tmp");
            Files.write(temporary.toPath(),value.toString().getBytes(StandardCharsets.UTF_8));Files.move(temporary.toPath(),new File(directory,"pairing-test.json").toPath(),StandardCopyOption.REPLACE_EXISTING,StandardCopyOption.ATOMIC_MOVE);
        }catch(Exception ignored){}
    }
    private boolean capable(){
        if(!PairingController.permitted(this)||!PairingController.settingsPermitted(this)){status.setText("请先在 desktop『小工具 → StepX伴侣』完成安装或修复权限，再返回扫码。");return false;}
        if(!controller.ready()||!controller.supported()){status.setText(controller.readinessMessage());controller.refresh();return false;}
        return true;
    }
    private boolean wifi(){ConnectivityManager m=getSystemService(ConnectivityManager.class);for(Network n:m.getAllNetworks()){NetworkCapabilities c=m.getNetworkCapabilities(n);if(c!=null&&c.hasTransport(NetworkCapabilities.TRANSPORT_WIFI))return true;}return false;}
    private void confirmEnable(){
        if(!capable())return;
        if(controller.wirelessEnabled()){
            new AlertDialog.Builder(this).setTitle("关闭无线连接？").setMessage("已连接的电脑将断开无线连接。USB 连接和已配对记录会保留。").setNegativeButton("保留",null).setPositiveButton("关闭",(d,w)->{try{waitingNetwork=false;scannerRequested=false;closeCamera();controller.disableWireless();updateReadiness();}catch(Exception e){showError(e);}}).show();return;
        }
        if(!wifi()){status.setText("请先让手机与电脑连接同一 Wi-Fi。");startActivity(new Intent(Settings.ACTION_WIFI_SETTINGS));return;}
        new AlertDialog.Builder(this).setTitle("开启与电脑的无线连接").setMessage("仅已安全配对的电脑可以连接此手机。若系统询问是否信任当前 Wi-Fi，请确认当前网络。你可以随时在此关闭。").setNegativeButton("暂不开启",null).setPositiveButton("开启",(d,w)->{try{controller.enableWireless();waitingNetwork=true;updateReadiness();}catch(Exception e){showError(e);}}).show();
    }
    private void requestGallery(){
        if(!capable())return;scannerRequested=false;galleryRequested=true;closeCamera();
        if(!controller.wirelessEnabled()){confirmEnable();return;}
        if(controller.port()<=0){waitingNetwork=true;status.setText("等待系统开启无线连接，请检查手机上的 Wi-Fi 确认弹窗。");return;}
        galleryRequested=false;selectingImage=true;Intent i=new Intent(Intent.ACTION_OPEN_DOCUMENT).setType("image/*").addCategory(Intent.CATEGORY_OPENABLE);startActivityForResult(i,20);
    }
    private void requestScan(){
        if(!capable())return;
        scannerRequested=true;galleryRequested=false;
        if(!controller.wirelessEnabled()){confirmEnable();return;}
        if(controller.port()<=0){waitingNetwork=true;status.setText("等待系统开启无线连接，请检查手机上的 Wi-Fi 确认弹窗。");return;}
        if(checkSelfPermission(Manifest.permission.CAMERA)!=PackageManager.PERMISSION_GRANTED){requestPermissions(new String[]{Manifest.permission.CAMERA},10);return;}
        if(controller.isPairing())controller.cancel();
        decoding.set(false);status.setText("将 desktop 二维码放入取景框；识别后会请你确认连接。");openCamera();
    }
    private void updateReadiness(){if(destroyed)return;
        boolean granted=PairingController.permitted(this)&&PairingController.settingsPermitted(this);
        enable.setText(controller.wirelessEnabled()?"关闭无线连接":"开启无线连接");
        cancel.setEnabled(controller!=null&&controller.isPairing());
        if(!granted)status.setText("请先在 desktop 小工具中安装或修复 StepX伴侣，完成安全配对权限适配。");
        else if(waitingNetwork&&controller.port()>0){waitingNetwork=false;status.setText("无线连接已就绪，可以扫码连接电脑。");if(scannerRequested)requestScan();else if(galleryRequested)requestGallery();}
        else if(controller.ready()&&!scannerRequested&&!controller.isPairing()&&!waitingNetwork&&"正在检查扫码能力…".contentEquals(status.getText()))status.setText("请选择扫描二维码，或从相册选择二维码。");
        controller.publish();
    }
    private final Runnable tick=new Runnable(){public void run(){updateReadiness();ui.postDelayed(this,1000);}};
    private void candidate(String raw){
        ui.post(()->{if(destroyed||!resumed)return;closeCamera();try{PairingCode code=PairingCode.parse(raw);new AlertDialog.Builder(this).setTitle("连接这台 StepX 电脑？").setMessage("请确认刚才扫描的是你电脑上 desktop『连接设备』页面的二维码。配对后，这台电脑可以管理手机文件与任务。").setNegativeButton("取消",(d,w)->{decoding.set(false);scannerRequested=false;status.setText("已取消，未向电脑授权。");}).setPositiveButton("配对并连接",(d,w)->{try{scannerRequested=false;controller.begin(code);}catch(Exception e){showError(e);}}).setOnCancelListener(d->{decoding.set(false);scannerRequested=false;}).show();}catch(Exception e){decoding.set(false);scannerRequested=false;showError(e);}});
    }
    private void openCamera(){
        if(!resumed||destroyed||!texture.isAvailable()||camera!=null||openingCamera||checkSelfPermission(Manifest.permission.CAMERA)!=PackageManager.PERMISSION_GRANTED)return;
        try{
            if(cameraThread==null){cameraThread=new HandlerThread("StepXLocalQr");cameraThread.start();cameraHandler=new Handler(cameraThread.getLooper());}
            CameraManager manager=getSystemService(CameraManager.class);String chosen=null;Size size=null;
            for(String id:manager.getCameraIdList()){CameraCharacteristics c=manager.getCameraCharacteristics(id);if(Integer.valueOf(CameraCharacteristics.LENS_FACING_BACK).equals(c.get(CameraCharacteristics.LENS_FACING))){chosen=id;StreamConfigurationMap map=c.get(CameraCharacteristics.SCALER_STREAM_CONFIGURATION_MAP);if(map!=null){Size[] sizes=map.getOutputSizes(ImageFormat.YUV_420_888);if(sizes!=null)size=Arrays.stream(sizes).filter(s->s.getWidth()<=1280&&s.getHeight()<=960).min(Comparator.comparingInt(s->Math.abs(s.getWidth()*s.getHeight()-640*480))).orElse(sizes[0]);}break;}}
            if(chosen==null||size==null)throw new IOException("未找到可用的后置相机");
            final Size target=size; reader=ImageReader.newInstance(size.getWidth(),size.getHeight(),ImageFormat.YUV_420_888,2);reader.setOnImageAvailableListener(r->{try(android.media.Image im=r.acquireLatestImage()){if(im==null||decoding.get()||SystemClock.elapsedRealtime()-lastDecode<180)return;lastDecode=SystemClock.elapsedRealtime();android.media.Image.Plane p=im.getPlanes()[0];ByteBuffer b=p.getBuffer();int w=im.getWidth(),h=im.getHeight(),stride=p.getRowStride(),pixel=p.getPixelStride();byte[] data=new byte[w*h];int origin=b.position();for(int y=0;y<h;y++)for(int x=0;x<w;x++)data[y*w+x]=b.get(origin+y*stride+x*pixel);try{String raw=new QRCodeReader().decode(new BinaryBitmap(new HybridBinarizer(new PlanarYUVLuminanceSource(data,w,h,0,0,w,h,false)))).getText();if(decoding.compareAndSet(false,true))candidate(raw);}catch(ReaderException ignored){}}catch(Exception ignored){}},cameraHandler);
            openingCamera=true;manager.openCamera(chosen,new CameraDevice.StateCallback(){@Override public void onOpened(CameraDevice device){openingCamera=false;camera=device;try{if(!resumed||destroyed||!texture.isAvailable()){closeCamera();return;}SurfaceTexture st=texture.getSurfaceTexture();st.setDefaultBufferSize(target.getWidth(),target.getHeight());Surface preview=new Surface(st);final ImageReader current=reader;if(current==null){device.close();return;}device.createCaptureSession(Arrays.asList(preview,current.getSurface()),new CameraCaptureSession.StateCallback(){@Override public void onConfigured(CameraCaptureSession session){capture=session;try{CaptureRequest.Builder req=device.createCaptureRequest(CameraDevice.TEMPLATE_PREVIEW);req.addTarget(preview);req.addTarget(current.getSurface());req.set(CaptureRequest.CONTROL_AF_MODE,CaptureRequest.CONTROL_AF_MODE_CONTINUOUS_PICTURE);session.setRepeatingRequest(req.build(),null,cameraHandler);}catch(Exception e){ui.post(()->showError(e));}}@Override public void onConfigureFailed(CameraCaptureSession session){ui.post(()->status.setText("相机启动失败，可以从相册选择二维码。"));}},cameraHandler);}catch(Exception e){ui.post(()->showError(e));}}@Override public void onDisconnected(CameraDevice c){openingCamera=false;c.close();camera=null;}@Override public void onError(CameraDevice c,int code){openingCamera=false;c.close();camera=null;ui.post(()->status.setText("无法使用相机，请检查权限或从相册选择二维码。"));}},cameraHandler);
        }catch(Exception e){openingCamera=false;showError(e);}
    }
    private void closeCamera(){if(capture!=null){capture.close();capture=null;}if(camera!=null){camera.close();camera=null;}if(reader!=null){reader.close();reader=null;}}
    static String decodeBitmap(Bitmap bitmap)throws Exception{if(bitmap==null)throw new IOException("无法读取二维码图片");try{int w=bitmap.getWidth(),h=bitmap.getHeight();if(w*h>4000000)throw new IOException("二维码图片过大，请裁剪后重试");int[] pixels=new int[w*h];bitmap.getPixels(pixels,0,w,0,0,w,h);return new QRCodeReader().decode(new BinaryBitmap(new HybridBinarizer(new RGBLuminanceSource(w,h,pixels)))).getText();}finally{bitmap.recycle();}}
    @Override protected void onActivityResult(int request,int result,Intent data){super.onActivityResult(request,result,data);selectingImage=false;if(request==20&&result==RESULT_OK&&data!=null&&data.getData()!=null){try{BitmapFactory.Options options=new BitmapFactory.Options();options.inJustDecodeBounds=true;try(InputStream in=getContentResolver().openInputStream(data.getData())){BitmapFactory.decodeStream(in,null,options);}options.inSampleSize=1;while(options.outWidth/options.inSampleSize>1600||options.outHeight/options.inSampleSize>1600)options.inSampleSize*=2;options.inJustDecodeBounds=false;try(InputStream in=getContentResolver().openInputStream(data.getData())){candidate(decodeBitmap(BitmapFactory.decodeStream(in,null,options)));}}catch(Exception e){showError(e);}}}
    @Override public void onRequestPermissionsResult(int request,String[] permissions,int[] results){super.onRequestPermissionsResult(request,permissions,results);if(request==10&&results.length>0&&results[0]==PackageManager.PERMISSION_GRANTED)requestScan();else status.setText("相机权限未开启，可以从相册选择二维码。");}
    private void showError(Exception e){Throwable t=e;while(t.getCause()!=null)t=t.getCause();String m=t instanceof SecurityException?"安全配对权限尚未适配，请在 desktop 小工具中修复 StepX伴侣。":t instanceof ReaderException?"图片中没有识别到二维码，请选择清晰完整的 StepX 二维码。":t.getMessage();controller.update("error",m==null?"操作未完成，请重试。":m);}
    @Override protected void onResume(){super.onResume();resumed=true;ui.post(tick);}
    @Override protected void onPause(){resumed=false;ui.removeCallbacks(tick);closeCamera();super.onPause();}
    @Override protected void onStop(){super.onStop();if(diagnosticPending){diagnosticPending=false;ui.removeCallbacks(diagnosticWait);diagnostic("error","扫码页已离开前台，测试已停止。");}if(!selectingImage&&controller!=null&&controller.isPairing())controller.cancel();}
    @Override protected void onDestroy(){destroyed=true;ui.removeCallbacksAndMessages(null);closeCamera();if(controller!=null)controller.close();if(cameraThread!=null)cameraThread.quitSafely();super.onDestroy();}
    @Override public void onSurfaceTextureAvailable(SurfaceTexture s,int w,int h){if(scannerRequested)openCamera();}
    @Override public void onSurfaceTextureSizeChanged(SurfaceTexture s,int w,int h){}
    @Override public boolean onSurfaceTextureDestroyed(SurfaceTexture s){closeCamera();return true;}
    @Override public void onSurfaceTextureUpdated(SurfaceTexture s){}
}

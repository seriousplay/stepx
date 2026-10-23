package com.stepx.companion.app;

import com.google.zxing.*;
import com.google.zxing.common.*;
import com.google.zxing.qrcode.*;

/** Real QR encode/decode round trip plus strict protocol validation. No phone mutations. */
public final class PairingCodeTest {
    private static final String QR="WIFI:T:ADB;S:studio-stepx-0123456789abcdef;P:aB12cD34eF56gH78iJ90;;";
    private static void check(boolean b,String reason){if(!b)throw new AssertionError(reason);}
    private static void reject(String value){try{PairingCode.parse(value);throw new AssertionError("Unsafe / unsupported QR accepted");}catch(IllegalArgumentException expected){}}
    public static void main(String[] args)throws Exception {
        PairingCode p=PairingCode.parse(QR);check(p.service.equals("studio-stepx-0123456789abcdef"),"Service mismatch");check(p.password.length()==20,"Password length mismatch");
        BitMatrix matrix=new QRCodeWriter().encode(QR,BarcodeFormat.QR_CODE,420,420);int[] pixels=new int[420*420];for(int y=0;y<420;y++)for(int x=0;x<420;x++)pixels[y*420+x]=matrix.get(x,y)?0xff000000:0xffffffff;
        String decoded=new QRCodeReader().decode(new BinaryBitmap(new HybridBinarizer(new RGBLuminanceSource(420,420,pixels)))).getText();check(PairingCode.parse(decoded).service.equals(p.service),"QR image round trip failed");
        reject("https://example.com");reject(QR.replace("T:ADB","T:WPA"));reject(QR.replace("studio-stepx-","untrusted-"));reject(QR.replace(";;",";H:true;;"));reject(QR.replace("T:ADB;","T:ADB;T:ADB;"));reject(QR.replace("iJ90","iJ9;"));reject(QR.substring(0,QR.length()-1));reject(null);reject("x".repeat(513));
        check(TransferStore.safeName("../../secret.txt").equals("_.._secret.txt"),"Path fragments must not survive as separators");
        check(TransferStore.safeName("  ").equals("文件"),"Empty name fallback");
        check(TransferStore.safeName(TransferStore.safeName("notes.json.tmp")).equals(TransferStore.safeName("notes.json.tmp")),"Filename sanitizer must be idempotent");
        check(TransferStore.safeName("manifest.json").equals("file-manifest.json"),"Payload must not overwrite manifest");
        check(TransferStore.safeName("图".repeat(120)+".png").getBytes(java.nio.charset.StandardCharsets.UTF_8).length<=220,"Unicode filename exceeds filesystem limit");
        check(TransferStore.uuid("678a4534-d8b6-489f-851c-31e0b97454dd"),"UUID rejected");check(!TransferStore.uuid("../../escape"),"Invalid ID accepted");
        System.out.println("PairingCodeTest: real QR round trip and malformed / non-StepX QR rejection passed");
    }
}

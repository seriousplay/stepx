package com.stepx.companion.app;

import java.io.ByteArrayOutputStream;
import java.nio.charset.StandardCharsets;
import java.util.Base64;
import java.util.UUID;

/** Minimal audited StreamMsg; no task id, body, account token or tool operation. */
final class CloudWakeMessage {
    static String create(){return encode("stepx-companion-"+UUID.randomUUID());}
    static String encode(String trace){
        if(!trace.matches("stepx-companion-[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}"))throw new IllegalArgumentException("Invalid local trace");
        byte[] entry=concat(field(1,"Zyql_TraceId".getBytes(StandardCharsets.UTF_8)),field(2,trace.getBytes(StandardCharsets.UTF_8)));
        byte[] head=concat(field(5,"CloudWake".getBytes(StandardCharsets.UTF_8)),field(6,entry));
        return Base64.getEncoder().encodeToString(field(1,head));
    }
    private static byte[] field(int number,byte[] value){ByteArrayOutputStream out=new ByteArrayOutputStream();varint(out,(number<<3)|2);varint(out,value.length);out.write(value,0,value.length);return out.toByteArray();}
    private static void varint(ByteArrayOutputStream out,int value){while(value>=128){out.write((value&127)|128);value>>>=7;}out.write(value);}
    private static byte[] concat(byte[] a,byte[] b){byte[] r=new byte[a.length+b.length];System.arraycopy(a,0,r,0,a.length);System.arraycopy(b,0,r,a.length,b.length);return r;}
}

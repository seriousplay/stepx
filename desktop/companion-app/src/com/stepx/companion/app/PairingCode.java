package com.stepx.companion.app;

import java.util.HashMap;
import java.util.Map;

/** Strict parser for the one-time QR payload emitted by StepX desktop. */
final class PairingCode {
    final String service, password;
    private PairingCode(String service,String password){this.service=service;this.password=password;}
    static PairingCode parse(String text) {
        if(text==null||text.length()>512||!text.startsWith("WIFI:")||!text.endsWith(";;"))throw new IllegalArgumentException("请扫描 desktop『连接设备』中的 StepX 二维码");
        Map<String,String> fields=new HashMap<>();
        for(String item:text.substring(5).split(";",-1)){
            if(item.isEmpty())continue;
            int split=item.indexOf(':');
            if(split!=1||fields.put(item.substring(0,split),item.substring(split+1))!=null)throw new IllegalArgumentException("连接二维码格式无效，请在 desktop 刷新");
        }
        String service=fields.get("S"),password=fields.get("P");
        if(fields.size()!=3||!"ADB".equals(fields.get("T"))||service==null||!service.matches("studio-stepx-[0-9a-f]{16}")||password==null||!password.matches("[A-Za-z0-9]{20}"))throw new IllegalArgumentException("这不是有效的 StepX 连接二维码");
        return new PairingCode(service,password);
    }
}

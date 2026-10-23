package com.stepx.companion.app;

import org.json.JSONArray;
import org.json.JSONObject;
import java.io.IOException;
import java.util.Calendar;

final class DisplayConfig {
    String title,jobId,kind,preset;
    int fps,brightness,durationSecs;
    int[][] frames;
    static DisplayConfig parse(String raw)throws Exception{
        if(raw.length()>262144)throw new IOException("背屏配置超过256KiB");
        JSONObject j=new JSONObject(raw);DisplayConfig c=new DisplayConfig();
        c.title=j.optString("title","背屏图案");c.jobId=j.optString("requestId",j.optString("jobId",""));
        if(c.title.length()>100||c.jobId.length()>100)throw new IOException("配置名称过长");
        c.fps=j.optInt("fps",6);c.brightness=j.optInt("brightness",5);c.durationSecs=j.optInt("durationSecs",0);
        if(c.fps<1||c.fps>12||c.brightness<1||c.brightness>8||c.durationSecs<0||c.durationSecs>3600)throw new IOException("帧率、亮度或时长超出范围");
        c.kind=j.optString("kind","frames");c.preset=j.optString("key","");
        if(c.kind.equals("preset")){
            if(!c.preset.equals("clock")&&!c.preset.equals("battery")&&!c.preset.equals("countdown"))throw new IOException("不支持此动态预设");
            if(c.preset.equals("countdown")&&c.durationSecs<1)throw new IOException("倒计时必须指定时长");
        }else if(c.kind.equals("frames")){
            JSONArray fs=j.getJSONArray("frames");if(fs.length()<1||fs.length()>120)throw new IOException("需1至120帧");c.frames=new int[fs.length()][391];
            for(int i=0;i<fs.length();i++){JSONArray a=fs.getJSONArray(i);if(a.length()!=391)throw new IOException("每帧必须为391槽");for(int p=0;p<391;p++){Object v=a.get(p);if(!(v instanceof Number)||((Number)v).doubleValue()!=((Number)v).intValue())throw new IOException("像素必须为整数");int b=((Number)v).intValue();if(b<0||b>255||GameCore.masked(p%17,p/17)&&b!=0)throw new IOException("像素越界或无效灯珠非零");c.frames[i][p]=b;}}
        }else throw new IOException("未知背屏配置类型");
        return c;
    }
    int[] frame(long elapsedMs,int battery){
        if(frames!=null)return frames[(int)((elapsedMs*fps/1000)%frames.length)];
        int[]f=new int[391];int b=255;
        if(preset.equals("clock")){Calendar t=Calendar.getInstance();int h=t.get(Calendar.HOUR_OF_DAY),m=t.get(Calendar.MINUTE);GameCore.digit(f,h/10,5,4,b);GameCore.digit(f,h%10,9,4,b);GameCore.digit(f,m/10,5,12,b);GameCore.digit(f,m%10,9,12,b);GameCore.put(f,8,10,b);}
        if(preset.equals("countdown")){int n=Math.max(0,durationSecs-(int)(elapsedMs/1000));if(n>99){int m=n/60,s=n%60;GameCore.digit(f,m/10,5,4,b);GameCore.digit(f,m%10,9,4,b);GameCore.digit(f,s/10,5,12,b);GameCore.digit(f,s%10,9,12,b);GameCore.put(f,8,10,b);}else{GameCore.digit(f,n/10,5,9,b);GameCore.digit(f,n%10,9,9,b);}}
        if(preset.equals("battery")){for(int x=3;x<14;x++){GameCore.put(f,x,7,b);GameCore.put(f,x,16,b);}for(int y=7;y<=16;y++){GameCore.put(f,3,y,b);GameCore.put(f,13,y,b);}for(int x=6;x<=10;x++)GameCore.put(f,x,6,b);for(int y=15;y>=15-7*Math.max(0,battery)/100;y--)for(int x=5;x<12;x++)GameCore.put(f,x,y,b);}
        return f;
    }
}

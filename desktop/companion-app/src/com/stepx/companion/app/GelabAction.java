package com.stepx.companion.app;
import java.util.*;

/** Android port of GELab-Zero's tab-separated action parser; no executor/shell. */
final class GelabAction {
    static Map<String,String> parse(String text){
        if(text.length()>24000)throw new IllegalArgumentException("模型响应过长");
        int think=text.toLowerCase(Locale.ROOT).lastIndexOf("</think>");if(think>=0)text=text.substring(think+8);
        Map<String,String> result=new LinkedHashMap<>();
        for(String field:text.trim().split("[\\t\\n]+")){int i=field.indexOf(':');if(i>0){String k=field.substring(0,i).trim();if(result.containsKey(k))throw new IllegalArgumentException("模型返回重复字段");result.put(k,field.substring(i+1).trim());}}
        String action=result.get("action");
        if(!Arrays.asList("CLICK","LONGPRESS","SLIDE","TYPE","AWAKE","BACK","HOME","WAIT","COMPLETE","ABORT","INFO","CALL_USER").contains(action))throw new IllegalArgumentException("模型动作不受支持");
        if(action.equals("CLICK")||action.equals("LONGPRESS"))point(result.get("point"));
        if(action.equals("SLIDE")){point(result.get("point1"));point(result.get("point2"));}
        if(action.equals("TYPE")&&(!result.containsKey("value")||result.get("value").length()>4000))throw new IllegalArgumentException("输入内容无效");
        return result;
    }
    static int[] point(String text){
        if(text==null)throw new IllegalArgumentException("缺少坐标");
        String[] parts=text.trim().split("[, ]+");if(parts.length!=2)throw new IllegalArgumentException("坐标格式无效");
        int x=Integer.parseInt(parts[0]),y=Integer.parseInt(parts[1]);if(x<0||y<0||x>1000||y>1000)throw new IllegalArgumentException("坐标越界");return new int[]{x,y};
    }
}

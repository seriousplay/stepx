package com.stepx.companion.app;
public class GelabActionTest {
    public static void main(String[] args){
        for(String action:new String[]{"action:exec\tvalue:id","action:CLICK\tpoint:1001,0","action:CLICK\tpoint:1,NaN","action:CLICK\taction:TYPE\tpoint:1,2"}){
            boolean rejected=false;try{GelabAction.parse(action);}catch(Exception e){rejected=true;}if(!rejected)throw new AssertionError("Accepted unsupported action");
        }
        if(!GelabAction.parse("verify:已打开\tnote:搜索框\taction:TYPE\tvalue:电池健康").get("value").equals("电池健康"))throw new AssertionError("Chinese input lost");
        if(GelabAction.point("500,800")[1]!=800)throw new AssertionError("Bad coordinates");
        if(!GelabAction.parse("action:CALL_USER\tvalue:需要登录").get("action").equals("CALL_USER"))throw new AssertionError("Handoff lost");
        System.out.println("GelabActionTest: passed");
    }
}

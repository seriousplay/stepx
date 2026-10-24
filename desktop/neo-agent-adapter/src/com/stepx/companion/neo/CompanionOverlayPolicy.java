package com.stepx.companion.neo;

/** Presentation only. Never decides whether an action is allowed or completed. */
final class CompanionOverlayPolicy {
    static String action(String name) {
        if(name==null)return "观察";
        switch(name){
            case "tap":case "click":return "点击";
            case "swipe":case "scroll":return "滑动";
            case "type":case "document_write":return "输入";
            case "back":return "返回";
            case "home":return "主页";
            case "wait":return "等待";
            case "finish":return "核验";
            default:return "观察";
        }
    }
    static String phase(boolean paused,String status){
        if(paused)return "暂停";
        if(status!=null&&(status.contains("规划")||status.contains("目标 ")))return "规划";
        if(status!=null&&(status.contains("核验")||status.contains("复核")))return "核验";
        return "观察";
    }
}

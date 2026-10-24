package com.stepx.companion.app;

public final class NeoPolicyTest {
    private static void check(boolean value){if(!value)throw new AssertionError();}
    public static void main(String[] args){
        check(NeoPolicy.systemVersionPreset(" 打开设置，查看 Android 版本 "));
        check(!NeoPolicy.systemVersionPreset("打开设置，查看 Android 版本，然后关闭网络"));
        check(!NeoPolicy.systemVersionPreset(null));
        check(NeoPolicy.phaseLabel("reported").equals("结果待核实"));
        check(NeoPolicy.phaseLabel("interrupted").equals("执行已中断"));
        check(NeoPolicy.phaseLabel(null).equals("状态未知"));
        for(String phase:new String[]{"queued","planning","running","paused","paused_planning","cancelling"})check(NeoPolicy.active(phase));
        for(String phase:new String[]{"idle","completed","reported","failed","stopped",null})check(!NeoPolicy.active(phase));
        for(String phase:new String[]{"idle","completed","reported","failed","stopped"}){
            check(!NeoPolicy.canControl("pause",phase));check(!NeoPolicy.canControl("resume",phase));check(!NeoPolicy.canControl("stop",phase));check(NeoPolicy.canStart(phase,true));
        }
        check(!NeoPolicy.canStart("running",true));check(!NeoPolicy.canStart("idle",false));
        check(NeoPolicy.canControl("pause","planning"));check(!NeoPolicy.canControl("resume","running"));
        check(NeoPolicy.canControl("resume","paused"));check(NeoPolicy.canControl("stop","paused"));
        check(NeoPolicy.verified("completed",true,true,true));
        check(!NeoPolicy.verified("reported",true,true,true));
        check(!NeoPolicy.verified("completed",false,true,true));
        check(!NeoPolicy.verified("completed",true,false,true));
        check(!NeoPolicy.verified("completed",true,true,false));
        check(NeoPolicy.statusText("completed",true,true,"待核实").equals("已完成 · 屏幕结果已核验"));
        check(NeoPolicy.statusText("completed",true,false,"待核实").contains("等待屏幕核验"));
        check(NeoPolicy.statusText("reported",true,true,"待核实").contains("等待屏幕核验"));
        check(NeoPolicy.statusText("failed",true,true,"网络错误").equals("任务失败 · 网络错误"));
        check(NeoPolicy.statusText("needs_input",true,true,"请登录").equals("需要你处理 · 请登录"));
        check(NeoPolicy.owns(10001,10001,"task-a","task-a","task-a"));
        check(!NeoPolicy.owns(10002,10001,"task-a","task-a","task-a"));
        check(!NeoPolicy.owns(10001,10001,"task-a","task-b","task-a"));
        check(!NeoPolicy.owns(10001,10001,"task-a","task-a","replacement"));
        check(!NeoPolicy.owns(10001,10001,"","",""));
        check(!NeoPolicy.owns(10001,10001,null,null,null));
        System.out.println("Neo routing, ownership and verified completion boundaries passed");
    }
}

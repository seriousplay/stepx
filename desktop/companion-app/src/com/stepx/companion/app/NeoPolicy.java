package com.stepx.companion.app;

/** Pure policy shared by the Neo route and its offline boundary tests. */
final class NeoPolicy {
    // Exact read-only preset: extra instructions must never inherit its completion criteria.
    static boolean systemVersionPreset(String goal) {
        return goal!=null&&"打开设置，查看 Android 版本".equals(goal.trim());
    }
    static String phaseLabel(String phase) {
        if(phase==null)return "状态未知";
        switch(phase){
            case "idle":return "准备就绪";
            case "planning":case "queued":return "正在规划";
            case "running":return "正在执行";
            case "paused":case "paused_planning":return "已暂停";
            case "cancelling":return "正在停止";
            case "completed":return "已核验完成";
            case "reported":return "结果待核实";
            case "needs_input":return "需要你接管";
            case "failed":return "执行失败";
            case "stopped":case "cancelled":return "已停止";
            case "interrupted":return "执行已中断";
            case "pending":return "等待执行";
            default:return "状态未知";
        }
    }
    static boolean active(String phase) {
        return "queued".equals(phase) || "planning".equals(phase) || "running".equals(phase)
            || "paused".equals(phase) || "paused_planning".equals(phase) || "cancelling".equals(phase);
    }
    static boolean canPause(String phase) { return "queued".equals(phase)||"planning".equals(phase)||"running".equals(phase); }
    static boolean canResume(String phase) { return "paused".equals(phase)||"paused_planning".equals(phase); }
    static boolean canStart(String phase,boolean ready) { return ready&&!active(phase); }
    static boolean canControl(String op,String phase) {
        return "pause".equals(op)?canPause(phase):"resume".equals(op)?canResume(phase):"stop".equals(op)&&active(phase);
    }
    static boolean verified(String state, boolean completed, boolean goalVerified, boolean terminal) {
        return "completed".equals(state) && completed && goalVerified && terminal;
    }
    static String statusText(String phase,boolean verified,boolean stopped,String progress) {
        if("completed".equals(phase)&&verified&&stopped)return "已完成 · 屏幕结果已核验";
        if("completed".equals(phase)||"reported".equals(phase))return "模型报告完成，等待屏幕核验";
        String detail=progress==null?"":progress;
        if("failed".equals(phase))return "任务失败 · "+detail;
        if("needs_input".equals(phase))return "需要你处理 · "+detail;
        if("cancelled".equals(phase)||"stopped".equals(phase))return "任务已停止 · "+detail;
        return detail;
    }
    static boolean owns(int uid, int ownerUid, String stored, String requested, String live) {
        return uid == ownerUid && stored != null && !stored.isEmpty()
            && stored.equals(requested) && stored.equals(live);
    }
}

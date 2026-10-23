package com.stepx.companion.app;

/** Phone-owned execution lease. Every action is tied to a task and observation epoch. */
final class GuiTaskState {
    static final long LEASE_MS = 45000;
    volatile String id = "", goal = "", phase = "idle", detail = "";
    volatile long heartbeat, epoch, actions;
    volatile boolean userPaused, userStopped;
    boolean active() { return phase.equals("running") || phase.equals("paused"); }
    void requireCanStart(long now) {
        expire(now);
        if (active()) throw new IllegalStateException("已有任务，请先停止或继续当前任务");
        if (userStopped || userPaused) throw new IllegalStateException("用户已停止，请本人点击继续或新建手机任务");
    }
    // Only the phone's explicit Start button may authorize a new task after a human stop.
    void allowNewTaskFromUser(long now) {
        expire(now);
        if (active()) throw new IllegalStateException("已有任务，请先停止或继续当前任务");
        userStopped = false; userPaused = false;
    }
    void start(String task, String target, long now) {
        requireCanStart(now);
        if (!task.matches("[a-f0-9]{32}") || target.trim().isEmpty() || target.length() > 12000)
            throw new IllegalArgumentException("任务标识或目标无效");
        id = task; goal = target; phase = "running"; detail = "准备观察屏幕";
        heartbeat = now; epoch++; actions = 0; userPaused = false;
    }
    void require(String task, long now) {
        expire(now);
        if (!id.equals(task) || !active()) throw new IllegalStateException("任务已停止、过期或被替换");
    }
    void renew(String task, long now) { require(task, now); heartbeat = now; }
    void pause(String task, long now) { require(task, now); phase = "paused"; epoch++; detail = "已暂停，等待继续"; }
    void resume(String task, long now) { if(userPaused)throw new IllegalStateException("用户已暂停，请本人点击继续"); require(task, now); phase = "running"; epoch++; detail = "已继续，重新观察屏幕"; }
    void userPause(String task, long now) { pause(task,now); userPaused=true; }
    void userContinue(String task, long now) {
        expire(now);
        if(userStopped||(!active()&&userPaused)){userStopped=false; userPaused=false; detail="已允许新任务，请重新发送目标"; return;}
        require(task,now); userPaused=false; resume(task,now);
    }
    void userStop(String task) { stop(task,"用户已停止，需本人点击继续才允许新任务"); userStopped=true; userPaused=false; }
    void stop(String task, String reason) {
        if (!id.equals(task)) throw new IllegalStateException("任务标识不匹配");
        phase = "stopped"; detail = reason; epoch++;
    }
    void action(String task, long observedEpoch, long now) {
        require(task, now);
        if (!phase.equals("running") || epoch != observedEpoch)
            throw new IllegalStateException("任务已暂停或画面已过期，请重新观察");
        actions++; epoch++;
    }
    void expire(long now) {
        if (active() && (now < heartbeat || now - heartbeat > LEASE_MS)) {
            phase = "stopped"; detail = "任务连接超时，已停止操作"; epoch++;
        }
    }
}

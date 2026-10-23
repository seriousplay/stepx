package com.stepx.companion.app;
public final class GuiTaskStateTest {
    static void check(boolean value) { if (!value) throw new AssertionError(); }
    static void rejected(Runnable action) { try { action.run(); } catch (IllegalStateException expected) { return; } throw new AssertionError("Unsafe action accepted"); }
    public static void main(String[] args) {
        GuiTaskState s = new GuiTaskState(); String id = "0123456789abcdef0123456789abcdef";
        s.start(id, "查看手机设置", 100); long before = s.epoch;
        s.pause(id, 101); rejected(() -> s.action(id, before, 102));
        s.resume(id, 103); rejected(() -> s.action(id, before, 104));
        s.action(id, s.epoch, 105); check(s.actions == 1);
        rejected(() -> s.action("wrong-task", s.epoch, 106));
        s.renew(id, 200); s.expire(45201); check(!s.active());
        rejected(() -> s.action(id, s.epoch, 45202));
        s.start(id, "新任务", 46000); s.stop(id, "用户停止");
        rejected(() -> s.resume(id, 46001));
        check(s.actions == 0);
        s.start(id,"人工控制测试",47000);s.userPause(id,47001);
        rejected(()->s.resume(id,47002));rejected(()->s.action(id,s.epoch,47002));
        s.userContinue(id,47003);s.action(id,s.epoch,47004);check(s.actions==1);
        s.userStop(id);rejected(()->s.start(id,"模型重试不能撤销用户停止",47005));
        s.stop(id,"模型自己stop也不能解除人工停止");rejected(()->s.start(id,"仍应拒绝",47006));
        s.userContinue(id,47007);s.start(id,"用户允许后可开始",47008);check(s.active());
        s.userPause(id,47009);s.expire(100000);
        rejected(()->s.start(id,"人工暂停过期也不能自行重启",100001));
        s.userContinue(id,100002);s.start(id,"本人允许",100003);
        s.userPause(id,100004);rejected(()->s.allowNewTaskFromUser(100005));
        s.userStop(id);rejected(()->s.requireCanStart(100006));
        s.allowNewTaskFromUser(100007);s.start(id,"手机新建任务即明确允许",100008);
        check(s.active()&&!s.userStopped&&!s.userPaused);
        System.out.println("GuiTaskStateTest passed");
    }
}

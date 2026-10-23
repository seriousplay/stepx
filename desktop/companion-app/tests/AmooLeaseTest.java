package com.stepx.companion.app;

import java.nio.charset.StandardCharsets;
import java.util.Base64;

public final class AmooLeaseTest {
    private static void check(boolean value,String message){if(!value)throw new AssertionError(message);}
    public static void main(String[] args){
        AmooLeaseState s=new AmooLeaseState();s.acquire("work",0);s.acquire("snapshot",1);s.release("snapshot");
        check(s.has("work",2),"snapshot release cannot revoke Work");
        check(s.wakeDelay(false,0)==15000,"cold asleep process must settle");
        check(s.wakeDelay(false,14999)==1,"no early wake");check(s.wakeDelay(false,15000)==0,"settled wake");s.woke(15000);
        check(s.wakeDelay(false,16000)==59000,"native wake rate limit");s.renew("work",25000);
        check(s.wakeDelay(true,30000)==-1,"screen-on never sends wake");check(s.wakeDelay(false,100000)==15000,"new off transition settles again");
        check(!s.active(115000),"90 seconds without renewal expires");
        boolean rejected=false;try{s.renew("work",115000);}catch(IllegalStateException e){rejected=true;}check(rejected,"expired renewal cannot recreate lease");
        s.acquire("work",120000);s.clear();check(!s.active(120001),"disable clears every lease");
        s.screen(false,120000);s.woke(120000);s.clear();check(s.nativeWindowRemaining(120001)==179999,"stop does not claim native window ended");
        s.screen(true,120002);check(s.nativeWindowRemaining(120002)==0,"real screen-on invalidates native override");
        s.acquire("new-work",120003);check(s.wakeDelay(false,120003)==59997,"next off epoch retains rate limit, not old window");
        check(s.lastWake()==-1,"old screen epoch must not claim a current wake");
        AmooLeaseState restarted=new AmooLeaseState();restarted.restoreWake(120000,120004);restarted.acquire("work",120004);
        check(restarted.wakeDelay(false,120004)==59996,"service restart retains rate limit");
        check(restarted.nativeWindowRemaining(120004)==0&&restarted.lastWake()==-1,"already-asleep service cannot infer old epoch");
        check(restarted.wakeDelay(false,180000)==0,"fresh request permitted after limits");restarted.woke(180000);
        check(restarted.nativeWindowRemaining(180001)==179999,"fresh off-epoch request has bounded window");
        AmooLeaseState grace=new AmooLeaseState();grace.acquire("short-read",0);grace.screen(false,0);grace.woke(15000);grace.release("short-read");
        check(!grace.active(20000)&&grace.idleGraceRemaining(true,false,20000)==175000,"idle observer keeps only the existing native window");
        check(grace.idleGraceRemaining(true,false,25000)==170000&&grace.lastWake()==15000,"status cannot extend idle grace or send another wake");
        grace.acquire("next-read",25000);check(grace.lastWake()==15000&&grace.nativeWindowRemaining(25000)==170000,"next snapshot retains witnessed screen epoch");
        check(grace.idleGraceRemaining(true,false,25000)==0,"active task is not idle grace");grace.release("next-read");
        check(grace.idleGraceRemaining(false,false,26000)==0,"user disable stops idle observer immediately");
        check(grace.idleGraceRemaining(true,false,195000)==0,"idle grace naturally ends with original native window");
        grace.screen(true,30000);grace.screen(false,30001);
        check(grace.idleGraceRemaining(true,false,30002)==0&&grace.lastWake()==-1,"rapid real ON/OFF must invalidate idle epoch");
        String wire=CloudWakeMessage.encode("stepx-companion-00000000-0000-0000-0000-000000000001");
        byte[] bytes=Base64.getDecoder().decode(wire);String raw=new String(bytes,StandardCharsets.ISO_8859_1);
        check(bytes[0]==10&&bytes[2]==42,"Stream head1 and command5 tags");
        check(raw.contains("CloudWake")&&raw.contains("Zyql_TraceId")&&raw.contains("stepx-companion-"),"minimal native schema");
        check(!raw.contains("task")&&!raw.contains("CallTool")&&!raw.contains("account"),"no fake task or credentials");
        rejected=false;try{CloudWakeMessage.encode("real-push-id");}catch(IllegalArgumentException e){rejected=true;}check(rejected,"only honest local trace");
        System.out.println("AmooLeaseTest: passed");
    }
}

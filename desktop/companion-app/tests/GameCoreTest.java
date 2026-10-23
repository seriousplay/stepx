package com.stepx.companion.app;

public final class GameCoreTest {
    static void check(boolean value,String message){if(!value)throw new AssertionError(message);}
    static boolean tap(GameCore.TapDetector t,long at){t.sample(at-10,0,0,0,true);return t.sample(at,9,1,0,true);}
    static double angleForColumn(int x,int width){double value=((x-1.)/(15-width)*2-1)*22;return value==0?0:Math.copySign(Math.abs(value)+3,value);}
    static void mask(int[]frame){for(int p=0;p<391;p++)check(!GameCore.masked(p%17,p/17)||frame[p]==0,"masked pixel");}
    public static void main(String[]args){
        GameCore.TapDetector t=new GameCore.TapDetector();check(!tap(t,1000),"single");check(!tap(t,1250),"double");check(tap(t,1500),"triple");check(!tap(t,1600),"cooldown");
        t.reset();tap(t,1000);tap(t,1600);check(tap(t,2200),"deliberate slower triple");
        t.reset();tap(t,1000);tap(t,1250);check(!t.sample(1500,9,1,4,true),"rotation rejection");
        t.reset();tap(t,1000);tap(t,1700);check(!tap(t,1900),"wide gaps rejected");
        t.reset();check(!t.sample(1000,10,0,0,false),"clear front-up posture rejected");
        t.reset();for(int i=0;i<30;i++){t.sample(1000+i*400,0,0,0,true);check(!t.sample(1100+i*400,6,8,1,true),"walking-like lateral impulses rejected");}check(t.recent.size()<=10,"bounded diagnostics");
        GameCore g=new GameCore(true);boolean[] widths=new boolean[6];for(int i=0;i<200;i++){g.update(.05,0);widths[g.width]=true;mask(g.frame(8));}
        check(g.phase.equals("won"),"synthetic full game");for(int w=1;w<=5;w++)check(widths[w],"width "+w);check(g.score==150,"synthetic score");
        GameCore controlled=new GameCore(false);for(int i=0;i<10000&&controlled.completedLevels<35;i++){controlled.update(.05,angleForColumn(controlled.target,controlled.width));mask(controlled.frame(8));}
        check(controlled.completedLevels>=35&&controlled.round>=7&&controlled.score>=1050,"real tilt input cycles beyond five minutes with cumulative scoring");check(!controlled.ended(),"real game never automatically ends at five levels");
        for(int level=1;level<=5;level++){
            GameCore speed=new GameCore(false);speed.phase="falling";speed.level=level;speed.width=6-level;speed.target=3;speed.x=3;speed.filtered=angleForColumn(3,speed.width);
            for(int i=0;i<(13-level)*20-2;i++)speed.update(.05,speed.filtered);
            check(speed.phase.equals("falling"),"full drop takes 12 through 8 seconds");for(int i=0;i<4;i++)speed.update(.05,speed.filtered);check(speed.phase.equals("hit"),"drop lands within timing tolerance");
            GameCore landing=new GameCore(false);landing.level=level;landing.width=6-level;landing.phase="falling";landing.target=3;landing.x=4;landing.y=19.99;landing.filtered=angleForColumn(landing.x,landing.width);landing.update(.02,landing.filtered);check(landing.phase.equals(level<=2?"hit":"miss"),"one-column tolerance only in levels 1 and 2");if(level<=2)check(landing.x==landing.target,"successful tolerance snaps into slot");
        }
        GameCore missed=new GameCore(false);missed.level=4;missed.width=2;missed.score=120;missed.phase="falling";missed.y=19.99;missed.target=1;missed.filtered=80;missed.update(.02,80);check(missed.phase.equals("miss"),"miss recorded");for(int i=0;i<21;i++)missed.update(.05,0);check(missed.level==1&&missed.width==5&&missed.score==120&&!missed.ended(),"miss resets widths but keeps cumulative score");
        check(GameCore.backViewTilt(3,0,-9)>0&&GameCore.backViewTilt(-3,0,-9)<0,"back-view right and left sign");check(Math.abs(GameCore.backViewTilt(3,9,0)-GameCore.backViewTilt(3,0,-9))<.001,"upright and flat tilt gain consistent");
        GameCore.FaceExit face=new GameCore.FaceExit();check(!face.sample(0,9.8,new double[]{0,0,1})&&!face.armed,"desktop start face-up does not immediately exit");
        check(!face.sample(100,-9.8,new double[]{0,0,-1})&&face.armed,"back-facing pose arms exit");check(!face.sample(200,0,new double[]{1,0,0}),"upright is not itself an exit");check(!face.sample(300,9.8,new double[]{0,0,1}),"brief flip does not exit");check(!face.sample(700,9.8,new double[]{0,0,1}),"front-up debounce");check(face.sample(900,9.8,new double[]{0,0,1}),"stable front-up exit");
        face.reset();face.sample(0,0,new double[]{1,0,0});check(!face.sample(100,0,new double[]{-1,0,0}),"upright yaw flip starts debounce");check(face.sample(700,0,new double[]{-1,0,0}),"upright 180 degree yaw flip exits using rotation vector");
        face.reset();face.sample(0,0,new double[]{1,0,0});for(int i=1;i<=500;i++){double a=i*Math.PI/500;check(!face.sample(i*40,0,new double[]{Math.cos(a),0,Math.sin(a)}),"slow body turn follows reference without exiting");}
        face.reset();face.sample(0,0,new double[]{1,0,0});boolean fastFlip=false;for(int i=1;i<=70;i++){double a=Math.min(1,i/50.)*Math.PI;fastFlip|=face.sample(i*40,0,new double[]{Math.cos(a),0,Math.sin(a)});}check(fastFlip,"two-second deliberate flip survives slow reference tracking");
        face.reset();face.sample(0,-9.8,null);check(!face.sample(100,6,null)&&face.sample(700,6,null),"gravity fallback with missing rotation sensor");
        for(int w=1;w<=5;w++)for(int target=1;target<=16-w;target++){GameCore edge=new GameCore(false);edge.phase="falling";edge.width=w;edge.target=target;int[]ef=edge.frame(8);int slots=0;for(int xx=0;xx<17;xx++)if(ef[21*17+xx]==255)slots++;check(slots==w,"all edge target slots have physical LEDs");}
        GameCore hud=new GameCore(false);hud.score=1234;int[]frame=hud.frame(8);int lit=0;for(int yy=1;yy<=5;yy++)for(int xx=1;xx<=15;xx++)if(frame[yy*17+xx]>0)lit++;check(lit>20,"cumulative score drawn on actual LED frame");hud.score=15000;mask(hud.frame(8));hud.score=4000000;mask(hud.frame(8));
        int masked=0;for(int i=0;i<391;i++)if(GameCore.masked(i%17,i/17))masked++;check(masked==12,"12 corner slots");
        System.out.println("GameCoreTest passed: tap timing/rejection/diagnostics, 12-to-8-second falls, 7 continuous real-input rounds, score HUD, miss recovery, back-view direction, gravity and upright-rotation flip exits, 379 LED mask");
    }
}

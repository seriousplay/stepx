package com.stepx.companion.app;

import java.util.ArrayDeque;

/** Android-independent rules; physical sensors and renderer remain outside the model. */
public final class GameCore {
    public int level=1, width=5, x=6, target=3, round=1, completedLevels=0, misses=0;
    public long score=0;
    public double y=7, filtered=0;
    public String phase="countdown";
    private double phaseTime=0;
    private final boolean test;
    public GameCore(boolean test) { this.test=test; }
    public double dropSeconds(){return 13-Math.min(5,level);}
    public void update(double seconds, double tilt) {
        seconds=Math.max(0,Math.min(.2,seconds));
        phaseTime+=seconds;
        if (phase.equals("countdown")) { if(phaseTime>=(test?.6:3)) {phase="falling";phaseTime=0;} return; }
        if (phase.equals("hit") || phase.equals("miss")) {
            if(phaseTime<(test?.15:1.0))return;
            if(test&&completedLevels>=5){phase="won";return;}
            if(phase.equals("miss"))level=1;
            else if(level>5){round++;level=1;}
            width=6-level; target=1+((level*5+(round-1)*3)%(16-width)); x=(17-width)/2;y=7;phase="falling";phaseTime=0;
        }
        if(!phase.equals("falling"))return;
        filtered += Math.min(1,seconds*12)*(tilt-filtered);
        double value=Math.abs(filtered)<3?0:Math.copySign(Math.abs(filtered)-3,filtered);
        double pos=1+(Math.max(-1,Math.min(1,value/22))+1)*.5*(15-width);
        // Half-cell hysteresis prevents a held device from oscillating at a column boundary.
        if(pos>x+.65 || pos<x-.65) x=(int)Math.max(1,Math.min(16-width,Math.round(pos)));
        if(test)x=target;
        y+=seconds*(test?27:13/dropSeconds());
        if(y>=20){y=20;phaseTime=0;if(Math.abs(x-target)<=(level<=2?1:0)){x=target;score+=width*10;completedLevels++;level++;phase="hit";}else{misses++;phase="miss";}}
    }
    public int countdown(){return Math.max(1,3-(int)phaseTime);}
    /** Only the explicitly synthetic test terminates automatically. Real play is continuous. */
    public boolean ended(){return test&&phase.equals("won");}
    public int[] frame(int brightness) {
        int[] f=new int[391]; int hi=Math.max(16,Math.min(255,brightness*255/8)),lo=Math.max(8,hi/5);
        score(f,score,1,hi);
        if(phase.equals("countdown")){digit(f,countdown(),7,10,hi);return f;}
        if(phase.equals("won")){for(int xx=3;xx<14;xx++)put(f,xx,16-(int)(Math.abs(xx-8)*.5),hi);put(f,4,11,hi);put(f,12,11,hi);return f;}
        for(int i=0;i<5;i++)put(f,4+i*2,6,i<Math.min(level,5)?lo:0);
        for(int xx=1;xx<=15;xx++) if(xx<target||xx>=target+width){put(f,xx,21,lo);put(f,xx,22,lo);}
        // Row 21 retains all columns 1..15; row 22 loses x=1 and x=15.
        for(int xx=target;xx<target+width;xx++)put(f,xx,21,hi);
        int row=(int)y;for(int xx=x;xx<x+width;xx++){put(f,xx,row,hi);put(f,xx,row+1,hi);}
        if(phase.equals("hit")){
            // A short acknowledgement gives the player time to read the updated total.
            put(f,6,12,hi);put(f,7,13,hi);put(f,8,14,hi);put(f,9,13,hi);put(f,10,12,hi);put(f,11,11,hi);
            if(((int)(phaseTime*10)%2)==0)for(int xx=target;xx<target+width;xx++)put(f,xx,20,hi);
        }else if(phase.equals("miss"))for(int xx=6;xx<=10;xx++){put(f,xx,xx+5,lo);put(f,16-xx,xx+5,lo);}
        return f;
    }
    /** Android +X is front-view right; its gravity sign maps to back-view LED columns. */
    public static double backViewTilt(double gx,double gy,double gz){return Math.toDegrees(Math.atan2(gx,Math.max(1,Math.hypot(gy,gz))));}
    public static boolean masked(int x,int y){return (y==0||y==22)&&(x<2||x>14)||(y==1||y==21)&&(x==0||x==16);}
    public static void put(int[]f,int x,int y,int v){if(x>=0&&x<17&&y>=0&&y<23&&!masked(x,y))f[y*17+x]=v;}
    private static final String[] DIGITS={"111101101101111","010110010010111","111001111100111","111001111001111","101101111001001","111100111001111","111100111101111","111001001001001","111101111101111","111101111001111"};
    public static void digit(int[]f,int n,int x,int y,int v){glyph(f,DIGITS[Math.floorMod(n,10)],x,y,v);}
    private static void glyph(int[]f,String d,int x,int y,int v){for(int i=0;i<15;i++)if(d.charAt(i)=='1')put(f,x+i%3,y+i/3,v);}
    private static void score(int[]f,long score,int y,int v){
        if(score<=9999){for(int i=3;i>=0;i--){digit(f,(int)(score%10),1+i*4,y,v);score/=10;}return;}
        boolean million=score>=1000000;long shown=score/(million?1000000:1000);shown=Math.min(999,shown);
        for(int i=2;i>=0;i--){digit(f,(int)(shown%10),1+i*4,y,v);shown/=10;}
        glyph(f,million?"101111111101101":"101101110101101",13,y,v);
    }

    /** Detect turning the front toward the user, including upright yaw turns. */
    public static final class FaceExit {
        public boolean armed=false;
        public double facingDot=1;
        private double[] reference;
        private long since=-1,lastSample=-1;
        public void reset(){armed=false;reference=null;since=lastSample=-1;facingDot=1;}
        public boolean sample(long ms,double gravityZ,double[] normal){
            long elapsed=lastSample<0?0:Math.max(0,Math.min(100,ms-lastSample));lastSample=ms;
            if(!armed){if(gravityZ<2){armed=true;if(normal!=null)reference=normal.clone();}return false;}
            if(reference==null&&normal!=null&&gravityZ<2)reference=normal.clone();
            if(reference!=null&&normal!=null)facingDot=reference[0]*normal[0]+reference[1]*normal[1]+reference[2]*normal[2];
            boolean flipped=gravityZ>5.5||(reference!=null&&normal!=null&&facingDot<-.5);
            if(!flipped){
                since=-1;
                // Follow slow heading changes and rotation-vector drift. Freeze the
                // reference beyond 45 degrees so a deliberate fast flip still exits.
                if(reference!=null&&normal!=null&&facingDot>.707){
                    double alpha=elapsed/1500.0,norm=0;
                    for(int i=0;i<3;i++){reference[i]+=(normal[i]-reference[i])*alpha;norm+=reference[i]*reference[i];}
                    norm=Math.sqrt(norm);if(norm>0)for(int i=0;i<3;i++)reference[i]/=norm;
                }
                return false;
            }
            if(since<0)since=ms;
            return ms-since>=600;
        }
    }

    public static final class TapDetector {
        private long previous=0,first=0,cooldown=0;
        private boolean high=false;
        public int count=0;
        public String lastReason="waiting";
        public final ArrayDeque<Impulse> recent=new ArrayDeque<>();
        public static final class Impulse {
            public long at;public double normal,lateral,gyro;public String reason;
            Impulse(long at,double normal,double lateral,double gyro,String reason){this.at=at;this.normal=normal;this.lateral=lateral;this.gyro=gyro;this.reason=reason;}
        }
        public void reset(){previous=first=cooldown=0;high=false;count=0;lastReason="waiting";}
        private void record(long ms,double normal,double lateral,double gyro,String reason){
            if(normal<3)return;
            lastReason=reason;Impulse last=recent.peekLast();
            if(last!=null&&ms-last.at<120){last.normal=Math.max(last.normal,normal);last.lateral=Math.max(last.lateral,lateral);last.gyro=Math.max(last.gyro,gyro);if(!last.reason.equals("accepted")&&!last.reason.equals("triple")||reason.equals("triple"))last.reason=reason;}
            else{if(recent.size()==10)recent.removeFirst();recent.addLast(new Impulse(ms,normal,lateral,gyro,reason));}
        }
        public boolean sample(long ms,double normalAccel,double lateralAccel,double gyro,boolean rearVisible) {
            double magnitude=Math.abs(normalAccel),lateral=Math.abs(lateralAccel);
            if(ms<cooldown){record(ms,magnitude,lateral,gyro,"cooldown");return false;}
            if(!rearVisible||gyro>3.5){count=0;high=false;record(ms,magnitude,lateral,gyro,!rearVisible?"front_facing_up":"rotation");return false;}
            if(count>0&&ms-previous>650){count=0;lastReason="gap_too_long";}
            if(magnitude<2)high=false;
            if(magnitude<4.5){record(ms,magnitude,lateral,gyro,"weak");return false;}
            if(magnitude<lateral*1.15){record(ms,magnitude,lateral,gyro,"lateral_motion");return false;}
            if(high){record(ms,magnitude,lateral,gyro,"same_impulse");return false;}
            high=true;
            if(previous!=0&&ms-previous<120){record(ms,magnitude,lateral,gyro,"debounce");return false;}
            if(count==0)first=ms;
            previous=ms;count++;
            boolean ok=count==3&&ms-first<=1600;
            record(ms,magnitude,lateral,gyro,ok?"triple":"accepted");
            if(count==3){count=0;cooldown=ms+1500;return ok;}
            return false;
        }
    }
}

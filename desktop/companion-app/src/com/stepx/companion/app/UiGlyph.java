package com.stepx.companion.app;
import android.graphics.*;
import android.graphics.drawable.Drawable;
import android.widget.TextView;
/** One outline family across navigation, tasks, files, connection and settings. */
final class UiGlyph extends Drawable {
    static final int PLAY=0,PAUSE=1,STOP=2,BACK=3,HOME=4,TASK=5,FOLDER=6,SETTINGS=7,SCAN=8,COMPUTER=9,SHIELD=10,MODEL=11,SCREEN=12,STUDIO=13,CHEVRON=14,UPLOAD=15,CHECK=16,CLOCK=17,LINK=18,IMAGE=19,VIDEO=20,FILE=21;
    private final Paint p=new Paint(Paint.ANTI_ALIAS_FLAG);private final int kind;
    UiGlyph(int kind,int color){this.kind=kind;p.setColor(color);p.setStyle(Paint.Style.STROKE);p.setStrokeWidth(1.75f);p.setStrokeCap(Paint.Cap.ROUND);p.setStrokeJoin(Paint.Join.ROUND);}
    static void attach(TextView view,int kind,int color,int size){UiGlyph icon=new UiGlyph(kind,color);icon.setBounds(0,0,size,size);view.setCompoundDrawablesRelative(icon,null,null,null);view.setCompoundDrawablePadding(Math.max(4,size/3));}
    private void path(Canvas c,float... xy){Path path=new Path();path.moveTo(xy[0],xy[1]);for(int i=2;i<xy.length;i+=2)path.lineTo(xy[i],xy[i+1]);c.drawPath(path,p);}
    @Override public void draw(Canvas c){c.save();c.translate(getBounds().left,getBounds().top);c.scale(getBounds().width()/24f,getBounds().height()/24f);
        switch(kind){
        case PLAY:path(c,8,4,20,12,8,20,8,4);break;
        case PAUSE:c.drawRoundRect(5,4,9,20,1,1,p);c.drawRoundRect(15,4,19,20,1,1,p);break;
        case STOP:c.drawRoundRect(5,5,19,19,2,2,p);break;
        case HOME:path(c,3,11,12,3,21,11,21,21,15,21,15,14,9,14,9,21,3,21,3,11);break;
        case TASK:c.drawRoundRect(4,3,20,21,3,3,p);path(c,8,8,16,8);path(c,8,12,16,12);path(c,8,16,12,16);break;
        case FOLDER:path(c,3,5,10,5,12,8,21,8,21,20,3,20,3,5);break;
        case IMAGE:c.drawRoundRect(3,3,21,21,2,2,p);c.drawCircle(8,8,1.5f,p);path(c,3,17,9,11,14,16,17,13,21,17);break;
        case VIDEO:c.drawRoundRect(3,3,21,21,2,2,p);path(c,9,7,17,12,9,17,9,7);break;
        case FILE:path(c,6,2,14,2,20,8,20,22,6,22,6,2);path(c,14,2,14,8,20,8);path(c,9,13,16,13);path(c,9,17,16,17);break;
        case SETTINGS:c.drawCircle(12,12,7,p);c.drawCircle(12,12,2.5f,p);for(int i=0;i<8;i++){c.save();c.rotate(i*45,12,12);path(c,12,2,12,5);c.restore();}break;
        case SCAN:for(int i=0;i<4;i++){c.save();c.rotate(i*90,12,12);path(c,3,8,3,3,8,3);c.restore();}path(c,6,12,18,12);break;
        case COMPUTER:c.drawRoundRect(2,4,22,17,2,2,p);path(c,12,17,12,21);path(c,8,21,16,21);break;
        case SHIELD:path(c,12,2,21,6,20,15,17,19,12,22,7,19,4,15,3,6,12,2);path(c,8,12,11,15,16,9);break;
        case MODEL:c.drawRoundRect(5,5,19,19,3,3,p);for(int i=8;i<=16;i+=4){path(c,i,2,i,5);path(c,i,19,i,22);path(c,2,i,5,i);path(c,19,i,22,i);}c.drawRect(9,9,15,15,p);break;
        case SCREEN:c.drawRoundRect(5,2,19,22,3,3,p);for(int x=9;x<=15;x+=3)for(int y=7;y<=16;y+=3)c.drawPoint(x,y,p);break;
        case STUDIO:c.drawRoundRect(3,3,21,21,3,3,p);path(c,3,9,21,9);path(c,8,13,5,16,8,19);path(c,16,13,19,16,16,19);break;
        case CHEVRON:path(c,9,5,16,12,9,19);break;
        case UPLOAD:path(c,12,16,12,3,7,8);path(c,12,3,17,8);path(c,3,15,3,21,21,21,21,15);break;
        case CHECK:path(c,4,12,10,18,20,6);break;
        case CLOCK:c.drawCircle(12,12,9,p);path(c,12,6,12,12,16,14);break;
        case LINK:c.drawRoundRect(2,6,14,14,4,4,p);c.drawRoundRect(10,10,22,18,4,4,p);break;
        default:path(c,20,12,4,12,10,6);path(c,4,12,10,18);}
        c.restore();}
    @Override public void setAlpha(int a){p.setAlpha(a);}@Override public void setColorFilter(ColorFilter f){p.setColorFilter(f);}@Override public int getOpacity(){return PixelFormat.TRANSLUCENT;}
}

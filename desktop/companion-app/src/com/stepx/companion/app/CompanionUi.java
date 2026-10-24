package com.stepx.companion.app;

import android.app.Activity;
import android.content.res.ColorStateList;
import android.graphics.*;
import android.graphics.drawable.*;
import android.view.*;
import android.widget.*;

/** Shared phone design tokens and accessible native controls. */
final class CompanionUi {
    // Keep in sync with desktop/src/ui/theme.rs; mobile uses larger touch targets.
    static final int INK=0xff18181b,MUTED=0xff71717a,LINE=0xffe5e5ea,BG=0xfffafafb,SURFACE=0xfff8f8fa,GREEN=0xff10b981,AMBER=0xff946200,RED=0xffb53a36;
    static final Typeface MEDIUM=Typeface.create("sans-serif-medium",Typeface.NORMAL);
    final Activity activity;
    CompanionUi(Activity activity){this.activity=activity;}
    int dp(float value){return Math.round(value*activity.getResources().getDisplayMetrics().density);}
    GradientDrawable shape(int fill,int border,int radius){GradientDrawable d=new GradientDrawable();d.setColor(fill);d.setCornerRadius(dp(radius));if(border!=0)d.setStroke(dp(1),border);return d;}
    android.graphics.drawable.Drawable touch(int fill,int border,int radius){return new RippleDrawable(ColorStateList.valueOf(0x16000000),shape(fill,border,radius),shape(Color.WHITE,0,radius));}
    LinearLayout column(){LinearLayout l=new LinearLayout(activity);l.setOrientation(1);return l;}
    TextView text(LinearLayout parent,String value,int size,int color,int gap){TextView t=new TextView(activity);t.setText(value);t.setTextSize(size);t.setTextColor(color);t.setLineSpacing(dp(3),1);LinearLayout.LayoutParams lp=new LinearLayout.LayoutParams(-1,-2);lp.topMargin=dp(gap);parent.addView(t,lp);return t;}
    TextView title(LinearLayout parent,String label,int size,int gap){TextView t=text(parent,label,size,INK,gap);t.setTypeface(MEDIUM);return t;}
    LinearLayout card(LinearLayout parent,int gap){LinearLayout l=column();l.setPadding(dp(16),dp(16),dp(16),dp(16));l.setBackground(shape(Color.WHITE,LINE,12));LinearLayout.LayoutParams lp=new LinearLayout.LayoutParams(-1,-2);lp.topMargin=dp(gap);parent.addView(l,lp);return l;}
    void styleButton(Button b,boolean primary){b.setAllCaps(false);b.setTextSize(14);b.setTypeface(MEDIUM);b.setTextColor(primary?Color.WHITE:INK);b.setStateListAnimator(null);b.setElevation(0);b.setBackgroundTintList(null);b.setBackground(touch(primary?INK:Color.WHITE,primary?0:LINE,8));b.setPadding(dp(12),dp(10),dp(12),dp(10));b.setMinHeight(dp(48));b.setMinimumWidth(0);b.setMinWidth(0);}
    Button button(LinearLayout parent,String label,boolean primary,Runnable action){Button b=new Button(activity);b.setText(label);styleButton(b,primary);LinearLayout.LayoutParams lp=new LinearLayout.LayoutParams(-1,-2);lp.topMargin=dp(12);parent.addView(b,lp);b.setOnClickListener(v->action.run());return b;}
    LinearLayout row(LinearLayout parent,int icon,String title,String detail,Runnable action){LinearLayout row=new LinearLayout(activity);row.setGravity(Gravity.CENTER_VERTICAL);row.setPadding(dp(4),dp(12),dp(4),dp(12));row.setMinimumHeight(dp(64));row.setBackground(touch(Color.TRANSPARENT,0,12));ImageView image=new ImageView(activity);image.setImageDrawable(new UiGlyph(icon,INK));row.addView(image,new LinearLayout.LayoutParams(dp(22),dp(22)));LinearLayout labels=column();LinearLayout.LayoutParams lp=new LinearLayout.LayoutParams(0,-2,1);lp.leftMargin=dp(14);row.addView(labels,lp);TextView name=title(labels,title,15,0);name.setMaxLines(2);name.setEllipsize(android.text.TextUtils.TruncateAt.END);if(detail!=null&&!detail.isEmpty())text(labels,detail,12,MUTED,3);if(action!=null){ImageView arrow=new ImageView(activity);arrow.setImageDrawable(new UiGlyph(UiGlyph.CHEVRON,MUTED));row.addView(arrow,new LinearLayout.LayoutParams(dp(18),dp(18)));row.setOnClickListener(v->action.run());row.setFocusable(true);}parent.addView(row,new LinearLayout.LayoutParams(-1,-2));return row;}
    void back(LinearLayout parent,String title,Runnable action){
        LinearLayout bar=new LinearLayout(activity);bar.setGravity(Gravity.CENTER_VERTICAL);bar.setMinimumHeight(dp(48));
        ImageButton b=new ImageButton(activity);b.setImageDrawable(new UiGlyph(UiGlyph.BACK,INK));b.setContentDescription("返回");b.setPadding(dp(12),dp(12),dp(12),dp(12));b.setBackground(touch(Color.TRANSPARENT,0,8));bar.addView(b,new LinearLayout.LayoutParams(dp(48),dp(48)));b.setOnClickListener(v->action.run());
        TextView label=new TextView(activity);label.setText(title);label.setTextSize(17);label.setTypeface(MEDIUM);label.setTextColor(INK);bar.addView(label);parent.addView(bar,new LinearLayout.LayoutParams(-1,-2));
    }
    void bars(View root){root.setOnApplyWindowInsetsListener((v,i)->{Insets b=i.getInsets(WindowInsets.Type.systemBars()|WindowInsets.Type.ime());v.setPadding(b.left,b.top,b.right,b.bottom);return i;});root.requestApplyInsets();activity.getWindow().getInsetsController().setSystemBarsAppearance(WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS|WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS,WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS|WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS);}
}

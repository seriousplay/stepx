package com.stepx.companion.app;
import android.animation.ValueAnimator;
import android.content.Context;
import android.view.View;
import android.view.animation.DecelerateInterpolator;
import android.widget.ProgressBar;
/** Motion is brief, cancellable, and respects both app and Android preferences. */
final class CompanionMotion {
 static boolean enabled(Context c){return ValueAnimator.areAnimatorsEnabled()&&!c.getSharedPreferences("phone_ui",0).getBoolean("reduced_motion",false);}
 static void enter(View v){v.animate().cancel();v.setTranslationY(0);v.setAlpha(1);if(!enabled(v.getContext()))return;v.setAlpha(.8f);v.setTranslationY(4*v.getResources().getDisplayMetrics().density);v.animate().alpha(1).translationY(0).setDuration(160).setInterpolator(new DecelerateInterpolator()).start();}
 static void stop(View v){if(v==null)return;v.animate().cancel();v.setAlpha(1);v.setTranslationY(0);}
 static ProgressBar progress(Context c){ProgressBar p=new ProgressBar(c,null,android.R.attr.progressBarStyleHorizontal);p.setIndeterminate(enabled(c));p.setMax(100);p.setProgress(100);p.setIndeterminateTintList(android.content.res.ColorStateList.valueOf(CompanionUi.MUTED));p.setProgressTintList(android.content.res.ColorStateList.valueOf(CompanionUi.LINE));p.setContentDescription("正在读取数据");p.setVisibility(View.INVISIBLE);return p;}
}

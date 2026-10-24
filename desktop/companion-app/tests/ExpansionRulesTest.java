package com.stepx.companion.app;
public final class ExpansionRulesTest {
    private static void check(boolean b){if(!b)throw new AssertionError();}
    public static void main(String[] args){
        check(ExpansionRules.kind("DCIM/20261024.jpg").equals("photos"));
        check(ExpansionRules.kind("DCIM/Camera/VID.mp4").equals("videos"));
        check(ExpansionRules.kind("DCIM/20261024.mp4").equals("videos"));
        check(ExpansionRules.kind("Download/report.pdf").equals("documents"));
        for(String s:new String[]{"DCIM/Screenshots/test.png","Pictures/test.jpg","DCIM/.cache/a.jpg","DCIM/../a.jpg","/DCIM/a.jpg","DCIM/a\\b.jpg"})check(ExpansionRules.kind(s).isEmpty());
        check(ExpansionRules.readyAt(1000,0)==121000);
        check(ExpansionRules.readyAt(1000,7)==604801000L);
        check(ExpansionRules.readyAt(1000,30)==2592001000L);
        System.out.println("Expansion camera roots, exclusions and retention rules passed");
    }
}

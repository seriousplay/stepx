package com.stepx.companion.neo;

public final class CompanionCompletionRulesTest {
    public static void main(String[] args) {
        if (!CompanionCompletionRules.androidVersion(true,true,true,true,true,false,false)) throw new AssertionError("About page");
        if (!CompanionCompletionRules.androidVersion(true,true,true,false,false,true,true)) throw new AssertionError("Version details");
        if (CompanionCompletionRules.androidVersion(false,true,true,true,true,true,true)) throw new AssertionError("Wrong/covered frame");
        if (CompanionCompletionRules.androidVersion(true,true,false,true,true,true,true)) throw new AssertionError("Missing actual version");
        if (CompanionCompletionRules.androidVersion(true,true,true,false,false,false,false)) throw new AssertionError("Search result is not a version page");
        if (CompanionCompletionRules.androidVersion(true,true,true,true,false,true,false)) throw new AssertionError("Mixed partial anchors");
        System.out.println("Version screen proof: overview/details, exact value, coverage and partial-anchor rejection passed");
    }
}

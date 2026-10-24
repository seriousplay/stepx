package com.stepx.companion.neo;

/** Accept the two real Settings version layouts, with a visible exact OS version. */
final class CompanionCompletionRules {
    static boolean androidVersion(boolean frame, boolean versionLabel, boolean exactVersion,
                                  boolean about, boolean system, boolean patch, boolean baseband) {
        return frame && versionLabel && exactVersion && ((about && system) || (patch && baseband));
    }
}

package com.stepx.companion.app;

/** Retry temporary model transport failures, never authentication/permission refusals. */
final class GuiRetryPolicy {
    static boolean http(int code) { return code == 408 || code == 429 || code >= 500 && code <= 599; }
}

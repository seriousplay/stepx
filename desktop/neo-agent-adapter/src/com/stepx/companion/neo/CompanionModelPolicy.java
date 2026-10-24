package com.stepx.companion.neo;

import java.net.URI;

/** Compatibility limited to the documented official Step 3.7 OpenAI endpoint. */
final class CompanionModelPolicy {
    static boolean step37(String endpoint, String model, String protocol) {
        if (!"OpenAI".equals(protocol) || !"step-3.7-flash".equals(model)) return false;
        try {
            URI uri = new URI(endpoint);
            return "https".equals(uri.getScheme()) && uri.getUserInfo() == null && uri.getQuery() == null && uri.getFragment() == null
                && (uri.getPort() == -1 || uri.getPort() == 443)
                && ("api.stepfun.com".equals(uri.getHost()) || "api.stepfun.ai".equals(uri.getHost()))
                && uri.getPath().matches("/(?:step_plan/)?v1(?:/chat/completions)?/?");
        } catch (Exception invalid) { return false; }
    }
    static int outputTokens(String endpoint, String model, String protocol, int requested) {
        int bounded = Math.max(512, Math.min(8192, requested));
        return step37(endpoint, model, protocol) ? Math.max(4096, bounded) : bounded;
    }
    static String effort(boolean visualAction) { return visualAction ? "low" : "medium"; }
    static boolean truncated(String reason) { return "length".equals(reason) || "max_tokens".equals(reason) || "MAX_TOKENS".equals(reason); }
}

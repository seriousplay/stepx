package com.stepx.companion.app;

/** Keep the legacy per-action controller from competing with the selected Agent. */
final class McpRoutePolicy {
    static boolean visible(String tool, boolean neoSelected) {
        return !neoSelected || !"stepx_gui_control".equals(tool);
    }
    static boolean legacyOperation(String operation, boolean neoSelected) {
        return !neoSelected || "status".equals(operation) || "stop".equals(operation);
    }
}

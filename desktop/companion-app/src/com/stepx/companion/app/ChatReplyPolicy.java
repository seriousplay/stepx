package com.stepx.companion.app;

/** Conservative recipient matching. Never accept prefixes or truncated conversation names. */
public final class ChatReplyPolicy {
    private ChatReplyPolicy() {}
    public static boolean sameRecipient(String expected, String actual) {
        if (expected == null || actual == null || expected.trim().isEmpty()) return false;
        String a=expected.trim(), b=actual.trim();
        if(a.equals(b)) return true;
        if(!a.matches("[+0-9 ()-]+") || !b.matches("[+0-9 ()-]+")) return false;
        a=a.replaceAll("[ ()-]", "");b=b.replaceAll("[ ()-]", "");
        // Android's Chinese SMS app displays +86 mobile senders in domestic form.
        if(a.matches("\\+861[0-9]{10}")) a=a.substring(3);
        if(b.matches("\\+861[0-9]{10}")) b=b.substring(3);
        return a.equals(b);
    }
    public static boolean verifiedAfterSend(int before, int after, boolean draftCleared, boolean sameTarget) {
        return sameTarget && draftCleared && after > before;
    }
}

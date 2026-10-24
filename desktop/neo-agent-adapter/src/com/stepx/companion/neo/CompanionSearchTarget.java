package com.stepx.companion.neo;

import java.util.regex.Matcher;
import java.util.regex.Pattern;

/** Normalize an explicit quoted search-editor instruction before native extraction. */
final class CompanionSearchTarget {
    private static final String OPEN = "“「『《\"‘'", CLOSE = "”」』》\"’'";
    private static final Pattern EDITOR = Pattern.compile(
        "搜索(?:输入)?[框栏](?:\\s*(?:中|内|里|后))?\\s*[，,]?\\s*(?:输入|键入|填写|填入)\\s*(?:(?:关键词|关键字)\\s*)?[:：]?\\s*([“「『《\"‘'])");
    static String normalizeEditorInstruction(String user) {
        if (user == null || user.isEmpty()) return user;
        Matcher matcher = EDITOR.matcher(user); StringBuilder result = new StringBuilder(); int copied = 0;
        while (matcher.find()) {
            if (matcher.start() < copied) continue;
            char opening = matcher.group(1).charAt(0), closing = CLOSE.charAt(OPEN.indexOf(opening));
            int end = user.indexOf(closing, matcher.end());
            if (end < 0 || end == matcher.end() || end - matcher.end() > 120) continue;
            String query = user.substring(matcher.end(), end);
            if (query.matches("(?s).*[\\p{Cntrl}\\u200B-\\u200F\\u202A-\\u202E].*")) continue;
            result.append(user, copied, matcher.start()).append("搜索");
            // Native SearchTarget supports double/title quotes. Preserve the
            // exact quoted value and every suffix (model, storage, colour, etc.).
            if (opening == '\'' || opening == '‘') result.append('“').append(query).append('”');
            else result.append(opening).append(query).append(closing);
            copied = end + 1;
        }
        return copied == 0 ? user : result.append(user, copied, user.length()).toString();
    }
}

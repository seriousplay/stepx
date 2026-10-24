package com.stepx.companion.app;

/** Exact private test skill allowlist; this is not a general skill installer. */
final class OfficialGuiSkillPolicy {
    static final String ID="802";
    static final String TITLE="StepX伴侣看屏操作";
    static void target(String id,String name,String title,String version) {
        // The market's installation identifier is not SKILL.md's runtime folder/name.
        // Pin the already verified market ID + display title, then use its own identifier.
        if(!ID.equals(id)||!TITLE.equals(title)||name==null||name.trim().isEmpty()||name.length()>200||name.matches("(?s).*[\\p{Cntrl}].*")||version==null||!version.matches("[0-9]+\\.[0-9]+\\.[0-9]+"))
            throw new IllegalArgumentException("官方测试技能身份不匹配，未发起安装");
    }
    static void install(String expected,String current,boolean testing,String requestId) {
        if(!testing||!current.equals(expected)||requestId==null||!requestId.matches("[a-fA-F0-9]{8}-[a-fA-F0-9]{4}-[a-fA-F0-9]{4}-[a-fA-F0-9]{4}-[a-fA-F0-9]{12}"))
            throw new IllegalArgumentException("测试技能版本或安装请求不匹配，请重新检查");
    }
    static String safeMessage(String message) {
        if(message==null)return "";
        if(message.matches("(?is).*(authorization|bearer|cookie|password|secret|token|https?://|[A-Za-z0-9_=-]{64,}).*"))return "官方服务返回错误，请在手机 Amoo 查看详情";
        return message.replaceAll("[\\p{Cntrl}]"," ").substring(0,Math.min(240,message.length()));
    }
    static String identityText(String value) {
        if(value==null)return "";
        // Only explicitly selected public catalog identity fields use this sanitizer.
        // An opaque long market ID is not an error string or an account credential.
        String clean=value.replaceAll("[\\p{Cntrl}]"," ").trim();
        return clean.substring(0,Math.min(200,clean.length()));
    }
}

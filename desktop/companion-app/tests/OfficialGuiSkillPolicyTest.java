package com.stepx.companion.app;
public final class OfficialGuiSkillPolicyTest {
    private static void reject(Runnable r){try{r.run();throw new AssertionError("accepted invalid input");}catch(IllegalArgumentException expected){}}
    public static void main(String[] args){
        OfficialGuiSkillPolicy.target("802","market-identifier",OfficialGuiSkillPolicy.TITLE,"0.8.0");
        reject(()->OfficialGuiSkillPolicy.target("803","market-identifier",OfficialGuiSkillPolicy.TITLE,"0.8.0"));
        reject(()->OfficialGuiSkillPolicy.target("802","market-identifier","其他技能","0.8.0"));
        reject(()->OfficialGuiSkillPolicy.target("802","",OfficialGuiSkillPolicy.TITLE,"0.8.0"));
        reject(()->OfficialGuiSkillPolicy.install("0.7.0","0.8.0",true,"12345678-1234-1234-1234-123456789abc"));
        reject(()->OfficialGuiSkillPolicy.install("0.8.0","0.8.0",false,"12345678-1234-1234-1234-123456789abc"));
        if(OfficialGuiSkillPolicy.safeMessage("Bearer secret").contains("secret"))throw new AssertionError("secret surfaced");
        if(!OfficialGuiSkillPolicy.safeMessage("skill.internal").equals("skill.internal"))throw new AssertionError("lost error");
        String opaque="abcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghij";
        if(!OfficialGuiSkillPolicy.identityText(opaque).equals(opaque))throw new AssertionError("catalog identity redacted");
        System.out.println("Official GUI skill policy: passed");
    }
}

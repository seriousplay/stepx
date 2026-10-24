package com.stepx.companion.neo;

public final class CompanionModelPolicyTest {
    private static void check(boolean value){if(!value)throw new AssertionError();}
    public static void main(String[] args){
        for(String endpoint:new String[]{"https://api.stepfun.com/step_plan/v1","https://api.stepfun.ai/v1/chat/completions","https://api.stepfun.com:443/v1/"})
            check(CompanionModelPolicy.step37(endpoint,"step-3.7-flash","OpenAI"));
        for(String endpoint:new String[]{"https://api.stepfun.com.attacker.test/v1","https://api.stepfun.com@attacker.test/v1","http://api.stepfun.com/v1","https://api.stepfun.com/other","https://api.stepfun.com:8443/v1","https://api.stepfun.com/v1?api_key=hidden",null})
            check(!CompanionModelPolicy.step37(endpoint,"step-3.7-flash","OpenAI"));
        check(!CompanionModelPolicy.step37("https://api.stepfun.com/v1","other-model","OpenAI"));
        check(!CompanionModelPolicy.step37("https://api.stepfun.com/v1","step-3.7-flash","Anthropic"));
        check(CompanionModelPolicy.outputTokens("https://api.stepfun.com/v1","step-3.7-flash","OpenAI",1536)==4096);
        check(CompanionModelPolicy.outputTokens("https://api.stepfun.com/v1","step-3.7-flash","OpenAI",6144)==6144);
        check(CompanionModelPolicy.outputTokens("https://other.test/v1","step-3.7-flash","OpenAI",1536)==1536);
        check(CompanionModelPolicy.outputTokens("https://api.stepfun.com/v1","step-3.7-flash","OpenAI",100000)==8192);
        check(CompanionModelPolicy.truncated("length"));check(CompanionModelPolicy.truncated("max_tokens"));check(CompanionModelPolicy.truncated("MAX_TOKENS"));
        check(!CompanionModelPolicy.truncated("stop"));check(!CompanionModelPolicy.truncated(null));
        check("low".equals(CompanionModelPolicy.effort(true)));check("medium".equals(CompanionModelPolicy.effort(false)));
        System.out.println("Official model endpoint, output budget and truncation boundaries passed");
    }
}

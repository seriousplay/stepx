package com.stepx.companion.app;
public final class StudioLocalPolicyTest {
    public static void main(String[] args)throws Exception {
        String a="local-0123456789abcdef0123456789abcdef",b="local-ffffffffffffffffffffffffffffffff";
        if(!StudioLocal.validId(a)||StudioLocal.validId("../x")||StudioLocal.validId("local-abc"))throw new AssertionError("id boundary");
        String u=StudioLocal.origin(a)+"/index.html";
        if(!StudioLocal.isDocument(a,u)||!StudioLocal.isDocument(a,u+"#score"))throw new AssertionError("document");
        for(String x:new String[]{StudioLocal.origin(b)+"/index.html",u+"?url=https://other.com",u+"/../x","file:///data/data/x","javascript:alert(1)","https://example.com","https://"+a+".local.stepx.invalid.evil/index.html"})if(StudioLocal.isDocument(a,x))throw new AssertionError(x);
        if(!StudioLocal.hash("abc".getBytes(java.nio.charset.StandardCharsets.UTF_8)).equals("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"))throw new AssertionError("sha256");
        System.out.println("StudioLocalPolicyTest passed");
    }
}

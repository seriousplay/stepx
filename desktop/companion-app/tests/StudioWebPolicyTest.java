package com.stepx.companion.app;
public final class StudioWebPolicyTest {
    public static void main(String[] args)throws Exception {
        for(String u:new String[]{"https://studio.stepx.com/projects","https://login-studio.stepx.com/login/new/login.html","https://studio-api.stepx.com/t/valid"})if(!StudioWeb.allowed(u))throw new AssertionError(u);
        for(String u:new String[]{"http://studio.stepx.com/","https://studio.stepx.com.evil/","https://attacker@studio.stepx.com/","file:///etc/passwd","javascript:alert(1)","https://studio.stepx.com:9000/","intent://login","https://other.com/"})if(StudioWeb.allowed(u))throw new AssertionError(u);
        String scope="a".repeat(64),other="b".repeat(64),id="p_new_app",old="p_existing";
        java.util.Set<String> records=java.util.Set.of(StudioWeb.createdKey(scope,id)),existing=java.util.Set.of(old,id);
        if(!StudioWeb.mayWriteProject(scope,id,records))throw new AssertionError("created project blocked");
        if(StudioWeb.mayWriteProject(other,id,records)||StudioWeb.mayWriteProject(scope,old,records)||StudioWeb.mayWriteProject("bad",id,records))throw new AssertionError("scope/ownership escape");
        if(!StudioWeb.mayRecordProject(scope,"p_fresh",existing,records)||!StudioWeb.mayRecordProject(scope,id,existing,records))throw new AssertionError("new/retry registration blocked");
        if(StudioWeb.mayRecordProject(scope,old,existing,records)||StudioWeb.mayRecordProject(other,id,existing,records)||StudioWeb.mayRecordProject(scope,"../p_fake",existing,records))throw new AssertionError("existing project hijack");
        if(!StudioWeb.sourcePath("/api/projects/p_x/file?path=src%2Fmain.tsx").equals("src/main.tsx"))throw new AssertionError("source read");
        for(String p:new String[]{"/api/projects/p_x/file?path=../secret","/api/projects/p_x/file?path=%252e%252e%2Fsecret","/api/projects/p_x/file?path=index.html&path=package.json","/api/projects/p_x/file?path=%2Fetc%2Fpasswd","/api/projects/p_x/file?path=a%5Cb","/api/projects/p_x/file?path=a//b","/api/projects/p_x/file?path=x&kind=directory","https://evil.example/api/projects/p_x/file?path=x","/api/projects/p_x/file?path=x#y"}){boolean rejected=false;try{StudioWeb.sourcePath(p);}catch(Exception e){rejected=true;}if(!rejected)throw new AssertionError(p);}
        System.out.println("StudioWebPolicyTest passed");
    }
}

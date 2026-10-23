package com.stepx.companion.app;

public final class GuiRetryPolicyTest {
    public static void main(String[] args) {
        for(int status:new int[]{408,429,500,502,503,504})
            if(!GuiRetryPolicy.http(status))throw new AssertionError("temporary error " + status);
        for(int status:new int[]{200,301,302,400,401,403,404,422,600})
            if(GuiRetryPolicy.http(status))throw new AssertionError("must not retry " + status);
        System.out.println("GuiRetryPolicyTest passed");
    }
}

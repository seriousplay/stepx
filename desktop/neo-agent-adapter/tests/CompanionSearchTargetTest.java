package com.stepx.companion.neo;

public final class CompanionSearchTargetTest {
    private static void equal(String expected,String actual){if(!expected.equals(actual))throw new AssertionError("Expected "+expected+", got "+actual);}
    private static void check(boolean value){if(!value)throw new AssertionError();}
    private static void search(String original,String expected,String pkg){
        String target=SearchTarget.fromUser(original);equal(expected,target);
        check(SearchQueryPolicy.allowed(original,original,target,pkg,expected));
    }
    public static void main(String[] args){
        String battery="打开设置，在设置搜索框输入“电池”，查看搜索结果，不修改任何设置。";
        search(battery,"电池","com.android.settings");
        search("在设置搜索框中输入“Android 版本”，查看结果，不修改设置。","Android 版本","com.android.settings");
        search("打开设置，在搜索栏里键入「电池健康」，然后查看结果。","电池健康","com.android.settings");
        search("点击搜索输入框后填写：\"Android 版本\"，查看结果。","Android 版本","com.android.settings");
        search("在搜索框内填入关键词‘系统更新’，只查看结果。","系统更新","com.android.settings");
        search("在搜索框，输入'蓝牙'，不要修改设置。","蓝牙","com.android.settings");
        String product="在京东搜索框输入“StepX Neo”16+256G 手机 白色，然后查看价格。";
        String target=SearchTarget.fromUser(product);equal("StepX Neo 16+256G 手机 白色",target);
        check(SearchQueryPolicy.allowed(product,product,target,"com.jingdong.app.mall",target));
        check(!SearchQueryPolicy.allowed(product,product,target,"com.jingdong.app.mall","StepX Neo 手机 白色"));
        check(!SearchQueryPolicy.allowed(product,product,target,"com.jingdong.app.mall","StepX Neo 16+512G 手机 白色"));
        check(!SearchQueryPolicy.allowed(product,product,target,"com.jingdong.app.mall","StepX Neo 16+256G 手机 黑色"));
        search("在淘宝搜索框输入「海信G7 Pro」65英寸，查看结果。","海信G7 Pro 65英寸","com.taobao.taobao");
        search("搜索《不再犹豫》，然后播放","不再犹豫","com.tencent.qqmusic");
        search("搜索 StepX Neo 16+256G 手机，然后查看价格","StepX Neo 16+256G 手机","com.jingdong.app.mall");
        check(!SearchQueryPolicy.allowed(battery,battery,SearchTarget.fromUser(battery),"com.android.settings","蓝牙"));
        for(String text:new String[]{"搜索框输入电池","搜索框输入“电池","搜索框输入“”","搜索框输入“电池\n付款”","搜索框删除“电池”"})
            equal(text,CompanionSearchTarget.normalizeEditorInstruction(text));
        check(CompanionSearchTarget.normalizeEditorInstruction(null)==null);
        System.out.println("Quoted search editor extraction, Chinese/Android terms and product specifications passed");
    }
}

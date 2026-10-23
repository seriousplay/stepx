use super::super::{theme::*, widgets::*, Action, Companion};
use facloud_ui::{elements::*, fonts::Weight};

impl Companion {
    pub(in super::super) fn about_page(&self) -> El {
        let p = self.p();
        let info = self.vstack(S5, vec![
            self.panel(vec![
                self.line_w("StepX", 32., p.text, Weight::Bold),
                self.text("手机与电脑之间，更顺手的工作台。", TITLE, p.text2),
                self.text("连接手机上的 Amoo、工作资料与背屏创作，在桌面继续对话、处理文件和管理设备。", BODY, p.text2),
                self.divider(),
                self.line(format!("版本 {}", stepx_companion::build_info::VERSION), BODY, p.text),
                self.line(format!("编译时间 {}", stepx_companion::build_info::compiled_at()), CAPTION, p.text3),
                self.hstack(S3, vec![
                    self.button("about-repository", "项目主页", Action::StudioOpenUrl("https://github.com/xydzf520/stepx".into()), Btn::Primary),
                    self.button("about-feedback", "反馈问题", Action::StudioOpenUrl("https://github.com/xydzf520/stepx/issues".into()), Btn::Secondary),
                ]),
            ]),
            self.panel(vec![
                self.line_w("联系与机会", HEADING, p.text, Weight::Semibold),
                self.text("目前在上海，寻求产品岗及复合型岗位机会，欢迎交流与推荐。", BODY, p.text2),
                self.between(
                    self.line_w("微信 · 164245026", TITLE, p.text, Weight::Medium),
                    self.button("about-copy-wechat", "复制微信", Action::CopyText("164245026".into(), "微信号已复制".into()), Btn::Secondary),
                ),
            ]),
            self.panel(vec![
                self.line_w("版权与开源", HEADING, p.text, Weight::Semibold),
                self.text("本项目为非官方开源项目，相关品牌与素材的权利归原权利人所有。如有侵权，请联系我，我会及时核实并删除相关内容。", BODY, p.text2),
                self.divider(),
                self.text("源码采用 AGPL-3.0-only 许可；第三方组件遵循各自许可。", CAPTION, p.text3),
                self.link_action("about-license", "查看开源许可与第三方说明", Action::StudioOpenUrl("https://github.com/xydzf520/stepx/blob/main/THIRD_PARTY_NOTICES.md".into())),
            ]),
        ]);
        ConstrainedBox::new(info).with_max_width(800.).finish()
    }
}

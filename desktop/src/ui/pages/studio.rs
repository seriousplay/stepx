use super::super::{theme::*, widgets::*, Action, Companion, F_STUDIO_NAME, F_STUDIO_URL};
use facloud_ui::{color::ColorU, elements::*, fonts::Weight, platform::Cursor};

impl Companion {
    pub(in super::super) fn studio_actions(&self) -> Vec<El> {
        vec![
            self.button(
                "st-open-web",
                "↗ 打开 studio.stepx.com",
                Action::StudioOpenUrl("https://studio.stepx.com/".into()),
                Btn::Secondary,
            ),
            self.button(
                "st-deploy-top",
                "✓ 一键发送到手机并建图标",
                Action::StudioDeployApp,
                Btn::Primary,
            ),
        ]
    }

    fn studio_app_card(
        &self,
        idx: usize,
        letter: &'static str,
        badge_bg: ColorU,
        name: &'static str,
        desc: &'static str,
        tag: &'static str,
    ) -> El {
        let p = self.p();
        let active = self.studio_app_idx == idx;
        let state = self.mouse_state(format!("st-app-card:{idx}"));
        let icon_box = ConstrainedBox::new(
            Container::new(
                Flex::row()
                    .with_main_axis_size(MainAxisSize::Max)
                    .with_main_axis_alignment(MainAxisAlignment::Center)
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_child(self.line_w(letter, CAPTION, ColorU::white(), Weight::Bold))
                    .finish(),
            )
            .with_background_color(badge_bg)
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
            .finish(),
        )
        .with_width(32.)
        .with_height(32.)
        .finish();

        let top_right = if active {
            Container::new(self.line_w("已选", 10., p.success_text, Weight::Medium))
                .with_horizontal_padding(6.)
                .with_vertical_padding(2.)
                .with_background_color(p.success_soft)
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
                .finish()
        } else {
            Empty::new().finish()
        };

        let content = Flex::column()
            .with_spacing(8.)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_child(self.between(icon_box, top_right))
            .with_child(
                Flex::column()
                    .with_spacing(3.)
                    .with_cross_axis_alignment(CrossAxisAlignment::Start)
                    .with_child(self.line_w(name, CAPTION, p.text, Weight::Semibold))
                    .with_child(self.text(desc, 11., p.text2))
                    .finish(),
            )
            .with_child(self.mono(tag, 10., p.text2))
            .finish();

        Hoverable::new(state, move |mouse| {
            let hovered = mouse.is_mouse_over_element();
            let bg = if active {
                if p.dark {
                    rgb(0x22, 0x22, 0x28)
                } else {
                    rgb(0xF4, 0xF4, 0xF6)
                }
            } else if hovered {
                p.hover
            } else {
                p.surface
            };
            let border = if active {
                p.accent
            } else if hovered {
                p.border_strong
            } else {
                p.surface_border
            };
            Container::new(content)
                .with_uniform_padding(12.)
                .with_background_color(bg)
                .with_border(Border::all(1.).with_border_color(border))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
                .finish()
        })
        .with_cursor(Cursor::PointingHand)
        .on_click(move |ctx, _, _| ctx.dispatch_typed_action(Action::StudioSelectApp(idx)))
        .finish()
    }

    pub(in super::super) fn studio_page(&self) -> El {
        let p = self.p();

        // 1. Left 7 Cols: studio.stepx.com App Gallery & One-Click Deploy to Phone Home Screen
        let app_cards = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_spacing(10.)
            .with_child(
                Expanded::new(
                    1.,
                    self.studio_app_card(
                        0,
                        "画",
                        p.success,
                        "背屏像素画师",
                        "一句话生成 23×17 背屏动画与跑马灯",
                        "WebApp · 背屏联动",
                    ),
                )
                .finish(),
            )
            .with_child(
                Expanded::new(
                    1.,
                    self.studio_app_card(
                        1,
                        "票",
                        p.blue,
                        "发票报销助手",
                        "拍照自动提取发票金额并写入报销表",
                        "Skill · 视觉识别",
                    ),
                )
                .finish(),
            )
            .with_child(
                Expanded::new(
                    1.,
                    self.studio_app_card(
                        2,
                        "勤",
                        p.warning,
                        "晨间通勤播报",
                        "结合高德路况与飞书日历语音播报",
                        "Agent · 工作流",
                    ),
                )
                .finish(),
            )
            .finish();

        let config_inputs = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_spacing(12.)
            .with_child(
                Expanded::new(
                    4.,
                    self.vstack(
                        4.,
                        vec![
                            self.line("手机桌面图标名称", CAPTION, p.text2),
                            self.input(F_STUDIO_NAME, "背屏像素画师", None),
                        ],
                    ),
                )
                .finish(),
            )
            .with_child(
                Expanded::new(
                    8.,
                    self.vstack(
                        4.,
                        vec![
                            self.line("Studio 应用地址或本地 Skill 包", CAPTION, p.text2),
                            self.input(
                                F_STUDIO_URL,
                                "https://studio.stepx.com/app/matrix-painter",
                                None,
                            ),
                        ],
                    ),
                )
                .finish(),
            )
            .finish();

        let config_checks = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_spacing(8.)
            .with_child(
                Expanded::new(
                    1.,
                    self.checkbox_row(
                        "st-chk-icon",
                        self.studio_create_icon,
                        "在手机桌面创建图标",
                        Action::StudioToggleCreateIcon,
                    ),
                )
                .finish(),
            )
            .with_child(
                Expanded::new(
                    1.,
                    self.checkbox_row(
                        "st-chk-skill",
                        self.studio_sync_skill,
                        "同步到手机技能库",
                        Action::StudioToggleSyncSkill,
                    ),
                )
                .finish(),
            )
            .with_child(
                Expanded::new(
                    1.,
                    self.checkbox_row(
                        "st-chk-open",
                        self.studio_open_now,
                        "发送后立即在手机打开",
                        Action::StudioToggleOpenNow,
                    ),
                )
                .finish(),
            )
            .finish();

        let config_box = Container::new(self.vstack(12., vec![config_inputs, config_checks]))
            .with_uniform_padding(14.)
            .with_background_color(p.surface)
            .with_border(Border::all(1.).with_border_color(p.surface_border))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
            .finish();

        let (cur_letter, cur_color, cur_short) = match self.studio_app_idx {
            1 => ("票", p.blue, "发票报销"),
            2 => ("勤", p.warning, "晨间通勤"),
            _ => ("画", p.success, "背屏像素"),
        };

        let dock_icon = |letter: &'static str, bg: ColorU, label: &'static str, bold: bool| -> El {
            let box_el = ConstrainedBox::new(
                Container::new(
                    Flex::row()
                        .with_main_axis_size(MainAxisSize::Max)
                        .with_main_axis_alignment(MainAxisAlignment::Center)
                        .with_cross_axis_alignment(CrossAxisAlignment::Center)
                        .with_child(self.line_w(letter, CAPTION, ColorU::white(), Weight::Bold))
                        .finish(),
                )
                .with_background_color(bg)
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(10.)))
                .finish(),
            )
            .with_width(32.)
            .with_height(32.)
            .finish();

            Flex::column()
                .with_spacing(4.)
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_child(box_el)
                .with_child(self.line_w(
                    label,
                    10.,
                    if bold { p.text } else { p.text2 },
                    if bold {
                        Weight::Semibold
                    } else {
                        Weight::Normal
                    },
                ))
                .finish()
        };

        let dock_preview = Container::new(self.hstack(
            10.,
            vec![
                dock_icon("A", rgb(0x18, 0x18, 0x1B), "Amoo", false),
                dock_icon(cur_letter, cur_color, cur_short, true),
                dock_icon("S", p.blue, "Studio", false),
            ],
        ))
        .with_horizontal_padding(10.)
        .with_vertical_padding(8.)
        .with_background_color(p.surface)
        .with_border(Border::all(1.).with_border_color(p.surface_border))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
        .finish();

        let preview_top = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_spacing(10.)
            .with_child(dock_preview)
            .with_child(
                Expanded::new(
                    1.,
                    Flex::column()
                        .with_spacing(3.)
                        .with_cross_axis_alignment(CrossAxisAlignment::Start)
                        .with_child(self.line_w(
                            "手机桌面图标预览",
                            CAPTION,
                            p.text,
                            Weight::Semibold,
                        ))
                        .with_child(self.line(
                            "发送后将在手机桌面生成独立图标，点击即可直接运行",
                            11.,
                            p.text2,
                        ))
                        .finish(),
                )
                .finish(),
            )
            .with_child(self.button(
                "st-deploy-main",
                if self.studio_deployed {
                    "✓ 已发送到手机"
                } else {
                    "一键发送到手机"
                },
                Action::StudioDeployApp,
                Btn::Primary,
            ))
            .finish();

        let step_bar = Container::new(self.between(
            self.hstack(
                16.,
                vec![
                    self.line_w("✓ 1. 打包 Studio 应用", CAPTION, p.success, Weight::Medium),
                    self.line_w("✓ 2. 注入手机技能库", CAPTION, p.success, Weight::Medium),
                    self.line_w(
                        if self.studio_deployed {
                            "✓ 3. 桌面图标已生成"
                        } else {
                            "● 3. 桌面图标已生成"
                        },
                        CAPTION,
                        if self.studio_deployed {
                            p.success
                        } else {
                            p.text2
                        },
                        Weight::Semibold,
                    ),
                ],
            ),
            self.mono("桌面直推就绪", 11., p.text2),
        ))
        .with_padding_top(8.)
        .with_border(Border::top(1.).with_border_color(p.border))
        .finish();

        let preview_card = Container::new(self.vstack(10., vec![preview_top, step_bar]))
            .with_uniform_padding(14.)
            .with_background_color(p.bg)
            .with_border(Border::all(1.).with_border_color(p.border))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
            .finish();

        let workshop_header = self.between(
            self.hstack(
                8.,
                vec![
                    self.heading("StepX Studio 应用工坊"),
                    Container::new(self.mono("studio.stepx.com", 11., p.text))
                        .with_horizontal_padding(8.)
                        .with_vertical_padding(2.)
                        .with_background_color(p.surface)
                        .with_border(Border::all(1.).with_border_color(p.surface_border))
                        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
                        .finish(),
                ],
            ),
            self.line("选择应用一键发送到手机桌面", CAPTION, p.text2),
        );

        let left_card = Container::new(
            Flex::column()
                .with_main_axis_size(MainAxisSize::Max)
                .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
                .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                .with_child(self.vstack(14., vec![workshop_header, app_cards, config_box]))
                .with_child(preview_card)
                .finish(),
        )
        .with_uniform_padding(S4)
        .with_background_color(p.bg)
        .with_border(Border::all(1.).with_border_color(p.border))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
        .finish();

        // 2. Right 5 Cols: Phone Built-in OpenClaw Reverse Proxy & Remote Machine Sharing
        let service_row = |id: &str,
                           icon: &'static str,
                           icon_fg: ColorU,
                           title: &'static str,
                           endpoint: String,
                           endpoint_fg: ColorU,
                           btn_label: &'static str,
                           act: Action|
         -> El {
            let icon_bg = if p.dark { p.bg } else { ColorU::white() };
            let icon_border = if p.dark {
                p.surface_border
            } else {
                rgb(0xE4, 0xE4, 0xE7)
            };
            let box_el = self.icon_box(icon, 32., icon_bg, icon_fg, Some(icon_border));
            let text_col = Flex::column()
                .with_spacing(2.)
                .with_cross_axis_alignment(CrossAxisAlignment::Start)
                .with_child(self.line_w(title, CAPTION, p.text, Weight::Semibold))
                .with_child(self.mono(endpoint, CAPTION, endpoint_fg))
                .finish();
            Container::new(self.between(
                self.hstack(10., vec![box_el, text_col]),
                self.button(id, btn_label, act, Btn::Secondary),
            ))
            .with_uniform_padding(12.)
            .with_background_color(p.surface)
            .with_border(Border::all(1.).with_border_color(p.surface_border))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
            .finish()
        };

        let gw_url = format!("127.0.0.1:{}", self.studio_status.gateway_port);
        let model_url = format!(
            "127.0.0.1:{} · OpenAI 兼容",
            self.studio_status.model_proxy_port
        );
        let lan_url = format!(
            "http://{}:{}",
            self.studio_status.lan_ip, self.studio_status.gateway_port
        );

        let svc1 = service_row(
            "st-svc-gw",
            "⌘",
            p.success,
            "OpenClaw 网关控制台",
            gw_url,
            p.success,
            "打开",
            Action::StudioOpenUrl(format!(
                "http://127.0.0.1:{}/?token={}",
                self.studio_status.gateway_port, self.studio_status.token
            )),
        );
        let svc2 = service_row(
            "st-svc-model",
            "⚡",
            p.blue,
            "StepOS 模型流式代理",
            model_url,
            p.text2,
            "复制",
            Action::CopyText(
                format!("http://127.0.0.1:{}", self.studio_status.model_proxy_port),
                "已复制模型代理地址".into(),
            ),
        );
        let svc3 = service_row(
            "st-svc-lan",
            "🌐",
            p.indigo,
            "远端机器 / 局域网固定访问",
            lan_url.clone(),
            p.blue,
            "复制",
            Action::CopyText(lan_url, "已复制局域网固定访问地址".into()),
        );

        let token_short = if self.studio_status.token.len() > 38 {
            format!("{}…", &self.studio_status.token[..38])
        } else {
            self.studio_status.token.clone()
        };

        let token_box = Container::new(self.vstack(
            6.,
            vec![
                self.between(
                    self.line_w("设备鉴权令牌 (Token)", CAPTION, p.text2, Weight::Medium),
                    self.link_action(
                        "st-copy-token",
                        "复制令牌",
                        Action::CopyText(
                            self.studio_status.token.clone(),
                            "已复制设备鉴权令牌".into(),
                        ),
                    ),
                ),
                self.mono(token_short, CAPTION, p.text),
            ],
        ))
        .with_uniform_padding(12.)
        .with_background_color(p.surface)
        .with_border(Border::all(1.).with_border_color(p.surface_border))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
        .finish();

        let proxy_bottom_btns = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_spacing(8.)
            .with_child(
                Expanded::new(
                    1.,
                    self.button(
                        "st-btn-remap",
                        "重置端口映射",
                        Action::StudioResetProxy,
                        Btn::Secondary,
                    ),
                )
                .finish(),
            )
            .with_child(
                Expanded::new(
                    1.,
                    self.button(
                        "st-btn-console",
                        "打开本地控制台",
                        Action::StudioOpenUrl(format!(
                            "http://127.0.0.1:{}/?token={}",
                            self.studio_status.gateway_port, self.studio_status.token
                        )),
                        Btn::Primary,
                    ),
                )
                .finish(),
            )
            .finish();

        let proxy_header = self.between(
            Flex::column()
                .with_spacing(2.)
                .with_cross_axis_alignment(CrossAxisAlignment::Start)
                .with_child(self.heading("手机内置网关与反向代理"))
                .with_child(self.line(
                    "将手机内置算力与网关暴露给本机或固定远端机器",
                    CAPTION,
                    p.text2,
                ))
                .finish(),
            self.pill_badge("已开启", p.success_text, p.success_soft),
        );

        let right_card = Container::new(
            Flex::column()
                .with_main_axis_size(MainAxisSize::Max)
                .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
                .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                .with_child(self.vstack(
                    12.,
                    vec![
                        proxy_header,
                        self.vstack(8., vec![svc1, svc2, svc3]),
                        token_box,
                    ],
                ))
                .with_child(proxy_bottom_btns)
                .finish(),
        )
        .with_uniform_padding(S4)
        .with_background_color(p.bg)
        .with_border(Border::all(1.).with_border_color(p.border))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
        .finish();

        Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_spacing(20.)
            .with_child(Expanded::new(7., left_card).finish())
            .with_child(Expanded::new(5., right_card).finish())
            .finish()
    }
}

//! Window chrome: sidebar, page header, status bar and shared page states.

use super::{
    icons::{self, UiIcon},
    theme::*,
    widgets::*,
    *,
};
use facloud_ui::{color::ColorU, fonts::Weight, platform::Cursor};

impl Companion {
    pub(super) fn scrollable(&self, state: ClippedScrollStateHandle, content: El) -> El {
        let p = self.p();
        let thumb = ColorU::new(p.text3.r, p.text3.g, p.text3.b, 90);
        let thumb_active = ColorU::new(p.text3.r, p.text3.g, p.text3.b, 160);
        ClippedScrollable::vertical(
            state,
            Container::new(content).with_uniform_padding(S5).finish(),
            ScrollbarWidth::Auto,
            thumb.into(),
            thumb_active.into(),
            Fill::None,
        )
        .finish()
    }

    /// Same scroll container without page padding (for panes that draw their own).
    pub(super) fn pane_scroll(&self, state: ClippedScrollStateHandle, content: El) -> El {
        let p = self.p();
        let thumb = ColorU::new(p.text3.r, p.text3.g, p.text3.b, 90);
        let thumb_active = ColorU::new(p.text3.r, p.text3.g, p.text3.b, 160);
        ClippedScrollable::vertical(
            state,
            content,
            ScrollbarWidth::Auto,
            thumb.into(),
            thumb_active.into(),
            Fill::None,
        )
        .finish()
    }

    pub(super) fn loading_page(&self) -> El {
        let p = self.p();
        let dots = (0..4)
            .map(|index| {
                self.dot(if index == self.loading_frame {
                    p.accent
                } else {
                    p.border
                })
            })
            .collect();
        Align::new(self.vstack(
            S4,
            vec![
            Align::new(self.ui_icon(UiIcon::Smartphone, 36., p.text2)).finish(),
            Align::new(self.line_w("正在载入手机数据", TITLE, p.text, Weight::Semibold)).finish(),
            Align::new(self.line(&self.status, CAPTION, p.text2)).finish(),
            Align::new(self.hstack(S2, dots)).finish(),
        ],
        ))
        .finish()
    }

    pub(super) fn no_device(&self) -> El {
        let p = self.p();
        let ready = self.devices.iter().filter(|d| d.state == "device").count();
        let count_label = if ready > 0 {
            format!("{ready} 台待选")
        } else {
            "未检测到可用设备".to_string()
        };
        let candidate_label = self
            .devices
            .first()
            .map(|d| {
                let model = if d.model.is_empty() {
                    "未知型号".to_string()
                } else {
                    d.model.replace('_', " ")
                };
                format!("{model} (USB)")
            })
            .unwrap_or_else(|| "连接手机后将在这里显示".to_string());

        let icon_bg = if p.dark {
            rgb(0x27, 0x27, 0x2A)
        } else {
            rgb(0xF4, 0xF4, 0xF6)
        };
        let phone_icon = ConstrainedBox::new(
            Container::new(
                Flex::row()
                    .with_main_axis_size(MainAxisSize::Max)
                    .with_main_axis_alignment(MainAxisAlignment::Center)
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_child(self.line_w("📱", TITLE, p.text, Weight::Semibold))
                    .finish(),
            )
            .with_background_color(icon_bg)
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(16.)))
            .finish(),
        )
        .with_width(48.)
        .with_height(48.)
        .finish();

        let channel_box = Container::new(self.vstack(
            S2,
            vec![
                self.between(
                    self.line(
                        if ready > 0 {
                            "检测到未选择通道"
                        } else {
                            "等待连接"
                        },
                        CAPTION,
                        p.text2,
                    ),
                    self.mono(count_label, CAPTION, p.warning),
                ),
                self.divider(),
                self.between(
                    self.mono(candidate_label, CAPTION, p.text),
                    self.link_action("nodev-pick", "前往选择", Action::Page(Page::Connect)),
                ),
            ],
        ))
        .with_uniform_padding(14.)
        .with_background_color(p.surface)
        .with_border(Border::all(1.).with_border_color(p.surface_border))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
        .finish();

        let card = ConstrainedBox::new(
            Container::new(
                Flex::column()
                    .with_spacing(S4 + 4.)
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_child(phone_icon)
                    .with_child(
                        Flex::column()
                            .with_spacing(6.)
                            .with_cross_axis_alignment(CrossAxisAlignment::Center)
                            .with_child(self.line_w(
                                "尚未连接 StepX 设备",
                                TITLE,
                                p.text,
                                Weight::Semibold,
                            ))
                            .with_child(self.text(
                                "从 USB 调试授权开始，按引导安装 StepX伴侣，再切换无线扫码连接。已有系统无线调试配对也可直接连接。",
                                CAPTION,
                                p.text2,
                            ))
                            .finish(),
                    )
                    .with_child(
                        Flex::column()
                            .with_main_axis_size(MainAxisSize::Min)
                            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                            .with_child(channel_box)
                            .finish(),
                    )
                    .with_child(self.hstack(
                        10.,
                        vec![
                            self.button("nodev-refresh", "刷新检测", Action::Refresh, Btn::Secondary),
                            self.button(
                                "nodev-connect",
                                "开始连接引导",
                                Action::ConnectGuide(Some(0)),
                                Btn::Primary,
                            ),
                        ],
                    ))
                    .finish(),
            )
            .with_uniform_padding(28.)
            .with_background_color(p.bg)
            .with_border(Border::all(1.).with_border_color(p.border))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(16.)))
            .finish(),
        )
        .with_width(440.)
        .finish();

        Container::new(
            Flex::column()
                .with_main_axis_size(MainAxisSize::Max)
                .with_main_axis_alignment(MainAxisAlignment::Center)
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_child(card)
                .finish(),
        )
        .with_uniform_padding(S5)
        .finish()
    }

    fn nav_group_title(&self, title: &'static str, first: bool) -> El {
        let p = self.p();
        Container::new(
            facloud_ui::elements::Text::new(title, self.font, 10.)
                .with_color(p.text3)
                .with_style(facloud_ui::fonts::Properties::default().weight(Weight::Semibold))
                .soft_wrap(false)
                .finish(),
        )
        .with_horizontal_padding(10.)
        .with_padding_top(if first { 0. } else { 10. })
        .with_padding_bottom(4.)
        .finish()
    }

    fn nav_item_badge(
        &self,
        id: &str,
        icon: &'static str,
        label: &'static str,
        badge: Option<String>,
        active: bool,
        action: Action,
    ) -> El {
        let p = self.p();
        let state = self.mouse_state(format!("nav:{id}"));
        let font = self.font;
        let mono_font = self.mono_font;
        Hoverable::new(state, move |mouse| {
            let hovered = mouse.is_mouse_over_element();
            let bg = if active {
                p.selected
            } else if hovered {
                p.hover
            } else {
                ColorU::transparent_black()
            };
            let border = if active && !p.dark {
                Border::all(1.).with_border_color(rgb(0xE4, 0xE4, 0xE7))
            } else {
                Border::all(1.).with_border_color(ColorU::transparent_black())
            };
            let icon_fg = if active { p.text } else { p.text2 };
            let text_fg = if active {
                p.text
            } else if p.dark {
                p.text2
            } else {
                rgb(0x52, 0x52, 0x5B)
            };
            let left = Flex::row()
                .with_spacing(10.)
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_child(
                    ConstrainedBox::new(
                        Align::new(if icon == "✦" {
                            // Keep the original Amoo navigation mark.
                            facloud_ui::elements::Text::new(icon, font, CAPTION)
                                .with_color(icon_fg)
                                .finish()
                        } else {
                            icons::draw_icon(
                                icons::symbol(icon).unwrap_or(UiIcon::Apps),
                                16.,
                                icon_fg,
                            )
                        })
                        .finish(),
                    )
                    .with_width(20.)
                    .with_height(20.)
                    .finish(),
                )
                .with_child(
                    facloud_ui::elements::Text::new(label, font, CAPTION)
                        .with_color(text_fg)
                        .with_style(facloud_ui::fonts::Properties::default().weight(if active {
                            Weight::Semibold
                        } else {
                            Weight::Medium
                        }))
                        .soft_wrap(false)
                        .finish(),
                )
                .finish();
            let row = match &badge {
                Some(txt) => {
                    let (badge_bg, badge_fg) = if active {
                        if p.dark {
                            (rgba(0xFF, 0xFF, 0xFF, 38), ColorU::white())
                        } else {
                            (rgb(0xF4, 0xF4, 0xF5), rgb(0x18, 0x18, 0x1B))
                        }
                    } else if p.dark {
                        (rgba(0xFF, 0xFF, 0xFF, 14), rgb(0x71, 0x71, 0x7A))
                    } else {
                        (rgba(0x00, 0x00, 0x00, 13), rgb(0x71, 0x71, 0x7A))
                    };
                    let badge_node = Container::new(
                        facloud_ui::elements::Text::new(txt.clone(), mono_font, 10.)
                            .with_color(badge_fg)
                            .with_style(
                                facloud_ui::fonts::Properties::default().weight(Weight::Medium),
                            )
                            .soft_wrap(false)
                            .finish(),
                    )
                    .with_horizontal_padding(6.)
                    .with_vertical_padding(1.)
                    .with_background_color(badge_bg)
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.)))
                    .finish();
                    Flex::row()
                        .with_main_axis_size(MainAxisSize::Max)
                        .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
                        .with_cross_axis_alignment(CrossAxisAlignment::Center)
                        .with_child(left)
                        .with_child(badge_node)
                        .finish()
                }
                None => left,
            };
            Container::new(row)
                .with_horizontal_padding(10.)
                .with_vertical_padding(6.)
                .with_background_color(bg)
                .with_border(border)
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_CONTROL)))
                .finish()
        })
        .with_cursor(Cursor::PointingHand)
        .on_click(move |ctx, _, _| ctx.dispatch_typed_action(action.clone()))
        .finish()
    }

    fn device_block(&self) -> El {
        let p = self.p();
        let state = self.mouse_state("device-block");
        let card_bg = if p.dark {
            rgb(0x1C, 0x1C, 0x21)
        } else {
            ColorU::white()
        };
        let card_border = if p.dark {
            rgb(0x2E, 0x2E, 0x36)
        } else {
            rgb(0xE4, 0xE4, 0xE7)
        };
        let icon_bg = if p.dark {
            rgb(0x27, 0x27, 0x2A)
        } else {
            rgb(0xF4, 0xF4, 0xF5)
        };

        let content = match &self.selected {
            Some(d) => {
                let metrics = stepx_companion::device::DeviceMetrics::from_summary(&self.summary);
                let transport = if is_wireless(&d.transport) {
                    "无线直连"
                } else {
                    "USB 直连"
                };
                let detail = match metrics.battery_level {
                    Some(b) => format!("{transport} · {b}%"),
                    None => format!("{transport} · 电量未读取"),
                };
                let title = if d.model.contains("StepX") {
                    d.model.clone()
                } else {
                    format!("StepX {}", d.model)
                };
                let icon_box = ConstrainedBox::new(
                    Container::new(
                        Flex::row()
                            .with_main_axis_size(MainAxisSize::Max)
                            .with_main_axis_alignment(MainAxisAlignment::Center)
                            .with_cross_axis_alignment(CrossAxisAlignment::Center)
                            .with_child(self.line("📱", CAPTION, p.text))
                            .finish(),
                    )
                    .with_background_color(icon_bg)
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_CONTROL)))
                    .finish(),
                )
                .with_width(32.)
                .with_height(32.)
                .finish();

                let text_col = Flex::column()
                    .with_spacing(2.)
                    .with_cross_axis_alignment(CrossAxisAlignment::Start)
                    .with_child(self.lead(
                        6.,
                        self.dot(p.success),
                        self.line_w(title, CAPTION, p.text, Weight::Semibold),
                    ))
                    .with_child(self.line(detail, CAPTION, p.text2))
                    .finish();

                self.between(
                    self.hstack(10., vec![icon_box, text_col]),
                    self.line("⇅", CAPTION, p.text3),
                )
            }
            None => {
                let icon_box = ConstrainedBox::new(
                    Container::new(
                        Flex::row()
                            .with_main_axis_size(MainAxisSize::Max)
                            .with_main_axis_alignment(MainAxisAlignment::Center)
                            .with_cross_axis_alignment(CrossAxisAlignment::Center)
                            .with_child(self.line("📱", CAPTION, p.text2))
                            .finish(),
                    )
                    .with_background_color(icon_bg)
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_CONTROL)))
                    .finish(),
                )
                .with_width(32.)
                .with_height(32.)
                .finish();

                let text_col = Flex::column()
                    .with_spacing(2.)
                    .with_cross_axis_alignment(CrossAxisAlignment::Start)
                    .with_child(self.line_w("未连接设备", CAPTION, p.text, Weight::Semibold))
                    .with_child(self.line("点击前往配对", CAPTION, p.text2))
                    .finish();

                self.between(
                    self.hstack(10., vec![icon_box, text_col]),
                    self.line("›", CAPTION, p.text3),
                )
            }
        };

        Hoverable::new(state, move |mouse| {
            Container::new(content)
                .with_uniform_padding(10.)
                .with_background_color(if mouse.is_mouse_over_element() {
                    p.hover
                } else {
                    card_bg
                })
                .with_border(Border::all(1.).with_border_color(card_border))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
                .finish()
        })
        .with_cursor(Cursor::PointingHand)
        .on_click(|ctx, _, _| ctx.dispatch_typed_action(Action::Page(Page::Connect)))
        .finish()
    }

    pub(super) fn sidebar(&self) -> El {
        let p = self.p();
        let sms_badge =
            (self.sms_loaded && self.sms_error.is_none()).then(|| format!("{} 条", self.sms.len()));
        let app_badge = (self.apps_loaded && self.apps_error.is_none()).then(|| {
            self.apps
                .iter()
                .filter(|app| app.in_default_scope())
                .count()
                .to_string()
        });

        let logo_box = ConstrainedBox::new(
            Image::new(
                facloud_ui::assets::asset_cache::AssetSource::Bundled {
                    path: "amoo_desktop_icon.png",
                },
                CacheOption::BySize,
            )
            .contain()
            .finish(),
        )
        .with_width(28.)
        .with_height(28.)
        .finish();

        let brand_row = Container::new(
            self.between(
                self.hstack(
                    10.,
                    vec![
                        logo_box,
                        self.hstack(
                            6.,
                            vec![
                                self.line_w("StepX", SUBHEAD, p.text, Weight::Semibold),
                                self.mono(stepx_companion::build_info::VERSION, CAPTION, p.text2),
                            ],
                        ),
                    ],
                ),
                Container::new(self.mono("桌面端", 11., p.text2))
                    .with_horizontal_padding(6.)
                    .with_vertical_padding(2.)
                    .with_background_color(if p.dark {
                        rgba(0xFF, 0xFF, 0xFF, 14)
                    } else {
                        rgba(0x00, 0x00, 0x00, 10)
                    })
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
                    .finish(),
            ),
        )
        .with_horizontal_padding(6.)
        .with_padding_top(2.)
        .finish();

        let top_group = Flex::column()
            .with_spacing(14.)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_child(brand_row)
            .with_child(self.device_block())
            .with_child(
                Flex::column()
                    .with_spacing(2.)
                    .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                    .with_children(vec![
                        self.nav_group_title("设备与硬件", true),
                        self.nav_item_badge(
                            "overview",
                            Page::Overview.icon(),
                            Page::Overview.title(),
                            None,
                            matches!(self.page, Page::Overview | Page::Account),
                            Action::Page(Page::Overview),
                        ),
                        self.nav_item_badge(
                            "backscreen",
                            Page::BackScreen.icon(),
                            Page::BackScreen.title(),
                            Some("点阵".into()),
                            self.page == Page::BackScreen,
                            Action::Page(Page::BackScreen),
                        ),
                        self.nav_group_title("智能与开发", false),
                        self.nav_item_badge(
                            "amoo",
                            Page::Amoo.icon(),
                            Page::Amoo.title(),
                            None,
                            self.page == Page::Amoo,
                            Action::Page(Page::Amoo),
                        ),
                        self.nav_item_badge(
                            "work",
                            Page::Work.icon(),
                            Page::Work.title(),
                            if self.work.running {
                                Some("进行中".into())
                            } else {
                                None
                            },
                            self.page == Page::Work,
                            Action::Page(Page::Work),
                        ),
                        self.nav_item_badge(
                            "studio",
                            Page::Studio.icon(),
                            Page::Studio.title(),
                            Some("上机".into()),
                            self.page == Page::Studio,
                            Action::Page(Page::Studio),
                        ),
                        self.nav_group_title("通信与数据", false),
                        self.nav_item_badge(
                            "sms",
                            Page::Sms.icon(),
                            Page::Sms.title(),
                            sms_badge,
                            self.page == Page::Sms,
                            Action::Page(Page::Sms),
                        ),
                        self.nav_item_badge(
                            "phone",
                            Page::Phone.icon(),
                            Page::Phone.title(),
                            None,
                            self.page == Page::Phone,
                            Action::Page(Page::Phone),
                        ),
                        self.nav_item_badge(
                            "files",
                            Page::Files.icon(),
                            Page::Files.title(),
                            None,
                            self.page == Page::Files,
                            Action::Page(Page::Files),
                        ),
                        self.nav_item_badge(
                            "apps",
                            Page::Apps.icon(),
                            Page::Apps.title(),
                            app_badge,
                            self.page == Page::Apps,
                            Action::Page(Page::Apps),
                        ),
                        self.nav_item_badge(
                            "tools",
                            Page::Tools.icon(),
                            Page::Tools.title(),
                            None,
                            self.page == Page::Tools,
                            Action::Page(Page::Tools),
                        ),
                        self.nav_item_badge(
                            "connect",
                            Page::Connect.icon(),
                            Page::Connect.title(),
                            None,
                            self.page == Page::Connect,
                            Action::Page(Page::Connect),
                        ),
                    ])
                    .finish(),
            )
            .finish();

        self.fixed_w(
            SIDEBAR_W,
            Container::new(
                Flex::column()
                    .with_main_axis_size(MainAxisSize::Max)
                    .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
                    .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                    .with_child(top_group)
                    .with_child(self.vstack(
                        S2,
                        vec![
                            self.button("sidebar-about", "关于 StepX", Action::Page(Page::About), if self.page == Page::About { Btn::Secondary } else { Btn::Ghost }),
                        ],
                    ))
                    .finish(),
            )
            .with_uniform_padding(12.)
            .with_background_color(p.sidebar)
            .with_border(Border::right(1.).with_border_color(p.border))
            .with_corner_radius(CornerRadius::with_top_left(Radius::Pixels(12.)))
            .finish(),
        )
    }

    fn window_control_btn(
        &self,
        id: &str,
        symbol: &'static str,
        danger: bool,
        action: Action,
    ) -> El {
        let p = self.p();
        let state = self.mouse_state(format!("win:{id}"));
        Hoverable::new(state, move |mouse| {
            let hovered = mouse.is_mouse_over_element();
            let bg = if hovered {
                if danger {
                    p.danger
                } else {
                    p.hover
                }
            } else {
                ColorU::transparent_black()
            };
            let fg = if hovered && danger {
                ColorU::white()
            } else if hovered {
                p.text
            } else {
                p.text2
            };
            ConstrainedBox::new(
                Container::new(
                    Flex::row()
                        .with_main_axis_size(MainAxisSize::Max)
                        .with_main_axis_alignment(MainAxisAlignment::Center)
                        .with_cross_axis_alignment(CrossAxisAlignment::Center)
                        .with_child(icons::draw_icon(
                            match symbol {
                                "—" => UiIcon::Minus,
                                "□" => UiIcon::Maximize,
                                _ => UiIcon::Close,
                            },
                            14.,
                            fg,
                        ))
                        .finish(),
                )
                .with_background_color(bg)
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.)))
                .finish(),
            )
            .with_width(28.)
            .with_height(28.)
            .finish()
        })
        .with_cursor(Cursor::PointingHand)
        .on_click(move |ctx, _, _| ctx.dispatch_typed_action(action.clone()))
        .finish()
    }

    fn page_subtitle(&self) -> String {
        if self.selected.is_none() && self.page.needs_device() {
            return String::new();
        }
        match self.page {
            Page::Overview => {
                let android = self
                    .selected
                    .as_ref()
                    .map(|d| d.android.as_str())
                    .unwrap_or("16");
                format!(
                    "{} · Android {android} · 设备已连接",
                    self.selected
                        .as_ref()
                        .map(|d| d.model.as_str())
                        .unwrap_or("设备")
                )
            }
            Page::BackScreen => "17 列 × 23 行 · 379 颗单色灯珠 · 创作与手机联动".into(),
            Page::Amoo => "后台连接 · 会话与文字对话".into(),
            Page::Account => "手机账号 · 真实消息 Token 用量".into(),
            Page::About => String::new(),
            Page::Work => "手机任务同步 · 本地文件协作".into(),
            Page::Studio => "studio.stepx.com 应用一键上机建图标 · OpenClaw 远端反向代理".into(),
            Page::Sms => {
                if self.sms_error.is_some() {
                    "短信读取失败，请刷新重试".into()
                } else if !self.sms_loaded {
                    "正在读取手机短信…".into()
                } else {
                    let threads = self
                        .sms
                        .iter()
                        .map(|m| stepx_companion::sms::normalize_address(&m.address))
                        .collect::<std::collections::HashSet<_>>()
                        .len();
                    let unread = self.sms.iter().filter(|m| m.kind == 1 && !m.read).count();
                    format!(
                        "{} 条短信 · {threads} 个会话 · {unread} 条未读",
                        self.sms.len()
                    )
                }
            }
            Page::Phone => "通话记录、手机通讯录与桌面直接拨号".into(),
            Page::Files => {
                let m = stepx_companion::device::DeviceMetrics::from_summary(&self.summary);
                if m.storage_total.is_empty() {
                    "内部存储 · 容量暂不可用".into()
                } else {
                    format!("数据分区 · 可用 {} / {}", m.storage_avail, m.storage_total)
                }
            }
            Page::Apps => "管理手机端已安装应用与 APK 安装".into(),
            Page::Tools => self
                .tool_page
                .map(|t| t.description().to_string())
                .unwrap_or_else(|| "空间整理、应用安装与设备工具".into()),
            Page::Connect => "管理 USB 通道、局域网无线配对与已信任设备".into(),
        }
    }

    pub(super) fn header(&self) -> El {
        let p = self.p();
        let mut actions = if self.selected.is_none() && self.page.needs_device() && self.page != Page::Overview {
            vec![
                self.button("hdr-rescan", "重新扫描", Action::Refresh, Btn::Secondary),
                self.button(
                    "hdr-connect",
                    "⇄ 连接设备",
                    Action::Page(Page::Connect),
                    Btn::Primary,
                ),
            ]
        } else {
            self.page_actions()
        };
        if !actions.is_empty() {
            actions.push(
                ConstrainedBox::new(
                    Container::new(Empty::new().finish())
                        .with_background_color(p.border)
                        .finish(),
                )
                .with_width(1.)
                .with_height(16.)
                .finish(),
            );
        }
        let controls = self.hstack(
            S2,
            vec![
                self.window_control_btn("min", "—", false, Action::MinimizeWindow),
                self.window_control_btn("max", "□", false, Action::MaximizeWindow),
                self.window_control_btn("close", "×", true, Action::CloseWindow),
            ],
        );

        let header_bg = if p.dark { p.sidebar } else { p.bg };
        ConstrainedBox::new(
            Container::new(
                self.between(
                    Clipped::new(
                        self.between(
                            Flex::row()
                                .with_main_axis_size(MainAxisSize::Max)
                                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                                .with_spacing(S3)
                                .with_child(self.line_w(
                                    if self.page == Page::Tools {
                                        self.tool_page.map(ToolPage::title).unwrap_or("小工具")
                                    } else {
                                        self.page.title()
                                    },
                                    TITLE,
                                    p.text,
                                    Weight::Semibold,
                                ))
                                .with_child(
                                    Expanded::new(
                                        1.,
                                        self.line(self.page_subtitle(), CAPTION, p.text2),
                                    )
                                    .finish(),
                                )
                                .finish(),
                            self.hstack(S2, actions),
                        ),
                    )
                    .finish(),
                    controls,
                ),
            )
            .with_horizontal_padding(S5)
            .with_vertical_padding(10.)
            .with_background_color(header_bg)
            .with_border(Border::bottom(1.).with_border_color(p.border))
            .with_corner_radius(CornerRadius::with_top_right(Radius::Pixels(12.)))
            .finish(),
        )
        .with_height(52.)
        .finish()
    }

    pub(super) fn status_bar(&self) -> El {
        let p = self.p();
        let transport = match &self.selected {
            Some(d) if is_wireless(&d.transport) => "无线",
            Some(_) => "USB",
            None => "ADB",
        };
        let detail = if self.status.is_empty() || self.status == "就绪" {
            if self.initializing { "正在检测设备" }
            else if self.selected.is_some() { "设备已连接" }
            else { "等待设备连接" }
        } else { &self.status };
        let left = self.lead(
            S2,
            self.dot(if self.error { p.danger } else if self.busy || self.initializing { p.warning }
                else if self.selected.is_some() { p.success } else { p.text3 }),
            self.line(format!("{transport} · {detail}"), CAPTION, if self.error { p.danger } else { p.text2 }),
        );
        let right = self.mono(format!("v{} · {}", stepx_companion::build_info::VERSION,
            stepx_companion::build_info::compiled_id()), CAPTION, p.text3);
        ConstrainedBox::new(
            Container::new(self.between(left, right))
                .with_horizontal_padding(S4)
                .with_vertical_padding(5.)
                .with_background_color(p.sidebar)
                .with_border(Border::top(1.).with_border_color(p.border))
                .with_corner_radius(CornerRadius::with_bottom(Radius::Pixels(12.)))
                .finish(),
        )
        .with_height(28.)
        .finish()
    }
}

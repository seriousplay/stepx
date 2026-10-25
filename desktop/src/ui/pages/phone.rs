use super::super::{
    icons::UiIcon, theme::*, widgets::*, Action, Companion, F_CONTACT_SEARCH, F_PHONE_DIAL,
};
use chrono::{Local, TimeZone};
use facloud_ui::{color::ColorU, elements::*, fonts::Weight, platform::Cursor};
use stepx_companion::device::ContactEntry;

const DIAL_KEYS: [(&str, &str); 12] = [
    ("1", ""),
    ("2", "ABC"),
    ("3", "DEF"),
    ("4", "GHI"),
    ("5", "JKL"),
    ("6", "MNO"),
    ("7", "PQRS"),
    ("8", "TUV"),
    ("9", "WXYZ"),
    ("*", ""),
    ("0", "+"),
    ("#", ""),
];

struct DisplayCallRow {
    name: String,
    num: String,
    outgoing: bool,
    missed: bool,
    label: String,
    time: String,
    sim: String,
    is_real: bool,
}

fn norm_phone(s: &str) -> String {
    stepx_companion::sms::normalize_address(s)
}

fn format_call_time(ms: i64) -> String {
    if ms <= 0 {
        return "时间未知".into();
    }
    if let Some(dt) = Local.timestamp_millis_opt(ms).single() {
        dt.format("%m-%d %H:%M").to_string()
    } else {
        "时间未知".into()
    }
}

fn contact_role_and_badge(name: &str, is_real: bool) -> (String, String) {
    let badge = name
        .trim()
        .chars()
        .next()
        .unwrap_or('联')
        .to_uppercase()
        .to_string();
    let role = if is_real {
        "手机实机通讯录 · 已同步".into()
    } else {
        "常用联系人".into()
    };
    (role, badge)
}

impl Companion {
    pub(in super::super) fn phone_actions(&self) -> Vec<El> {
        vec![
            self.button("ph-refresh", "同步记录", Action::ReadPhone, Btn::Secondary),
            self.button(
                "ph-new-contact",
                "新建联系人",
                Action::PhoneCreateContact,
                Btn::Secondary,
            ),
        ]
    }

    pub(in super::super) fn filtered_contacts(&self) -> Vec<ContactEntry> {
        let source = self.contacts.clone();
        let q = self.fields[F_CONTACT_SEARCH].text.trim().to_lowercase();
        source
            .into_iter()
            .filter(|c| {
                q.is_empty()
                    || c.name.to_lowercase().contains(&q)
                    || c.number.to_lowercase().contains(&q)
            })
            .collect()
    }

    pub(in super::super) fn phone_page(&self) -> El {
        let p = self.p();
        let all_contacts = self.filtered_contacts();

        // Only records returned by the phone belong in this list.
        let mut all_calls: Vec<DisplayCallRow> = Vec::new();
        for r in &self.call_logs {
            let resolved_name = if !r.name.trim().is_empty() {
                r.name.clone()
            } else if let Some(c) = self
                .contacts
                .iter()
                .find(|c| norm_phone(&c.number) == norm_phone(&r.number))
            {
                c.name.clone()
            } else if r.number.is_empty() {
                "隐藏号码".into()
            } else {
                r.number.clone()
            };
            all_calls.push(DisplayCallRow {
                name: resolved_name,
                num: r.number.clone(),
                outgoing: r.call_type == 2,
                missed: r.call_type == 3,
                label: format!("{} · {}", r.type_label(), r.format_duration()),
                time: format_call_time(r.date_ms),
                sim: "手机记录".into(),
                is_real: true,
            });
        }
        let missed_count = all_calls.iter().filter(|c| c.missed).count();
        let total_calls_count = all_calls.len();

        // 1. Col 1-4: Call History (`通话记录`)
        let filter_pills = self.hstack(
            4.,
            vec![
                self.button(
                    "ph-flt-all",
                    "全部",
                    Action::PhoneFilterMissed(false),
                    if !self.call_filter_missed {
                        Btn::Primary
                    } else {
                        Btn::Ghost
                    },
                ),
                self.button(
                    "ph-flt-missed",
                    if self.phone_loaded && self.call_logs_error.is_none() {
                        format!("未接 ({missed_count})")
                    } else {
                        "未接".into()
                    },
                    Action::PhoneFilterMissed(true),
                    if self.call_filter_missed {
                        Btn::Primary
                    } else {
                        Btn::Ghost
                    },
                ),
            ],
        );

        let col1_header = Container::new(self.between(
            self.hstack(
                8.,
                vec![
                        self.heading("通话记录"),
                        Container::new(self.line(
                            if self.phone_loaded && self.call_logs_error.is_none() {
                                format!("全部 {total_calls_count}")
                            } else {
                                "—".into()
                            },
                            CAPTION,
                            p.text2,
                        ))
                        .with_horizontal_padding(6.)
                        .with_vertical_padding(2.)
                        .with_background_color(if p.dark {
                            rgb(0x27, 0x27, 0x2A)
                        } else {
                            rgb(0xF4, 0xF4, 0xF5)
                        })
                        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
                        .finish(),
                    ],
            ),
            filter_pills,
        ))
        .with_uniform_padding(14.)
        .with_border(Border::bottom(1.).with_border_color(p.border))
        .finish();

        let mut call_rows: Vec<El> = Vec::new();
        for (i, c) in all_calls.iter().enumerate() {
            if self.call_filter_missed && !c.missed {
                continue;
            }
            if !call_rows.is_empty() {
                call_rows.push(self.divider());
            }
            let arrow = if c.outgoing { "↗" } else { "↙" };
            let arrow_color = if c.missed {
                p.danger
            } else if c.outgoing {
                p.success
            } else {
                p.text2
            };
            let sim_tag =
                Container::new(self.mono(&c.sim, 11., if c.is_real { p.success } else { p.text2 }))
                    .with_horizontal_padding(6.)
                    .with_vertical_padding(1.)
                    .with_background_color(if c.is_real {
                        p.success_soft
                    } else if p.dark {
                        rgb(0x27, 0x27, 0x2A)
                    } else {
                        rgb(0xF4, 0xF4, 0xF5)
                    })
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
                    .finish();

            let left_info = Flex::row()
                .with_spacing(10.)
                .with_cross_axis_alignment(CrossAxisAlignment::Start)
                .with_child(self.line_w(arrow, CAPTION, arrow_color, Weight::Bold))
                .with_child(
                    Flex::column()
                        .with_spacing(2.)
                        .with_cross_axis_alignment(CrossAxisAlignment::Start)
                        .with_child(self.hstack(
                            6.,
                            vec![
                                self.line_w(
                                    &c.name,
                                    CAPTION,
                                    if c.missed { p.danger } else { p.text },
                                    Weight::Semibold,
                                ),
                                sim_tag,
                            ],
                        ))
                        .with_child(self.mono(&c.num, CAPTION, p.text2))
                        .with_child(self.line(&c.label, CAPTION, p.text2))
                        .finish(),
                )
                .finish();

            let cb_state = self.mouse_state(format!("ph-cb:{i}"));
            let num_str = c.num.clone();
            let cb_btn = Hoverable::new(cb_state, move |mouse| {
                let hovered = mouse.is_mouse_over_element();
                ConstrainedBox::new(
                    Container::new(
                        Flex::row()
                            .with_main_axis_size(MainAxisSize::Max)
                            .with_main_axis_alignment(MainAxisAlignment::Center)
                            .with_cross_axis_alignment(CrossAxisAlignment::Center)
                            .with_child(self.ui_icon(UiIcon::Phone, 14., p.text2))
                            .finish(),
                    )
                    .with_background_color(if hovered { p.hover } else { p.surface })
                    .with_border(Border::all(1.).with_border_color(if hovered {
                        p.success
                    } else {
                        p.surface_border
                    }))
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(14.)))
                    .finish(),
                )
                .with_width(28.)
                .with_height(28.)
                .finish()
            })
            .with_cursor(Cursor::PointingHand)
            .on_click(move |ctx, _, _| {
                ctx.dispatch_typed_action(Action::PhoneCall(num_str.clone()))
            })
            .finish();

            let right_info = Flex::column()
                .with_spacing(6.)
                .with_cross_axis_alignment(CrossAxisAlignment::End)
                .with_child(self.mono(&c.time, CAPTION, p.text2))
                .with_child(cb_btn)
                .finish();

            call_rows.push(
                Container::new(self.between(left_info, right_info))
                    .with_horizontal_padding(14.)
                    .with_vertical_padding(10.)
                    .finish(),
            );
        }

        let col1_scroll = if let Some(error) = &self.call_logs_error {
            self.phone_empty(
                "通话记录读取失败",
                error,
                Some(("ph-retry-calls", "重新读取", Action::ReadPhone)),
            )
        } else if call_rows.is_empty() {
            self.phone_empty(
                if !self.phone_loaded {
                    "正在读取通话记录…"
                } else if self.call_filter_missed {
                    "暂无未接来电"
                } else {
                    "暂无通话记录"
                },
                if !self.phone_loaded {
                    "正在与手机同步"
                } else {
                    "这里显示手机中的通话记录"
                },
                None,
            )
        } else {
            self.pane_scroll(self.scroll.clone(), self.vstack(0., call_rows))
        };

        let col1_card = Container::new(
            Flex::column()
                .with_main_axis_size(MainAxisSize::Max)
                .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                .with_child(col1_header)
                .with_child(Expanded::new(1., col1_scroll).finish())
                .finish(),
        )
        .with_background_color(p.bg)
        .with_border(Border::all(1.).with_border_color(p.border))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
        .finish();

        // 2. Col 5-8: Contacts Directory & Contact Detail Card
        let col2_top = Container::new(self.vstack(
            8.,
            vec![
                self.between(
                    self.heading(if self.phone_loaded && self.contacts_error.is_none() {
                        format!("通讯录联系人 ({})", all_contacts.len())
                    } else {
                        "通讯录联系人".into()
                    }),
                    self.line("A–Z · 实机同步", MICRO, p.text2),
                ),
                self.input(
                    F_CONTACT_SEARCH,
                    "搜索姓名/手机号，或输入新联系人姓名…",
                    None,
                ),
            ],
        ))
        .with_uniform_padding(12.)
        .with_border(Border::bottom(1.).with_border_color(p.border))
        .finish();

        let fallback_contact = ContactEntry {
            name: "暂无联系人".into(),
            number: String::new(),
        };
        let active_idx = if all_contacts.is_empty() {
            0
        } else {
            self.selected_contact.min(all_contacts.len() - 1)
        };
        let sel = all_contacts.get(active_idx).unwrap_or(&fallback_contact);
        let sel_is_real = self
            .contacts
            .iter()
            .any(|c| norm_phone(&c.number) == norm_phone(&sel.number));
        let (sel_role, sel_badge) = contact_role_and_badge(&sel.name, sel_is_real);

        let sel_avatar = ConstrainedBox::new(
            Container::new(
                Flex::row()
                    .with_main_axis_size(MainAxisSize::Max)
                    .with_main_axis_alignment(MainAxisAlignment::Center)
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_child(self.line_w(&sel_badge, CAPTION, p.accent_fg, Weight::Semibold))
                    .finish(),
            )
            .with_background_color(p.accent)
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(18.)))
            .finish(),
        )
        .with_width(36.)
        .with_height(36.)
        .finish();

        let num_box_1 = Container::new(self.vstack(
            2.,
            vec![
                self.line("手机号码", CAPTION, p.text2),
                self.mono(&sel.number, CAPTION, p.text),
            ],
        ))
        .with_horizontal_padding(10.)
        .with_vertical_padding(6.)
        .with_background_color(p.bg)
        .with_border(Border::all(1.).with_border_color(p.border))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_CONTROL)))
        .finish();

        let num_box_2 = Container::new(self.vstack(
            2.,
            vec![
                self.line("来源状态", CAPTION, p.text2),
                self.mono(
                    if sel_is_real {
                        "手机实机通讯录"
                    } else {
                        "当前联系人"
                    },
                    CAPTION,
                    p.text,
                ),
            ],
        ))
        .with_horizontal_padding(10.)
        .with_vertical_padding(6.)
        .with_background_color(p.bg)
        .with_border(Border::all(1.).with_border_color(p.border))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_CONTROL)))
        .finish();

        let detail_box = Container::new(self.vstack(
            10.,
            vec![
                self.between(
                    self.hstack(
                        10.,
                        vec![
                            sel_avatar,
                            Flex::column()
                                .with_spacing(2.)
                                .with_cross_axis_alignment(CrossAxisAlignment::Start)
                                .with_child(self.line_w(
                                    &sel.name,
                                    CAPTION,
                                    p.text,
                                    Weight::Semibold,
                                ))
                                .with_child(self.line(&sel_role, CAPTION, p.text2))
                                .finish(),
                        ],
                    ),
                    self.hstack(
                        6.,
                        vec![
                            self.button(
                                "ph-sel-sms",
                                "短信",
                                Action::PhoneSmsTo(sel.number.clone()),
                                Btn::Secondary,
                            ),
                            self.button(
                                "ph-sel-call",
                                "呼叫",
                                Action::PhoneCall(sel.number.clone()),
                                Btn::Secondary,
                            ),
                        ],
                    ),
                ),
                Flex::row()
                    .with_main_axis_size(MainAxisSize::Max)
                    .with_spacing(8.)
                    .with_child(Expanded::new(1., num_box_1).finish())
                    .with_child(Expanded::new(1., num_box_2).finish())
                    .finish(),
            ],
        ))
        .with_uniform_padding(14.)
        .with_background_color(p.surface)
        .with_border(Border::bottom(1.).with_border_color(p.border))
        .finish();

        let mut contact_rows: Vec<El> = Vec::new();
        for (i, ct) in all_contacts.iter().enumerate() {
            if i > 0 {
                contact_rows.push(self.divider());
            }
            let active = i == active_idx;
            let is_real_ct = self
                .contacts
                .iter()
                .any(|c| norm_phone(&c.number) == norm_phone(&ct.number));
            let (role, badge) = contact_role_and_badge(&ct.name, is_real_ct);
            let badge_el = ConstrainedBox::new(
                Container::new(
                    Flex::row()
                        .with_main_axis_size(MainAxisSize::Max)
                        .with_main_axis_alignment(MainAxisAlignment::Center)
                        .with_cross_axis_alignment(CrossAxisAlignment::Center)
                        .with_child(self.line_w(
                            &badge,
                            CAPTION,
                            if p.dark {
                                p.text2
                            } else {
                                rgb(0x52, 0x52, 0x5B)
                            },
                            Weight::Semibold,
                        ))
                        .finish(),
                )
                .with_background_color(if p.dark {
                    rgb(0x27, 0x27, 0x2A)
                } else {
                    rgb(0xEF, 0xEF, 0xF2)
                })
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(14.)))
                .finish(),
            )
            .with_width(28.)
            .with_height(28.)
            .finish();

            let mut name_row = vec![self.line_w(&ct.name, CAPTION, p.text, Weight::Medium)];
            if is_real_ct {
                name_row.push(self.pill_badge("实机", p.success, p.success_soft));
            }

            let row_inner = self.between(
                self.hstack(
                    10.,
                    vec![
                        badge_el,
                        Flex::column()
                            .with_spacing(1.)
                            .with_cross_axis_alignment(CrossAxisAlignment::Start)
                            .with_child(self.hstack(6., name_row))
                            .with_child(self.line(&role, CAPTION, p.text2))
                            .finish(),
                    ],
                ),
                self.mono(&ct.number, CAPTION, p.text2),
            );
            let state = self.mouse_state(format!("ph-ct-row:{i}"));
            let row_el = Hoverable::new(state, move |mouse| {
                let hovered = mouse.is_mouse_over_element();
                let bg = if active {
                    if p.dark {
                        rgb(0x22, 0x22, 0x28)
                    } else {
                        rgb(0xF4, 0xF4, 0xF7)
                    }
                } else if hovered {
                    p.hover
                } else {
                    ColorU::transparent_black()
                };
                Container::new(row_inner)
                    .with_horizontal_padding(14.)
                    .with_vertical_padding(9.)
                    .with_background_color(bg)
                    .finish()
            })
            .with_cursor(Cursor::PointingHand)
            .on_click(move |ctx, _, _| ctx.dispatch_typed_action(Action::PhoneSelectContact(i)))
            .finish();
            contact_rows.push(row_el);
        }

        let col2_scroll = if let Some(error) = &self.contacts_error {
            self.phone_empty(
                "通讯录读取失败",
                error,
                Some(("ph-retry-contacts", "重新读取", Action::ReadPhone)),
            )
        } else if contact_rows.is_empty() {
            let searching = !self.fields[F_CONTACT_SEARCH].text.trim().is_empty();
            self.phone_empty(
                if !self.phone_loaded {
                    "正在读取通讯录…"
                } else if searching {
                    "未找到联系人"
                } else {
                    "暂无联系人"
                },
                if !self.phone_loaded {
                    "正在与手机同步"
                } else if searching {
                    "试试其他姓名或电话号码"
                } else {
                    "可在手机中添加，或新建联系人"
                },
                if !self.phone_loaded || searching {
                    None
                } else {
                    Some(("ph-empty-contact", "新建联系人", Action::PhoneCreateContact))
                },
            )
        } else {
            self.pane_scroll(self.scroll_aux.clone(), self.vstack(0., contact_rows))
        };
        let mut contact_content = Flex::column()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_child(col2_top);
        if !all_contacts.is_empty() {
            contact_content = contact_content.with_child(detail_box);
        }
        let col2_card = Container::new(
            contact_content
                .with_child(Expanded::new(1., col2_scroll).finish())
                .finish(),
        )
        .with_background_color(p.bg)
        .with_border(Border::all(1.).with_border_color(p.border))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
        .finish();

        // 3. Col 9-12: Desktop Dialpad & Active Call Controller
        let dial_number = self.fields[F_PHONE_DIAL].text.trim().to_string();

        let matched_contact_name = self
            .contacts
            .iter()
            .find(|c| norm_phone(&c.number) == norm_phone(&dial_number))
            .map(|c| format!("已匹配：{}", c.name))
            .unwrap_or_else(|| {
                if dial_number.is_empty() {
                    "输入号码或选择联系人".into()
                } else {
                    "未匹配联系人".into()
                }
            });

        let carrier_status = self
            .capabilities
            .as_ref()
            .map(|c| {
                c.subscriptions
                    .iter()
                    .find(|s| Some(s.id) == self.selected_call_sim)
                    .map(|s| format!("{} · 卡槽 {}", s.label, s.slot))
                    .unwrap_or_else(|| {
                        if c.subscriptions.is_empty() {
                            "未检测到通话 SIM".into()
                        } else {
                            "使用系统默认通话卡".into()
                        }
                    })
            })
            .unwrap_or_else(|| "SIM 信息未读取".into());

        let sim_choices: Vec<El> = self
            .capabilities
            .as_ref()
            .map(|caps| {
                caps.subscriptions
                    .iter()
                    .map(|sim| {
                        self.button(
                            format!("ph-sim-{}", sim.id),
                            format!("SIM {} · {}", sim.slot, sim.label),
                            Action::PhoneSelectSim(sim.id),
                            if self.selected_call_sim == Some(sim.id) {
                                Btn::Secondary
                            } else {
                                Btn::Ghost
                            },
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();

        let num_display_box = Container::new(
            Flex::column()
                .with_spacing(4.)
                .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                .with_child(self.between(
                    self.line("桌面拨号", CAPTION, p.text2),
                    self.hstack(
                        8.,
                        vec![
                            self.link_action("ph-dial-bksp", "退格", Action::PhoneDialBackspace),
                            self.link_action("ph-dial-clr", "清空", Action::PhoneDialClear),
                        ],
                    ),
                ))
                .with_child(self.input(F_PHONE_DIAL, "输入电话号码…", None))
                .with_child(self.between(
                    self.line(&carrier_status, MICRO, p.text2),
                    self.line_w(&matched_contact_name, MICRO, p.text2, Weight::Medium),
                ))
                .with_child(self.vstack(S1, sim_choices))
                .finish(),
        )
        .with_uniform_padding(12.)
        .with_background_color(p.surface)
        .with_border(Border::all(1.).with_border_color(p.surface_border))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
        .finish();

        let mut keypad_rows = Vec::with_capacity(4);
        for chunk in DIAL_KEYS.chunks(3) {
            let mut row_items = Vec::with_capacity(3);
            for &(digit, sub) in chunk {
                let state = self.mouse_state(format!("ph-pad:{digit}"));
                let d_str = digit.to_string();
                let mut col_children = vec![self.mono(digit, 14., p.text)];
                if !sub.is_empty() {
                    col_children.push(self.mono(sub, 10., p.text2));
                }
                let inner_col = Flex::column()
                    .with_main_axis_size(MainAxisSize::Max)
                    .with_main_axis_alignment(MainAxisAlignment::Center)
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_spacing(2.)
                    .with_children(col_children)
                    .finish();
                let key_btn = Hoverable::new(state, move |mouse| {
                    let hovered = mouse.is_mouse_over_element();
                    ConstrainedBox::new(
                        Container::new(inner_col)
                            .with_background_color(if hovered { p.hover } else { p.surface })
                            .with_border(Border::all(1.).with_border_color(if hovered {
                                p.border_strong
                            } else {
                                p.surface_border
                            }))
                            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
                            .finish(),
                    )
                    .with_height(44.)
                    .finish()
                })
                .with_cursor(Cursor::PointingHand)
                .on_click(move |ctx, _, _| {
                    ctx.dispatch_typed_action(Action::PhoneDialAppend(d_str.clone()))
                })
                .finish();
                row_items.push(Expanded::new(1., key_btn).finish());
            }
            keypad_rows.push(
                Flex::row()
                    .with_main_axis_size(MainAxisSize::Max)
                    .with_spacing(8.)
                    .with_children(row_items)
                    .finish(),
            );
        }

        // Put actions below the status text so a narrow dialpad never pushes
        // the contact button outside its card. Long names and numbers may wrap.
        let live_call_banner = if let Some(active_num) = &self.active_call_number {
            let peer_label = self
                .contacts
                .iter()
                .find(|c| norm_phone(&c.number) == norm_phone(active_num))
                .map(|c| format!("{} · {}", c.name, active_num))
                .unwrap_or_else(|| active_num.clone());
            Container::new(self.vstack(
                S2,
                vec![
                    self.hstack(
                        S2,
                        vec![
                            self.dot(p.success),
                            self.line_w("已提交呼叫", CAPTION, p.text, Weight::Medium),
                        ],
                    ),
                    self.text(
                        format!("{peer_label} · 通话状态以手机为准"),
                        CAPTION,
                        p.text2,
                    ),
                    self.wrap(vec![
                        self.button(
                            "ph-mute",
                            "短信",
                            Action::PhoneSmsTo(active_num.clone()),
                            Btn::Secondary,
                        ),
                        self.button("ph-hangup", "挂断", Action::PhoneHangup, Btn::DangerSolid),
                    ]),
                ],
            ))
            .with_horizontal_padding(S3)
            .with_vertical_padding(S2)
            .with_background_color(p.success_soft)
            .with_border(Border::all(1.).with_border_color(rgba(0x10, 0xB9, 0x81, 76)))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
            .finish()
        } else {
            Container::new(self.vstack(
                S2,
                vec![
                    self.hstack(
                        S2,
                        vec![
                            self.dot(p.accent),
                            self.line_w("手机拨号", CAPTION, p.text, Weight::Medium),
                        ],
                    ),
                    self.text("确认后将直接从手机呼叫", MICRO, p.text2),
                    self.wrap(vec![
                        self.button(
                            "ph-mute",
                            "短信",
                            Action::PhoneSmsTo(dial_number.clone()),
                            Btn::Secondary,
                        ),
                        self.button(
                            "ph-save-ct",
                            "存为联系人",
                            Action::PhoneCreateContact,
                            Btn::Secondary,
                        ),
                    ]),
                ],
            ))
            .with_horizontal_padding(S3)
            .with_vertical_padding(S2)
            .with_background_color(p.surface)
            .with_border(Border::all(1.).with_border_color(p.surface_border))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
            .finish()
        };

        let sim_call_row = if let Some(plan) = &self.pending_call {
            Container::new(self.vstack(
                S2,
                vec![
                    self.line_w(
                        format!("呼叫 {}", plan.number),
                        BODY,
                        p.text,
                        Weight::Medium,
                    ),
                    self.text(
                        format!("{} · 确认后由手机发起通话", plan.subscription_label),
                        CAPTION,
                        p.text2,
                    ),
                    self.wrap(vec![
                        self.button(
                            "ph-cancel-call",
                            "取消",
                            Action::PhoneCancelCall,
                            Btn::Secondary,
                        ),
                        self.button(
                            "ph-confirm-call",
                            "确认呼叫",
                            Action::PhoneConfirmCall,
                            Btn::Primary,
                        ),
                    ]),
                ],
            ))
            .with_uniform_padding(S3)
            .with_background_color(p.surface)
            .with_border(Border::all(1.).with_border_color(p.border_strong))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
            .finish()
        } else {
            self.button(
                "ph-call",
                "呼叫",
                Action::PhoneCall(dial_number),
                Btn::Primary,
            )
        };

        let col3_card = Container::new(
            Flex::column()
                .with_main_axis_size(MainAxisSize::Max)
                .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
                .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                .with_child(num_display_box)
                .with_child(self.vstack(8., keypad_rows))
                .with_child(self.vstack(8., vec![live_call_banner, sim_call_row]))
                .finish(),
        )
        .with_uniform_padding(S4)
        .with_background_color(p.bg)
        .with_border(Border::all(1.).with_border_color(p.border))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
        .finish();

        let columns = Container::new(
            Flex::row()
                .with_main_axis_size(MainAxisSize::Max)
                .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                .with_spacing(S4)
                .with_child(Expanded::new(4., col1_card).finish())
                .with_child(Expanded::new(4., col2_card).finish())
                .with_child(Expanded::new(4., col3_card).finish())
                .finish(),
        )
        .with_uniform_padding(S5)
        .finish();

        Flex::column()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_child(self.comm_mode_bar())
            .with_child(Expanded::new(1., columns).finish())
            .finish()
    }

    fn phone_empty(
        &self,
        title: &str,
        description: &str,
        action: Option<(&str, &str, Action)>,
    ) -> El {
        Flex::column()
            .with_main_axis_size(MainAxisSize::Max)
            .with_main_axis_alignment(MainAxisAlignment::Center)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_child(self.empty(
                title,
                description,
                action.map(|(id, label, action)| self.button(id, label, action, Btn::Secondary)),
            ))
            .finish()
    }
}

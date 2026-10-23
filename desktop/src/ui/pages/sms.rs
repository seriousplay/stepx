use super::super::{
    theme::*, widgets::*, Action, Companion, Page, SmsFilter, F_SMS_BODY, F_SMS_SEARCH, F_SMS_TO,
};
use chrono::{Local, TimeZone};
use facloud_ui::{color::ColorU, elements::*, fonts::Weight, platform::Cursor};

struct DisplaySmsThread {
    name: String,
    num: String,
    time: String,
    preview: String,
    unread: bool,
    badge: String,
    count: usize,
}

fn norm_addr(s: &str) -> String {
    stepx_companion::sms::normalize_address(s)
}

fn format_sms_time(ms: i64) -> String {
    if ms <= 0 {
        return "时间未知".into();
    }
    if let Some(dt) = Local.timestamp_millis_opt(ms).single() {
        dt.format("%m-%d %H:%M").to_string()
    } else {
        "时间未知".into()
    }
}

fn resolve_sender_info(
    addr: &str,
    contacts: &[stepx_companion::device::ContactEntry],
) -> (String, String) {
    let norm = norm_addr(addr);
    if let Some(c) = contacts.iter().find(|c| norm_addr(&c.number) == norm) {
        let badge = c.name.chars().next().unwrap_or('联').to_string();
        return (c.name.clone(), badge);
    }
    let label = if addr.is_empty() {
        "未知号码"
    } else {
        addr
    };
    (
        label.into(),
        label.chars().next().unwrap_or('#').to_string(),
    )
}

impl Companion {
    pub(in super::super) fn sms_actions(&self) -> Vec<El> {
        vec![
            self.button(
                "sms-open-phone",
                "📱 在手机打开短信",
                Action::OpenSms,
                Btn::Secondary,
            ),
            self.button("sms-sync", "刷新列表", Action::ReadSms, Btn::Secondary),
            self.button("sms-new", "＋ 新建短信", Action::NewSms, Btn::Primary),
        ]
    }

    pub(in super::super) fn sms_page(&self) -> El {
        let p = self.p();
        if !self.demo_mode && self.selected.is_none() {
            return self.empty(
                "连接手机后查看短信",
                "连接后将读取手机中的短信记录。",
                Some(self.button(
                    "sms-connect",
                    "连接设备",
                    Action::Page(Page::Connect),
                    Btn::Primary,
                )),
            );
        }
        let loaded = self.sms_loaded || self.demo_mode;
        let filtered =
            self.sms_filter != SmsFilter::All || !self.fields[F_SMS_SEARCH].text.trim().is_empty();

        // Build threads exclusively from the phone provider snapshot.
        let mut display_threads: Vec<DisplaySmsThread> = Vec::new();
        for rt in self.sms_threads() {
            let (name, badge) = resolve_sender_info(&rt.address, &self.contacts);
            let one_line: String = rt
                .last_body
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            let preview = if one_line.chars().count() > 38 {
                format!("{}…", one_line.chars().take(38).collect::<String>())
            } else {
                one_line
            };
            display_threads.push(DisplaySmsThread {
                name,
                num: rt.address,
                time: format_sms_time(rt.last_ms),
                preview,
                unread: rt.unread,
                badge,
                count: rt.count,
            });
        }

        let is_new_sms = self.selected_thread.as_deref() == Some("__new__");
        let active_addr = if is_new_sms {
            "__new__".to_string()
        } else if let Some(sel) = &self.selected_thread {
            sel.clone()
        } else if let Some(first) = display_threads.first() {
            first.num.clone()
        } else {
            String::new()
        };

        let filter_bar = self.segmented(
            "sms-filter-seg",
            vec![
                (
                    if loaded {
                        format!("全部 {}", self.sms.len())
                    } else {
                        "全部".into()
                    },
                    self.sms_filter == SmsFilter::All,
                    Action::SmsFilter(SmsFilter::All),
                ),
                (
                    if loaded {
                        format!("收件箱 {}", self.sms.iter().filter(|m| m.kind == 1).count())
                    } else {
                        "收件箱".into()
                    },
                    self.sms_filter == SmsFilter::Incoming,
                    Action::SmsFilter(SmsFilter::Incoming),
                ),
                (
                    if loaded {
                        format!("已发送 {}", self.sms.iter().filter(|m| m.kind == 2).count())
                    } else {
                        "已发送".into()
                    },
                    self.sms_filter == SmsFilter::Outgoing,
                    Action::SmsFilter(SmsFilter::Outgoing),
                ),
            ],
        );

        let mut thread_rows: Vec<El> = Vec::new();
        for (i, t) in display_threads.iter().enumerate() {
            if !thread_rows.is_empty() {
                thread_rows.push(self.divider());
            }
            let active = !is_new_sms && norm_addr(&active_addr) == norm_addr(&t.num);
            let avatar_bg = if active {
                p.accent
            } else if p.dark {
                rgb(0x27, 0x27, 0x2A)
            } else {
                rgb(0xEF, 0xEF, 0xF2)
            };
            let avatar_fg = if active {
                p.accent_fg
            } else if p.dark {
                p.text2
            } else {
                rgb(0x52, 0x52, 0x5B)
            };
            let avatar = ConstrainedBox::new(
                Container::new(
                    Flex::row()
                        .with_main_axis_size(MainAxisSize::Max)
                        .with_main_axis_alignment(MainAxisAlignment::Center)
                        .with_cross_axis_alignment(CrossAxisAlignment::Center)
                        .with_child(self.line_w(&t.badge, CAPTION, avatar_fg, Weight::Semibold))
                        .finish(),
                )
                .with_background_color(avatar_bg)
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(18.)))
                .finish(),
            )
            .with_width(36.)
            .with_height(36.)
            .finish();

            let preview_line = self.line_w(
                &t.preview,
                CAPTION,
                if t.unread { p.text } else { p.text2 },
                if t.unread {
                    Weight::Medium
                } else {
                    Weight::Normal
                },
            );
            let bottom_row = if t.unread {
                self.between(preview_line, self.dot(rgb(0x25, 0x63, 0xEB)))
            } else {
                preview_line
            };

            let title_items = vec![
                self.line_w(&t.name, CAPTION, p.text, Weight::Semibold),
                self.pill_badge(
                    if filtered {
                        format!("匹配 {} 条", t.count)
                    } else {
                        format!("{} 条", t.count)
                    },
                    p.text2,
                    p.tag_bg,
                ),
            ];

            let right_col = Flex::column()
                .with_spacing(3.)
                .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                .with_child(self.between(
                    self.hstack(6., title_items),
                    self.mono(&t.time, CAPTION, p.text2),
                ))
                .with_child(self.mono(&t.num, CAPTION, p.text2))
                .with_child(bottom_row)
                .finish();

            let row_inner = Flex::row()
                .with_main_axis_size(MainAxisSize::Max)
                .with_cross_axis_alignment(CrossAxisAlignment::Start)
                .with_spacing(12.)
                .with_child(avatar)
                .with_child(Expanded::new(1., right_col).finish())
                .finish();

            let state = self.mouse_state(format!("sms-th:{i}"));
            let num_str = t.num.clone();
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
                    .with_uniform_padding(12.)
                    .with_background_color(bg)
                    .finish()
            })
            .with_cursor(Cursor::PointingHand)
            .on_click(move |ctx, _, _| {
                ctx.dispatch_typed_action(Action::SelectSmsThread(Some(num_str.clone())))
            })
            .finish();
            thread_rows.push(row_el);
        }

        if let Some(error) = &self.sms_error {
            thread_rows.insert(
                0,
                self.empty(
                    "短信读取失败",
                    error.clone(),
                    Some(self.button(
                        "sms-read-retry",
                        "重试读取",
                        Action::ReadSms,
                        Btn::Secondary,
                    )),
                ),
            );
        } else if !loaded {
            thread_rows.push(self.empty(
                if self.busy {
                    "正在读取手机短信…"
                } else {
                    "尚未读取短信"
                },
                "读取完成后显示会话与短信数量。",
                if self.busy {
                    None
                } else {
                    Some(self.button(
                        "sms-first-read",
                        "读取短信",
                        Action::ReadSms,
                        Btn::Secondary,
                    ))
                },
            ));
        } else if thread_rows.is_empty() {
            thread_rows.push(self.empty(
                if self.sms.is_empty() {
                    "手机暂无短信"
                } else {
                    "没有匹配的短信"
                },
                if self.sms.is_empty() {
                    "收到或发送短信后，刷新即可查看。"
                } else {
                    "请调整搜索内容或短信筛选条件。"
                },
                None,
            ));
        }
        let left_scroll = self.pane_scroll(self.scroll.clone(), self.vstack(0., thread_rows));

        let left_pane = self.fixed_w(
            LIST_PANE_W,
            Container::new(
                Flex::column()
                    .with_main_axis_size(MainAxisSize::Max)
                    .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                    .with_child(
                        Container::new(self.vstack(
                            8.,
                            vec![
                                self.input(F_SMS_SEARCH, "搜索联系人、号码、验证码或快递码…", None),
                                filter_bar,
                                self.line(
                                    if loaded {
                                        format!(
                                            "{} 个会话 · {} 条{}短信",
                                            display_threads.len(),
                                            display_threads.iter().map(|t| t.count).sum::<usize>(),
                                            if filtered { "匹配" } else { "" }
                                        )
                                    } else {
                                        "短信数量待读取".into()
                                    },
                                    11.,
                                    p.text3,
                                ),
                            ],
                        ))
                        .with_uniform_padding(12.)
                        .with_border(Border::bottom(1.).with_border_color(p.border))
                        .finish(),
                    )
                    .with_child(Expanded::new(1., left_scroll).finish())
                    .finish(),
            )
            .with_background_color(if p.dark { p.sidebar } else { p.bg })
            .with_border(Border::right(1.).with_border_color(p.border))
            .finish(),
        );

        let selected_thread_info = display_threads
            .iter()
            .find(|t| norm_addr(&t.num) == norm_addr(&active_addr));

        let (peer_name, peer_num) = if is_new_sms {
            let typed = self.fields[F_SMS_TO].text.trim();
            (
                "新建短信会话".to_string(),
                if typed.is_empty() {
                    String::new()
                } else {
                    typed.to_string()
                },
            )
        } else if let Some(info) = selected_thread_info {
            (info.name.clone(), info.num.clone())
        } else if active_addr.is_empty() {
            ("短信会话".into(), String::new())
        } else {
            let (n, _) = resolve_sender_info(&active_addr, &self.contacts);
            (n, active_addr.clone())
        };

        let sims = self
            .capabilities
            .as_ref()
            .map(|c| c.subscriptions.as_slice())
            .unwrap_or(&[]);
        let selected_sim = self
            .selected_sim
            .or_else(|| self.capabilities.as_ref().map(|c| c.default_subscription));
        let real_sim_badge = sims
            .iter()
            .find(|s| Some(s.id) == selected_sim)
            .map(|s| format!("SIM {} · {}", s.slot, s.label))
            .unwrap_or_else(|| "请选择可用 SIM".into());
        let can_send = self.capabilities.as_ref().is_some_and(|c| c.send_sms)
            && sims.iter().any(|s| Some(s.id) == selected_sim);
        let has_recipient = !peer_num.trim().is_empty();
        let mut peer_labels = vec![self.line_w(&peer_name, CAPTION, p.text, Weight::Semibold)];
        if has_recipient {
            if peer_name != peer_num {
                peer_labels.push(self.mono(&peer_num, CAPTION, p.text2));
            }
            peer_labels.push(self.tag(&real_sim_badge, p.text2));
        }
        let peer_actions = if has_recipient {
            vec![
                self.button(
                    "sms-call-peer",
                    "☎ 拨号呼叫",
                    Action::PhoneCall(peer_num.clone()),
                    Btn::Secondary,
                ),
                self.button(
                    "sms-copy-num",
                    "复制号码",
                    Action::CopyText(peer_num.clone(), format!("已复制号码 {peer_num}")),
                    Btn::Secondary,
                ),
            ]
        } else {
            Vec::new()
        };

        // Right Conversation Detail Pane
        let subheader = ConstrainedBox::new(
            Container::new(
                self.between(self.hstack(10., peer_labels), self.hstack(8., peer_actions)),
            )
            .with_horizontal_padding(S5)
            .with_vertical_padding(8.)
            .with_background_color(p.bg)
            .with_border(Border::bottom(1.).with_border_color(p.border))
            .finish(),
        )
        .with_height(48.)
        .finish();

        let date_pill = |label: &str| -> El {
            Flex::row()
                .with_main_axis_size(MainAxisSize::Max)
                .with_main_axis_alignment(MainAxisAlignment::Center)
                .with_child(
                    Container::new(self.line(label, CAPTION, p.text2))
                        .with_horizontal_padding(10.)
                        .with_vertical_padding(2.)
                        .with_background_color(if p.dark {
                            rgb(0x22, 0x22, 0x27)
                        } else {
                            rgb(0xEF, 0xEF, 0xF2)
                        })
                        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(10.)))
                        .finish(),
                )
                .finish()
        };

        let incoming_bubble = |txt: &str, meta: &str| -> El {
            let b = Container::new(self.text(txt.to_string(), CAPTION, p.text))
                .with_horizontal_padding(16.)
                .with_vertical_padding(10.)
                .with_background_color(p.bg)
                .with_border(Border::all(1.).with_border_color(p.border))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(16.)))
                .finish();
            Flex::column()
                .with_spacing(4.)
                .with_cross_axis_alignment(CrossAxisAlignment::Start)
                .with_child(ConstrainedBox::new(b).with_max_width(540.).finish())
                .with_child(
                    Container::new(self.mono(meta.to_string(), CAPTION, p.text2))
                        .with_padding_left(4.)
                        .finish(),
                )
                .finish()
        };

        let outgoing_bubble = |txt: &str, meta: &str| -> El {
            let b = Container::new(self.text(txt.to_string(), CAPTION, p.accent_fg))
                .with_horizontal_padding(16.)
                .with_vertical_padding(10.)
                .with_background_color(p.accent)
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(16.)))
                .finish();
            Flex::row()
                .with_main_axis_size(MainAxisSize::Max)
                .with_main_axis_alignment(MainAxisAlignment::End)
                .with_child(
                    Flex::column()
                        .with_spacing(4.)
                        .with_cross_axis_alignment(CrossAxisAlignment::End)
                        .with_child(ConstrainedBox::new(b).with_max_width(540.).finish())
                        .with_child(
                            Container::new(self.mono(meta.to_string(), CAPTION, p.text2))
                                .with_padding_right(4.)
                                .finish(),
                        )
                        .finish(),
                )
                .finish()
        };

        let mut timeline_children: Vec<El> = Vec::new();
        let real_msgs = self.thread_messages(&active_addr);

        if is_new_sms {
            timeline_children.push(date_pill("新建短信 · 输入号码与正文后确认发送"));
        } else if !active_addr.is_empty() && real_msgs.is_empty() {
            timeline_children.push(date_pill("暂无短信，请选择会话或新建短信"));
        }

        if !real_msgs.is_empty() {
            timeline_children.push(date_pill(&format!("此会话共 {} 条短信", real_msgs.len())));
            for m in real_msgs {
                let ts = format_sms_time(m.date_ms);
                let sim = sims
                    .iter()
                    .find(|s| Some(s.id) == m.subscription)
                    .map(|s| format!("SIM {} · {}", s.slot, s.label))
                    .unwrap_or_else(|| "SIM 未知".into());
                let meta = format!("{ts} · {} · {sim}", m.status_label());
                if m.outgoing {
                    timeline_children.push(outgoing_bubble(&m.body, &meta));
                } else {
                    timeline_children.push(incoming_bubble(&m.body, &meta));
                }
            }
        }

        let timeline = if active_addr.is_empty() && !is_new_sms {
            Align::new(self.empty(
                if self.sms_error.is_some() {
                    "短信暂时无法读取"
                } else if !loaded {
                    "等待短信同步"
                } else if self.sms.is_empty() {
                    "手机暂无短信"
                } else {
                    "请选择一个会话"
                },
                if self.sms_error.is_some() {
                    "请重试读取手机短信。"
                } else if !loaded {
                    "同步完成后可在左侧选择会话。"
                } else {
                    "也可以点击“新建短信”填写收件人与内容。"
                },
                None,
            ))
            .finish()
        } else {
            let timeline_inner = Container::new(self.vstack(14., timeline_children))
                .with_uniform_padding(S4)
                .finish();
            self.pane_scroll(self.scroll_aux.clone(), timeline_inner)
        };

        // Inline Send Safety Confirmation Bar + Composer
        let target_num = if self.fields[F_SMS_TO].text.trim().is_empty() {
            if is_new_sms {
                "待输入号码".to_string()
            } else {
                peer_num.clone()
            }
        } else {
            self.fields[F_SMS_TO].text.trim().to_string()
        };
        let sim_label = &real_sim_badge;
        let next_sim = match sims.iter().position(|s| Some(s.id) == selected_sim) {
            Some(i) => sims.get((i + 1) % sims.len()).map(|s| s.id),
            None => sims.first().map(|s| s.id),
        };

        let safety_banner = if let Some(plan) = &self.pending_sms_plan {
            Container::new(self.between(
                self.hstack(
                    8.,
                    vec![
                        self.dot(p.warning),
                        self.line_w("敏感操作二次确认：", CAPTION, p.text, Weight::Semibold),
                        self.line("将通过", CAPTION, p.text2),
                        self.line_w(&plan.subscription_label, CAPTION, p.text, Weight::Semibold),
                        self.line("发送至", CAPTION, p.text2),
                        self.mono(&plan.number, CAPTION, p.text),
                        self.pill_badge(format!("共 {} 段", plan.parts), p.warning, p.warning_soft),
                    ],
                ),
                self.hstack(
                    8.,
                    vec![
                        self.button(
                            "sms-draft-phone",
                            "填入手机草稿",
                            Action::DraftSms,
                            Btn::Secondary,
                        ),
                        self.button(
                            "sms-cancel",
                            "取消",
                            Action::CancelDirectSms,
                            Btn::Secondary,
                        ),
                        self.button(
                            "sms-confirm",
                            "✓ 确认立即发送",
                            Action::ConfirmDirectSms,
                            Btn::Primary,
                        ),
                    ],
                ),
            ))
            .with_horizontal_padding(14.)
            .with_vertical_padding(8.)
            .with_background_color(p.warning_soft)
            .with_border(Border::all(1.).with_border_color(p.warning))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
            .finish()
        } else {
            Container::new(self.between(
                self.hstack(
                    8.,
                    vec![
                        self.dot(if can_send { p.success } else { p.text3 }),
                        self.line_w(
                            if can_send {
                                "短信发送"
                            } else {
                                "暂不可发送"
                            },
                            CAPTION,
                            p.text,
                            Weight::Medium,
                        ),
                        self.line(
                            if can_send {
                                format!("{real_sim_badge} · 收件人 {target_num}")
                            } else if self.capabilities.is_none() {
                                "正在确认手机发送能力".into()
                            } else {
                                "请确认手机短信权限与可用 SIM 卡".into()
                            },
                            CAPTION,
                            p.text2,
                        ),
                    ],
                ),
                self.hstack(
                    8.,
                    vec![
                        self.button(
                            "sms-draft-phone",
                            "填入手机草稿",
                            Action::DraftSms,
                            Btn::Secondary,
                        ),
                        self.button(
                            "sms-confirm",
                            "准备发送",
                            Action::PrepareDirectSms,
                            Btn::Secondary,
                        ),
                    ],
                ),
            ))
            .with_horizontal_padding(14.)
            .with_vertical_padding(8.)
            .with_background_color(if p.dark {
                rgb(0x22, 0x22, 0x28)
            } else {
                ColorU::white()
            })
            .with_border(Border::all(1.).with_border_color(if p.dark {
                rgb(0x3F, 0x3F, 0x46)
            } else {
                rgb(0xE4, 0xE4, 0xE7)
            }))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
            .finish()
        };

        let sim_btn = self.button(
            "sms-sim-switch",
            format!("{sim_label} · 切换"),
            Action::SelectSim(next_sim),
            Btn::Secondary,
        );

        let composer_box = Container::new(
            Flex::row()
                .with_main_axis_size(MainAxisSize::Max)
                .with_spacing(8.)
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_child(sim_btn)
                .with_child(self.input(
                    F_SMS_TO,
                    if is_new_sms {
                        "输入收件人号码…"
                    } else {
                        &peer_num
                    },
                    Some(150.),
                ))
                .with_child(
                    Expanded::new(
                        1.,
                        self.input(F_SMS_BODY, "输入短信内容，按 Enter 或点击发送…", None),
                    )
                    .finish(),
                )
                .with_child(self.button(
                    "sms-send",
                    if self.pending_sms_plan.is_some() {
                        "确认发送"
                    } else {
                        "发送"
                    },
                    if self.pending_sms_plan.is_some() {
                        Action::ConfirmDirectSms
                    } else {
                        Action::PrepareDirectSms
                    },
                    Btn::Primary,
                ))
                .finish(),
        )
        .with_uniform_padding(8.)
        .with_background_color(p.bg)
        .with_border(Border::all(1.).with_border_color(p.border))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
        .finish();

        let mut composer_parts = vec![safety_banner];
        if let Some(status) = &self.last_sms_status {
            composer_parts.push(self.between(
                self.text(status.summary_label(), CAPTION, p.text2),
                self.button(
                    "sms-check-status",
                    "查询发送状态",
                    Action::CheckSmsStatus(status.id.clone()),
                    Btn::Secondary,
                ),
            ));
        }
        composer_parts.push(composer_box);
        let bottom_area = Container::new(self.vstack(8., composer_parts))
            .with_horizontal_padding(S4)
            .with_padding_bottom(S4)
            .finish();

        let mut right_pane = Flex::column()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_child(subheader)
            .with_child(Expanded::new(1., timeline).finish());
        if has_recipient || is_new_sms {
            right_pane = right_pane.with_child(bottom_area);
        }
        let right_pane = right_pane.finish();

        Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_child(left_pane)
            .with_child(Expanded::new(1., right_pane).finish())
            .finish()
    }
}

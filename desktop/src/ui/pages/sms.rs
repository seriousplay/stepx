use super::super::{
    theme::*, widgets::*, Action, Companion, Page, PushAction, SmsFilter, F_IM_REPLY, F_SMS_BODY,
    F_SMS_SEARCH, F_SMS_TO,
};
use chrono::{Local, TimeZone};
use facloud_ui::{color::ColorU, elements::*, fonts::Weight, platform::Cursor};
use stepx_companion::push::{
    demo_push_snapshot, extract_first_url, extract_otp_code, ImChannel, ImThread, PushSnapshot,
    ReplyMode,
};

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

fn im_avatar_style(package_name: &str, app_name: &str) -> (String, ColorU, ColorU) {
    match ImChannel::from_package(package_name) {
        ImChannel::WeChat => ("微".into(), rgb(0x07, 0xC1, 0x60), ColorU::white()),
        ImChannel::WeCom => ("企".into(), rgb(0x2B, 0x6C, 0xB0), ColorU::white()),
        ImChannel::DingTalk => ("钉".into(), rgb(0x00, 0x89, 0xFF), ColorU::white()),
        ImChannel::Feishu => ("飞".into(), rgb(0x33, 0x70, 0xFF), ColorU::white()),
        ImChannel::QqTim => ("Q".into(), rgb(0x02, 0x84, 0xC7), ColorU::white()),
        ImChannel::Sms => ("短".into(), rgb(0x10, 0xB9, 0x81), ColorU::white()),
        ImChannel::SocialDm => ("私".into(), rgb(0xE1, 0x1D, 0x48), ColorU::white()),
        _ => {
            if package_name.contains("telegram") {
                ("TG".into(), rgb(0x02, 0x84, 0xC7), ColorU::white())
            } else if package_name.contains("whatsapp") {
                ("WA".into(), rgb(0x16, 0xA3, 0x4A), ColorU::white())
            } else {
                let first = app_name
                    .trim()
                    .chars()
                    .next()
                    .unwrap_or('聊')
                    .to_uppercase()
                    .to_string();
                (first, rgb(0x63, 0x66, 0xF1), ColorU::white())
            }
        }
    }
}

fn reply_mode_short_label(mode: ReplyMode) -> &'static str {
    match mode {
        ReplyMode::Auto => "自动择优",
        ReplyMode::RemoteInputOnly => "仅静默秒回",
        ReplyMode::UiBridgeFallback => "看屏投递",
    }
}

impl Companion {
    pub(in super::super) fn sms_actions(&self) -> Vec<El> {
        vec![
            self.button(
                "im-sync-chat-win",
                "同步聊天屏",
                Action::Push(PushAction::SyncActivePhoneChatWindow),
                Btn::Secondary,
            ),
            self.button(
                "im-refresh-push",
                "刷新",
                Action::Push(PushAction::Refresh),
                Btn::Secondary,
            ),
            self.button("sms-new", "新建短信", Action::NewSms, Btn::Primary),
        ]
    }

    pub(in super::super) fn comm_mode_bar(&self) -> El {
        let p = self.p();
        let im_unread: usize = self
            .push
            .snapshot
            .threads
            .iter()
            .map(|t| t.unread_count)
            .sum();
        let push_unread: usize = self
            .push
            .snapshot
            .notifications
            .iter()
            .filter(|n| !n.dismissed && !n.read && !n.is_ongoing)
            .count();
        let missed_calls: usize = self.call_logs.iter().filter(|r| r.call_type == 3).count();

        let mode_seg = self.segmented(
            "comm-mode-bar",
            vec![
                (
                    "即时消息与短信".into(),
                    self.page == Page::Sms,
                    Action::Page(Page::Sms),
                ),
                (
                    "手机推送通知".into(),
                    self.page == Page::Notifications,
                    Action::Page(Page::Notifications),
                ),
                (
                    "电话与通讯录".into(),
                    self.page == Page::Phone,
                    Action::Page(Page::Phone),
                ),
            ],
        );

        let mut right_badges: Vec<El> = Vec::new();
        if im_unread > 0 {
            right_badges.push(self.pill_badge(
                format!("{im_unread} 未读会话"),
                p.success_text,
                p.success_soft,
            ));
        }
        if push_unread > 0 {
            right_badges.push(self.pill_badge(
                format!("{push_unread} 未读推送"),
                p.warning,
                p.warning_soft,
            ));
        }
        if missed_calls > 0 {
            right_badges.push(self.pill_badge(
                format!("{missed_calls} 未接来电"),
                p.danger,
                p.danger_soft,
            ));
        }
        right_badges.push(self.line(
            "消息与通知 · 即时消息与短信 · 手机推送通知 · 电话与通讯录",
            MICRO,
            p.text3,
        ));

        Container::new(self.between(mode_seg, self.hstack(S2, right_badges)))
            .with_horizontal_padding(S4)
            .with_vertical_padding(8.)
            .with_background_color(if p.dark { p.sidebar } else { p.bg })
            .with_border(Border::bottom(1.).with_border_color(p.border))
            .finish()
    }

    fn im_channel_pill(
        &self,
        id: impl Into<String>,
        label: impl Into<String>,
        active: bool,
        action: Action,
    ) -> El {
        let p = self.p();
        let state = self.mouse_state(format!("im-ch-pill:{}", id.into()));
        let label = label.into();
        let font = self.font;
        Hoverable::new(state, move |mouse| {
            let hovered = mouse.is_mouse_over_element();
            let (bg, border, fg, weight) = if active {
                (p.accent_soft, p.accent, p.text, Weight::Semibold)
            } else if hovered {
                (p.hover, p.border_strong, p.text, Weight::Normal)
            } else {
                (p.surface, p.border, p.text2, Weight::Normal)
            };
            Container::new(
                facloud_ui::elements::Text::new(label.clone(), font, MICRO)
                    .with_color(fg)
                    .with_style(facloud_ui::fonts::Properties::default().weight(weight))
                    .soft_wrap(false)
                    .finish(),
            )
            .with_horizontal_padding(8.)
            .with_vertical_padding(3.)
            .with_background_color(bg)
            .with_border(Border::all(1.).with_border_color(border))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_CONTROL)))
            .finish()
        })
        .with_cursor(Cursor::PointingHand)
        .on_click(move |ctx, _, _| ctx.dispatch_typed_action(action.clone()))
        .finish()
    }

    fn im_thread_dialog(&self, t: &ImThread, snapshot: &PushSnapshot) -> El {
        let p = self.p();
        let app_label = if t.app_name.is_empty() {
            t.channel().label()
        } else {
            t.app_name.as_str()
        };
        let (badge_txt, badge_bg, badge_fg) = im_avatar_style(&t.package_name, app_label);
        let header_avatar = ConstrainedBox::new(
            Container::new(
                Flex::row()
                    .with_main_axis_size(MainAxisSize::Max)
                    .with_main_axis_alignment(MainAxisAlignment::Center)
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_child(self.line_w(badge_txt, CAPTION, badge_fg, Weight::Semibold))
                    .finish(),
            )
            .with_background_color(badge_bg)
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(18.)))
            .finish(),
        )
        .with_width(36.)
        .with_height(36.)
        .finish();

        let channel_status = if t.can_remote_reply {
            self.pill_badge("静默秒回", p.success_text, p.success_soft)
        } else if t.can_ui_reply {
            self.pill_badge("看屏投递", p.warning, p.warning_soft)
        } else {
            self.pill_badge("待激活", p.text2, p.tag_bg)
        };

        let title_row = self.hstack(
            8.,
            vec![
                self.line_w(&t.title, SUBHEAD, p.text, Weight::Semibold),
                self.pill_badge(
                    format!(
                        "{} · {}",
                        app_label,
                        if t.is_group { "群聊" } else { "私聊" }
                    ),
                    p.text2,
                    p.tag_bg,
                ),
                channel_status,
            ],
        );

        let reply_mode_pills: Vec<El> = ReplyMode::ALL
            .into_iter()
            .map(|mode| {
                self.im_channel_pill(
                    format!("rmode-{mode:?}"),
                    reply_mode_short_label(mode),
                    self.push.reply_mode == mode,
                    Action::Push(PushAction::SetReplyMode(mode)),
                )
            })
            .collect();

        let header_actions = self.hstack(
            6.,
            vec![
                self.hstack(4., reply_mode_pills),
                self.button(
                    "im-hdr-open-phone",
                    "手机打开",
                    Action::Push(PushAction::OpenOnPhone(t.notification_key.clone())),
                    Btn::Secondary,
                ),
            ],
        );

        let dialog_header = Container::new(self.between(
            self.hstack(
                10.,
                vec![
                    header_avatar,
                    self.vstack(
                        2.,
                        vec![
                            title_row,
                            self.line(
                                if !snapshot.screen_interactive || snapshot.keyguard_locked {
                                    format!("{} · 熄屏锁屏可后台无感秒回", t.package_name)
                                } else {
                                    format!("{} · 支持静默秒回与屏上气泡同步", t.package_name)
                                },
                                MICRO,
                                p.text3,
                            ),
                        ],
                    ),
                ],
            ),
            header_actions,
        ))
        .with_horizontal_padding(S4)
        .with_vertical_padding(10.)
        .with_background_color(p.bg)
        .with_border(Border::bottom(1.).with_border_color(p.border))
        .finish();

        // Message Bubble Stream
        let mut msg_items: Vec<El> = Vec::new();
        msg_items.push(
            Flex::row()
                .with_main_axis_size(MainAxisSize::Max)
                .with_main_axis_alignment(MainAxisAlignment::Center)
                .with_child(
                    Container::new(self.line(
                        format!(
                            "{}会话 · 共 {} 条已同步聊天气泡",
                            app_label,
                            t.messages.len()
                        ),
                        MICRO,
                        p.text3,
                    ))
                    .with_horizontal_padding(10.)
                    .with_vertical_padding(3.)
                    .with_background_color(p.tag_bg)
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(10.)))
                    .finish(),
                )
                .finish(),
        );

        for (idx, m) in t.messages.iter().enumerate() {
            let ts = super::format_time(m.timestamp_ms);
            let msg_text = if self.push.prefs.privacy_mask && !m.outgoing {
                "[防窥模式已开启，已隐藏消息内容]".to_string()
            } else {
                m.text.clone()
            };

            if m.outgoing {
                let status_txt = match m.delivery_status.as_str() {
                    "sending" => "发送中",
                    "failed" => "发送失败 · 草稿已保留",
                    "unknown" => "发送状态未确认 · 请核对手机",
                    _ if m.via == "remote_input" => "已提交给手机应用",
                    _ if m.via == "ui_bridge" => "手机会话已显示",
                    _ => "已发送",
                };
                let meta = if ts.is_empty() {
                    format!("我 · {status_txt}")
                } else {
                    format!("{ts} · {status_txt}")
                };
                let bubble_box = Container::new(self.text(msg_text, BODY, p.accent_fg))
                    .with_horizontal_padding(16.)
                    .with_vertical_padding(10.)
                    .with_background_color(p.accent)
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(16.)))
                    .finish();
                msg_items.push(
                    Flex::row()
                        .with_main_axis_size(MainAxisSize::Max)
                        .with_main_axis_alignment(MainAxisAlignment::End)
                        .with_child(
                            Flex::column()
                                .with_spacing(4.)
                                .with_cross_axis_alignment(CrossAxisAlignment::End)
                                .with_child(
                                    ConstrainedBox::new(bubble_box)
                                        .with_max_width(560.)
                                        .finish(),
                                )
                                .with_child(
                                    Container::new(self.mono(meta, MICRO, p.text3))
                                        .with_padding_right(4.)
                                        .finish(),
                                )
                                .finish(),
                        )
                        .finish(),
                );
            } else {
                let sender_name = if m.sender.trim().is_empty() {
                    t.title.as_str()
                } else {
                    m.sender.as_str()
                };
                let meta = if ts.is_empty() {
                    sender_name.to_string()
                } else {
                    format!("{sender_name} · {ts}")
                };

                let bubble_box = Container::new(self.text(msg_text, BODY, p.text))
                    .with_horizontal_padding(16.)
                    .with_vertical_padding(10.)
                    .with_background_color(p.bg)
                    .with_border(Border::all(1.).with_border_color(p.border))
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(16.)))
                    .finish();

                let mut msg_col = Flex::column()
                    .with_spacing(4.)
                    .with_cross_axis_alignment(CrossAxisAlignment::Start)
                    .with_child(
                        Container::new(self.line_w(meta, MICRO, p.text2, Weight::Medium))
                            .with_padding_left(4.)
                            .finish(),
                    )
                    .with_child(
                        ConstrainedBox::new(bubble_box)
                            .with_max_width(560.)
                            .finish(),
                    );

                if !self.push.prefs.privacy_mask {
                    let mut inline_actions: Vec<El> = Vec::new();
                    if let Some(code) = extract_otp_code(&m.text) {
                        inline_actions.push(self.button(
                            format!("im-msg-otp-{idx}"),
                            format!("复制验证码: {code}"),
                            Action::CopyText("验证码".into(), code),
                            Btn::Primary,
                        ));
                    }
                    if let Some(url) = extract_first_url(&m.text) {
                        inline_actions.push(self.button(
                            format!("im-msg-url-{idx}"),
                            "打开链接",
                            Action::StudioOpenUrl(url),
                            Btn::Secondary,
                        ));
                    }
                    if !inline_actions.is_empty() {
                        msg_col = msg_col.with_child(self.wrap(inline_actions));
                    }
                }

                msg_items.push(msg_col.finish());
            }
        }

        let stream_scroll = self.pane_scroll(
            self.sms_messages_scroll.clone(),
            Container::new(self.vstack(14., msg_items))
                .with_uniform_padding(S4)
                .finish(),
        );

        // Bottom Direct Reply Composer
        let quick_phrases = [
            ("收到", "收到"),
            ("没问题", "没问题"),
            ("谢谢", "谢谢"),
            ("稍后回你", "稍后回你"),
        ];
        let mut quick_chips: Vec<El> = quick_phrases
            .into_iter()
            .enumerate()
            .map(|(idx, (btn_label, phrase))| {
                self.button(
                    format!("im-qreply-{idx}"),
                    btn_label,
                    Action::Push(PushAction::QuickReplyPhrase {
                        thread_id: t.thread_id.clone(),
                        package_name: t.package_name.clone(),
                        title: t.title.clone(),
                        text: phrase.to_string(),
                    }),
                    Btn::Secondary,
                )
            })
            .collect();

        quick_chips.push(self.button(
            "im-qreply-ai",
            "AI 帮回",
            Action::Push(PushAction::AiDraftReply {
                thread_id: t.thread_id.clone(),
                package_name: t.package_name.clone(),
                title: t.title.clone(),
            }),
            Btn::Secondary,
        ));

        let is_sending = self.push.sending_thread.as_deref() == Some(t.thread_id.as_str());
        let status_node = if is_sending {
            Some(self.hstack(
                6.,
                vec![
                    self.dot(p.warning),
                    self.line("正在发送回复…", MICRO, p.warning),
                ],
            ))
        } else {
            self.push.last_reply_status.as_ref().map(|st| {
                self.line(
                    st,
                    MICRO,
                    if st.starts_with("回复未完成") {
                        p.danger
                    } else {
                        p.text2
                    },
                )
            })
        };

        let quick_bar = match status_node {
            Some(st) => self.between(self.wrap(quick_chips), st),
            None => self.wrap(quick_chips),
        };

        let composer_input_row = Container::new(
            Flex::row()
                .with_main_axis_size(MainAxisSize::Max)
                .with_spacing(8.)
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_child(
                    Expanded::new(
                        1.,
                        self.input(F_IM_REPLY, "输入回复内容，按回车发送…", None),
                    )
                    .finish(),
                )
                .with_child(self.button(
                    "im-send-reply",
                    if is_sending { "发送中" } else { "发送" },
                    Action::Push(PushAction::SendImReply),
                    Btn::Primary,
                ))
                .finish(),
        )
        .with_uniform_padding(8.)
        .with_background_color(p.bg)
        .with_border(Border::all(1.).with_border_color(p.border))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
        .finish();

        let mut composer_rows = vec![quick_bar];
        if !t.can_reply() || !t.can_remote_reply {
            composer_rows.push(self.button(
                "im-fallback-open-app",
                "通知快捷句柄已过期 · 在手机打开应用回复",
                Action::OpenApp(t.package_name.clone()),
                Btn::Secondary,
            ));
        }
        composer_rows.push(composer_input_row);

        let bottom_composer = Container::new(self.vstack(8., composer_rows))
            .with_horizontal_padding(S4)
            .with_padding_bottom(S4)
            .with_padding_top(S2)
            .with_background_color(p.bg)
            .with_border(Border::top(1.).with_border_color(p.border))
            .finish();

        Flex::column()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_child(dialog_header)
            .with_child(Expanded::new(1., stream_scroll).finish())
            .with_child(bottom_composer)
            .finish()
    }

    pub(in super::super) fn sms_page(&self) -> El {
        let p = self.p();
        if !self.demo_mode && self.selected.is_none() {
            return Flex::column()
                .with_main_axis_size(MainAxisSize::Max)
                .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                .with_child(self.comm_mode_bar())
                .with_child(
                    Expanded::new(
                        1.,
                        self.empty(
                            "连接手机后查看消息与短信",
                            "连接后将同步微信、企业微信、钉钉、飞书会话以及手机原生短信。",
                            Some(self.button(
                                "sms-connect",
                                "连接设备",
                                Action::Page(Page::Connect),
                                Btn::Primary,
                            )),
                        ),
                    )
                    .finish(),
                )
                .finish();
        }

        let demo_fallback: PushSnapshot;
        let snapshot: &PushSnapshot = if self.demo_mode
            && self.push.snapshot.threads.is_empty()
            && self.push.snapshot.notifications.is_empty()
        {
            demo_fallback = demo_push_snapshot();
            &demo_fallback
        } else {
            &self.push.snapshot
        };

        let loaded = self.sms_loaded || self.demo_mode;
        let search_q = self.fields[F_SMS_SEARCH].text.trim().to_lowercase();
        let filtered = self.sms_filter != SmsFilter::All || !search_q.is_empty();

        // Build native SMS threads from the phone provider snapshot.
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

        // Filter IM threads from `snapshot.threads`
        let matching_im_threads: Vec<&ImThread> = if self.push.im_channel == ImChannel::Sms {
            Vec::new()
        } else {
            snapshot
                .threads
                .iter()
                .filter(|t| self.push.im_channel.matches_package(&t.package_name))
                .filter(|t| {
                    if search_q.is_empty() {
                        return true;
                    }
                    t.title.to_lowercase().contains(&search_q)
                        || t.app_name.to_lowercase().contains(&search_q)
                        || t.last_sender.to_lowercase().contains(&search_q)
                        || t.last_message.to_lowercase().contains(&search_q)
                        || t.messages
                            .iter()
                            .any(|m| m.text.to_lowercase().contains(&search_q))
                })
                .collect()
        };

        let is_new_sms = self.selected_thread.as_deref() == Some("__new__");

        // Determine whether an IM thread or a native SMS thread is currently active in the right pane
        let active_im_thread: Option<&ImThread> =
            if is_new_sms || self.push.im_channel == ImChannel::Sms {
                None
            } else if let Some(sel_id) = self.push.selected_im_thread.as_deref() {
                if sel_id.is_empty() {
                    None
                } else {
                    matching_im_threads
                        .iter()
                        .copied()
                        .find(|t| t.thread_id == sel_id)
                        .or_else(|| snapshot.threads.iter().find(|t| t.thread_id == sel_id))
                }
            } else if self.selected_thread.is_none() || self.push.im_channel != ImChannel::All {
                matching_im_threads.first().copied()
            } else {
                None
            };

        let active_im_id = active_im_thread.map(|t| t.thread_id.as_str()).unwrap_or("");

        let active_addr = if is_new_sms {
            "__new__".to_string()
        } else if active_im_thread.is_some() {
            String::new()
        } else if let Some(sel) = &self.selected_thread {
            sel.clone()
        } else if let Some(first) = display_threads.first() {
            first.num.clone()
        } else {
            String::new()
        };

        // Top Channel Bar (`ImChannel`)
        let channel_tabs = [
            (ImChannel::All, "全部消息"),
            (ImChannel::WeChat, "微信"),
            (ImChannel::WeCom, "企微"),
            (ImChannel::DingTalk, "钉钉"),
            (ImChannel::Feishu, "飞书"),
            (ImChannel::QqTim, "QQ/TIM"),
            (ImChannel::GlobalIm, "国际IM"),
            (ImChannel::Sms, "原生短信"),
        ];
        let channel_pills: Vec<El> = channel_tabs
            .into_iter()
            .map(|(ch, label)| {
                self.im_channel_pill(
                    format!("ch-{ch:?}"),
                    label,
                    self.push.im_channel == ch,
                    Action::Push(PushAction::SelectImChannel(ch)),
                )
            })
            .collect();

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

        // 1. Render matching IM threads first when channel != ImChannel::Sms
        for (i, t) in matching_im_threads.iter().enumerate() {
            if !thread_rows.is_empty() {
                thread_rows.push(self.divider());
            }
            let active = !is_new_sms && t.thread_id == active_im_id;
            let app_label = if t.app_name.is_empty() {
                t.channel().label()
            } else {
                t.app_name.as_str()
            };
            let (badge_txt, badge_bg, badge_fg) = im_avatar_style(&t.package_name, app_label);
            let avatar = ConstrainedBox::new(
                Container::new(
                    Flex::row()
                        .with_main_axis_size(MainAxisSize::Max)
                        .with_main_axis_alignment(MainAxisAlignment::Center)
                        .with_cross_axis_alignment(CrossAxisAlignment::Center)
                        .with_child(self.line_w(badge_txt, CAPTION, badge_fg, Weight::Semibold))
                        .finish(),
                )
                .with_background_color(badge_bg)
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(18.)))
                .finish(),
            )
            .with_width(36.)
            .with_height(36.)
            .finish();

            let mut title_badges = vec![
                self.line_w(&t.title, CAPTION, p.text, Weight::Semibold),
                self.pill_badge(app_label, p.text2, p.tag_bg),
            ];
            if t.can_remote_reply {
                title_badges.push(self.pill_badge("可秒回", p.success_text, p.success_soft));
            } else if t.can_ui_reply {
                title_badges.push(self.pill_badge("看屏回", p.text2, p.tag_bg));
            }

            let raw_preview = if self.push.prefs.privacy_mask {
                "[防窥保护已开启]".to_string()
            } else if !t.last_sender.is_empty()
                && t.last_sender != t.title
                && !t.last_message.starts_with(&t.last_sender)
            {
                format!("{}: {}", t.last_sender, t.last_message)
            } else {
                t.last_message.clone()
            };
            let single_line: String = raw_preview
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            let preview_str = if single_line.chars().count() > 36 {
                format!("{}…", single_line.chars().take(36).collect::<String>())
            } else {
                single_line
            };

            let has_unread = t.unread_count > 0;
            let preview_el = self.line_w(
                preview_str,
                CAPTION,
                if has_unread { p.text } else { p.text2 },
                if has_unread {
                    Weight::Medium
                } else {
                    Weight::Normal
                },
            );

            let bottom_row = if has_unread {
                let unread_pill =
                    self.pill_badge(format!("{}", t.unread_count), ColorU::white(), p.danger);
                self.between(preview_el, unread_pill)
            } else {
                preview_el
            };

            let right_col = Flex::column()
                .with_spacing(4.)
                .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                .with_child(self.between(
                    self.wrap(title_badges),
                    self.mono(super::format_time(t.last_timestamp_ms), MICRO, p.text3),
                ))
                .with_child(bottom_row)
                .finish();

            let row_inner = Flex::row()
                .with_main_axis_size(MainAxisSize::Max)
                .with_cross_axis_alignment(CrossAxisAlignment::Start)
                .with_spacing(12.)
                .with_child(avatar)
                .with_child(Expanded::new(1., right_col).finish())
                .finish();

            let state = self.mouse_state(format!("im-th:{i}"));
            let tid = t.thread_id.clone();
            let row_el = Hoverable::new(state, move |mouse| {
                let hovered = mouse.is_mouse_over_element();
                let bg = if active {
                    p.selected
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
                ctx.dispatch_typed_action(Action::Push(PushAction::SelectImThread(tid.clone())))
            })
            .finish();

            thread_rows.push(row_el);
        }

        // 2. Render native SMS threads when channel is `All` or `Sms`
        let show_native_sms = matches!(self.push.im_channel, ImChannel::All | ImChannel::Sms);
        if show_native_sms {
            for (i, t) in display_threads.iter().enumerate() {
                if !thread_rows.is_empty() {
                    thread_rows.push(self.divider());
                }
                let active = !is_new_sms
                    && active_im_thread.is_none()
                    && !active_addr.is_empty()
                    && norm_addr(&active_addr) == norm_addr(&t.num);
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
                            format!("短信 · 匹配 {} 条", t.count)
                        } else {
                            format!("短信 · {} 条", t.count)
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
                    ctx.dispatch_typed_action(Action::Push(PushAction::SelectImThread(
                        String::new(),
                    )));
                    ctx.dispatch_typed_action(Action::SelectSmsThread(Some(num_str.clone())));
                })
                .finish();
                thread_rows.push(row_el);
            }
        }

        if show_native_sms {
            if let Some(error) = &self.sms_error {
                if thread_rows.is_empty() {
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
                }
            } else if !loaded && thread_rows.is_empty() {
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
            }
        }

        if thread_rows.is_empty() {
            thread_rows.push(self.empty(
                if self.push.im_channel == ImChannel::Sms {
                    if self.sms.is_empty() {
                        "手机暂无短信"
                    } else {
                        "没有匹配的短信"
                    }
                } else {
                    "暂无匹配的聊天会话"
                },
                if self.push.im_channel == ImChannel::Sms {
                    "收到或发送短信后，刷新即可查看。"
                } else {
                    "当手机收到微信、企微、钉钉、飞书或 QQ 消息时将自动生成会话并支持直接秒回。"
                },
                Some(self.button(
                    "im-empty-sync-win",
                    "同步聊天屏",
                    Action::Push(PushAction::SyncActivePhoneChatWindow),
                    Btn::Secondary,
                )),
            ));
        }

        let left_scroll = self.pane_scroll(
            self.sms_threads_scroll.clone(),
            self.vstack(0., thread_rows),
        );

        let mut left_top_controls = vec![
            self.input(
                F_SMS_SEARCH,
                "搜索微信/钉钉/飞书会话、联系人、号码或验证码…",
                None,
            ),
            self.wrap(channel_pills),
        ];
        if self.push.im_channel == ImChannel::Sms {
            left_top_controls.push(filter_bar);
        }
        left_top_controls.push(self.line(
            format!(
                "{} 个即时通讯会话 · {} 个短信会话{}",
                matching_im_threads.len(),
                display_threads.len(),
                if filtered { "（已筛选）" } else { "" }
            ),
            11.,
            p.text3,
        ));

        let left_pane = self.fixed_w(
            LIST_PANE_W + 24.,
            Container::new(
                Flex::column()
                    .with_main_axis_size(MainAxisSize::Max)
                    .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                    .with_child(
                        Container::new(self.vstack(8., left_top_controls))
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

        // If an IM thread is selected, render the IM chat bubble dialog & direct reply composer
        if let Some(im_thread) = active_im_thread {
            let right_pane = self.im_thread_dialog(im_thread, snapshot);
            let split = Flex::row()
                .with_main_axis_size(MainAxisSize::Max)
                .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                .with_child(left_pane)
                .with_child(Expanded::new(1., right_pane).finish())
                .finish();
            return Flex::column()
                .with_main_axis_size(MainAxisSize::Max)
                .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                .with_child(self.comm_mode_bar())
                .with_child(Expanded::new(1., split).finish())
                .finish();
        }

        // Otherwise render the native SMS conversation & SMS composer unchanged
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
            self.pane_scroll(self.sms_messages_scroll.clone(), timeline_inner)
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

        let split = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_child(left_pane)
            .with_child(Expanded::new(1., right_pane).finish())
            .finish();

        Flex::column()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_child(self.comm_mode_bar())
            .with_child(Expanded::new(1., split).finish())
            .finish()
    }
}

use super::super::{
    amoo_display::{retry_prompt, session_title, ToolRecord},
    icons::UiIcon,
    theme::*,
    widgets::*,
    Action, AmooChatItem, AmooUiAction, Companion, SafetyDecision, F_AMOO, F_AMOO_GUI_APP,
    F_AMOO_SEARCH,
};
use facloud_ui::{
    assets::asset_cache::AssetSource, color::ColorU, elements::*, fonts::Weight, platform::Cursor,
};
use pathfinder_geometry::vector::vec2f;
use stepx_companion::{amoo_gui::GuiRunState, device::BackScreenPattern};

fn ensure_amoo_tti_portrait_path() -> String {
    const PORTRAIT_PNG: &[u8] = include_bytes!("../../../assets/amoo_tti_portrait.png");
    let path = "/tmp/stepx_amoo_tti_portrait.png";
    if !std::path::Path::new(path).exists() {
        let _ = std::fs::write(path, PORTRAIT_PNG);
    }
    path.to_string()
}

const CAPABILITY_PILLS: [(&str, &str); 5] = [
    ("深度研究", "请针对以下主题进行深度研究并整理核心结论："),
    (
        "背屏图案",
        "帮我设计一个 23×17 的背屏赛博闪电像素图案并同步到背屏",
    ),
    (
        "AI 剪辑",
        "分析手机相册最新视频并生成 15 秒卡点高光剪辑方案",
    ),
    ("拍摄指导", "开启拍摄构图指导，并在背屏显示 3 秒倒计时闪光"),
    ("帮我写", "帮我起草一段礼貌简洁的会议改期确认回复："),
];

// A finished model turn is not proof that a phone operation succeeded. Only
// inspect the latest substantive assistant reply, never tool logs or old turns.
fn session_waits_for_unlock(items: &[AmooChatItem]) -> bool {
    for item in items.iter().rev() {
        match item {
            AmooChatItem::UserText { .. } | AmooChatItem::UserVoice { .. } => return false,
            AmooChatItem::AssistantRich {
                text: Some(body), ..
            } => {
                let Some(body) = stepx_companion::work_protocol::display_text(body, false)
                    .filter(|text| !text.trim().is_empty())
                else {
                    continue;
                };
                return reply_reports_lock_block(&body);
            }
            _ => {}
        }
    }
    false
}

fn reply_reports_lock_block(body: &str) -> bool {
    if ["解锁后已完成", "解锁后已经完成", "现已解锁并完成"]
        .iter()
        .any(|phrase| body.contains(phrase))
    {
        return false;
    }
    let locked = ["锁屏", "已锁定", "锁着", "手机锁定"]
        .iter()
        .any(|phrase| body.contains(phrase));
    let blocked = [
        "拦截",
        "被拦",
        "阻止",
        "无法",
        "不能",
        "失败",
        "卡在",
        "未能",
        "还没完成",
        "无法完成",
    ]
    .iter()
    .any(|phrase| body.contains(phrase));
    let unlock_request = [
        "请先解锁",
        "请解锁",
        "需要先解锁",
        "等你解锁",
        "手动解锁后",
        "你先解锁",
    ]
    .iter()
    .any(|phrase| body.contains(phrase));
    (locked && blocked) || unlock_request
}

impl Companion {
    fn legacy_demo(&self) -> bool { self.demo_mode && !std::env::args().any(|a| a == "--press-capture") }
    fn amoo_action_icon(
        &self,
        id: impl Into<String>,
        label: impl Into<String>,
        icon: UiIcon,
        action: Action,
    ) -> El {
        let p = self.p();
        let label = label.into();
        Hoverable::new(self.mouse_state(id.into()), move |mouse| {
            let hovered = mouse.is_mouse_over_element();
            let button = ConstrainedBox::new(
                Container::new(Align::new(self.ui_icon(icon, 16., p.text2)).finish())
                    .with_background_color(if hovered { p.hover } else { p.bg })
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_CONTROL)))
                    .finish(),
            )
            .with_width(30.)
            .with_height(30.)
            .finish();
            if !hovered {
                return button;
            }
            let tooltip = Container::new(self.line(label, 11., p.text))
                .with_horizontal_padding(8.)
                .with_vertical_padding(5.)
                .with_background_color(p.bg)
                .with_border(Border::all(1.).with_border_color(p.border))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_CONTROL)))
                .finish();
            Stack::new()
                .with_child(button)
                .with_positioned_child(
                    tooltip,
                    OffsetPositioning::offset_from_parent(
                        vec2f(0., -5.),
                        ParentOffsetBounds::WindowByPosition,
                        ParentAnchor::TopMiddle,
                        ChildAnchor::BottomMiddle,
                    ),
                )
                .finish()
        })
        .with_cursor(Cursor::PointingHand)
        .on_click(move |ctx, _, _| ctx.dispatch_typed_action(action.clone()))
        .finish()
    }

    fn amoo_execution_records(&self, key: &str, records: &[ToolRecord]) -> El {
        let p = self.p();
        let expanded = self.amoo_expanded_tool_records.contains(key);
        let authorization_failed = records.iter().any(|record| record.detail.as_deref()
            .is_some_and(|detail| detail.starts_with("授权超时：")));
        let action = Action::AmooToggleToolRecords(key.into());
        let mut names = Vec::<(String, usize)>::new();
        for record in records {
            if let Some((_, count)) = names.iter_mut().find(|(name, _)| *name == record.name) {
                *count += 1;
            } else {
                names.push((record.name.clone(), 1));
            }
        }
        let summary = names
            .iter()
            .map(|(name, count)| {
                if *count == 1 {
                    name.clone()
                } else {
                    format!("{name} × {count}")
                }
            })
            .collect::<Vec<_>>()
            .join(" · ");
        let header = Hoverable::new(
            self.mouse_state(format!("amoo-records:{key}")),
            move |mouse| {
                Container::new(self.hstack(
                    8.,
                    vec![
                        self.ui_icon(
                            if expanded {
                                UiIcon::ChevronDown
                            } else {
                                UiIcon::ChevronRight
                            },
                            14.,
                            p.text2,
                        ),
                        self.line_w(
                            format!("执行记录 · {} 条{}", records.len(), if authorization_failed { " · 授权超时" } else { "" }),
                            CAPTION,
                            p.text2,
                            Weight::Medium,
                        ),
                        Expanded::new(1., self.line(summary, 11., p.text3)).finish(),
                    ],
                ))
                .with_uniform_padding(10.)
                .with_background_color(if mouse.is_mouse_over_element() {
                    p.hover
                } else {
                    p.surface
                })
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
                .finish()
            },
        )
        .with_cursor(Cursor::PointingHand)
        .on_click(move |ctx, _, _| ctx.dispatch_typed_action(action.clone()))
        .finish();
        let mut children = vec![header];
        if expanded {
            let entries = records
                .iter()
                .enumerate()
                .map(|(index, record)| {
                    let mut parts = vec![self.between(
                        self.hstack(
                            8.,
                            vec![
                                self.mono(format!("{:02}", index + 1), 11., p.text3),
                                self.line(record.name.clone(), CAPTION, p.text2),
                            ],
                        ),
                        self.mono(record.time_label.clone(), 11., p.text3),
                    )];
                    if let Some(detail) = record.detail.as_ref().filter(|s| !s.is_empty()) {
                        parts.push(self.text(detail.clone(), 11., p.text2));
                    }
                    Container::new(self.vstack(4., parts))
                        .with_horizontal_padding(12.)
                        .with_vertical_padding(8.)
                        .with_border(Border::top(1.).with_border_color(p.surface_border))
                        .finish()
                })
                .collect();
            children.push(self.vstack(0., entries));
        }
        Container::new(
            Container::new(self.vstack(0., children))
                .with_background_color(p.surface)
                .with_border(Border::all(1.).with_border_color(p.surface_border))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
                .finish(),
        )
        .with_margin_left(34.)
        .finish()
    }

    pub(in super::super) fn amoo_actions(&self) -> Vec<El> {
        let mut actions = vec![self.button(
            "amoo-schedules",
            if self.amoo_show_schedules {
                "返回对话"
            } else {
                "定时任务"
            },
            Action::AmooToggleSchedules,
            Btn::Secondary,
        )];
        if self.legacy_demo() {
            actions.push(self.button(
                "amoo-express-quick",
                if self.legacy_demo() {
                    "📦 查快递 (2)"
                } else {
                    "查快递"
                },
                Action::AmooToggleExpressCard,
                Btn::Secondary,
            ));
        }
        if !self.legacy_demo() {
            actions.push(self.button(
                "amoo-sync-real",
                "↻ 后台同步",
                Action::AmooSnapshot,
                Btn::Secondary,
            ));
        }
        if self.legacy_demo() {
            actions.push(self.button(
                "amoo-read",
                "⊕ 圈选感知",
                Action::ReadVisible,
                Btn::Secondary,
            ));
        }
        actions.push(self.button(
            "amoo-details",
            if self.amoo_show_details {
                "收起详情"
            } else {
                "会话详情"
            },
            Action::AmooToggleDetails,
            Btn::Secondary,
        ));
        actions.push(self.button(
            "amoo-new-chat",
            "＋ 新建对话",
            Action::AmooNewChat,
            Btn::Primary,
        ));
        actions
    }

    fn agent_chip(&self, idx: usize, label: &'static str) -> El {
        let p = self.p();
        let active = self.amoo_agent_idx == idx;
        let state = self.mouse_state(format!("amoo-agent:{idx}"));
        let row = if active {
            self.lead(
                6.,
                self.dot(p.success),
                self.line_w(label, CAPTION, p.accent_fg, Weight::Medium),
            )
        } else {
            self.line(label, CAPTION, p.text2)
        };
        Hoverable::new(state, move |mouse| {
            let hovered = mouse.is_mouse_over_element();
            let bg = if active {
                p.accent
            } else if hovered {
                p.hover
            } else {
                p.surface
            };
            let border = if active { p.accent } else { p.surface_border };
            Container::new(row)
                .with_horizontal_padding(10.)
                .with_vertical_padding(6.)
                .with_background_color(bg)
                .with_border(Border::all(1.).with_border_color(border))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_CONTROL)))
                .finish()
        })
        .with_cursor(Cursor::PointingHand)
        .on_click(move |ctx, _, _| ctx.dispatch_typed_action(Action::AmooSelectAgent(idx)))
        .finish()
    }

    fn session_card(
        &self,
        idx: usize,
        title: String,
        agent: String,
        status: String,
        status_kind: u8,
        meta: String,
    ) -> El {
        let p = self.p();
        let active = self.amoo_session_idx == idx;
        let state = self.mouse_state(format!("amoo-sess-card:{idx}"));
        let badge = match status_kind {
            1 => Container::new(self.line_w(status, 11., p.warning, Weight::Medium))
                .with_horizontal_padding(6.)
                .with_vertical_padding(2.)
                .with_background_color(rgba(0xF5, 0x9E, 0x0B, 26))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
                .finish(),
            2 => Container::new(self.line_w(status, 11., p.success_text, Weight::Medium))
                .with_horizontal_padding(6.)
                .with_vertical_padding(2.)
                .with_background_color(p.success_soft)
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
                .finish(),
            _ => self.line(status, 11., p.text2),
        };
        let content = self.vstack(
            4.,
            vec![
                self.between(self.line_w(title, CAPTION, p.text, Weight::Semibold), badge),
                self.between(
                    self.line(agent, 11., p.text2),
                    self.mono(meta, 11., p.text2),
                ),
            ],
        );
        Hoverable::new(state, move |mouse| {
            let hovered = mouse.is_mouse_over_element();
            let bg = if active {
                if p.dark {
                    rgb(0x22, 0x22, 0x27)
                } else {
                    rgb(0xF4, 0xF4, 0xF6)
                }
            } else if hovered {
                p.hover
            } else {
                p.bg
            };
            let border = if active { p.border_strong } else { p.border };
            Container::new(content)
                .with_uniform_padding(10.)
                .with_background_color(bg)
                .with_border(Border::all(1.).with_border_color(border))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
                .finish()
        })
        .with_cursor(Cursor::PointingHand)
        .on_click(move |ctx, _, _| ctx.dispatch_typed_action(Action::AmooSelectSession(idx)))
        .finish()
    }

    pub(in super::super) fn amoo_page(&self) -> El {
        if let Some(picker) = self.amoo_album_picker() {
            return picker;
        }
        let p = self.p();
        let query = self.fields[F_AMOO_SEARCH].text.trim().to_lowercase();

        // 1. Col 1 (3 cols): Session Center, Parallel Tasks & Agent Switcher
        let agent_selector = Container::new(self.vstack(
            8.,
            vec![
                    self.between(
                        self.heading("协同智能体"),
                        Container::new(self.line_w(
                            if self.legacy_demo() || self.amoo_gateway_online {
                                "在线"
                            } else {
                                "未连接"
                            },
                            11.,
                            p.success_text,
                            Weight::Medium,
                        ))
                        .with_horizontal_padding(6.)
                        .with_vertical_padding(2.)
                        .with_background_color(p.success_soft)
                        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
                        .finish(),
                    ),
                    Flex::row()
                        .with_main_axis_size(MainAxisSize::Max)
                        .with_spacing(6.)
                        .with_child(Expanded::new(1., self.agent_chip(0, "Amoo 助手")).finish())
                        .with_child(Expanded::new(1., self.agent_chip(1, "桌面伙伴")).finish())
                        .finish(),
                    Flex::row()
                        .with_main_axis_size(MainAxisSize::Max)
                        .with_spacing(6.)
                        .with_child(Expanded::new(1., self.agent_chip(2, "飞书 / 企微")).finish())
                        .with_child(Expanded::new(1., self.agent_chip(3, "Claude 协同")).finish())
                        .finish(),
                ],
        ))
        .with_uniform_padding(10.)
        .with_background_color(p.bg)
        .with_border(Border::all(1.).with_border_color(p.border))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
        .finish();

        let agent_selector = if self.legacy_demo() {
            agent_selector
        } else {
            Container::new(self.vstack(
                S2,
                vec![
                    self.between(
                        self.hstack(
                            S2,
                            vec![
                                self.ui_icon(UiIcon::Star, 20., p.text),
                                self.heading("Amoo"),
                            ],
                        ),
                        self.pill_badge(
                            if self.amoo_gateway_online {
                                "已连接"
                            } else {
                                "未连接"
                            },
                            if self.amoo_gateway_online {
                                p.success_text
                            } else {
                                p.text3
                            },
                            if self.amoo_gateway_online {
                                p.success_soft
                            } else {
                                p.surface
                            },
                        ),
                    ),
                    self.text("与手机同步的日常对话", CAPTION, p.text2),
                ],
            ))
            .with_uniform_padding(S3)
            .with_background_color(p.bg)
            .with_border(Border::all(1.).with_border_color(p.border))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
            .finish()
        };
        let search_box = self.input(F_AMOO_SEARCH, "搜索会话或定时任务…", None);

        let filtered_sessions: Vec<(usize, &super::super::AmooSessionItem)> = self
            .amoo_sessions
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                query.is_empty()
                    || session_title(&s.title).to_lowercase().contains(&query)
                    || s.agent.to_lowercase().contains(&query)
                    || s.status.to_lowercase().contains(&query)
            })
            .collect();

        let raw_running = self
            .amoo_sessions
            .iter()
            .filter(|s| s.status == "执行中" || s.status == "进行中" || s.status == "请求中")
            .count();
        let running_label = if self.legacy_demo() {
            let c = self
                .amoo_sessions
                .iter()
                .filter(|s| s.status_kind == 2 || s.status == "执行中" || s.status == "进行中")
                .count()
                .max(1);
            format!("{c} 项运行中")
        } else if raw_running > 0 {
            format!("{raw_running} 项执行中")
        } else {
            format!("{} 个定时任务", self.amoo_cron_count)
        };

        let running_state = self.mouse_state("amoo-running-link");
        let running_badge = Hoverable::new(running_state, move |_| {
            self.line_w(running_label.clone(), 11., p.success, Weight::Medium)
        })
        .with_cursor(Cursor::PointingHand)
        .on_click(|ctx, _, _| ctx.dispatch_typed_action(Action::AmooOpenTasks))
        .finish();

        let sessions_header = Container::new(self.between(
            self.heading(format!("会话与任务 ({})", filtered_sessions.len())),
            running_badge,
        ))
        .with_horizontal_padding(4.)
        .finish();

        let mut session_cards: Vec<El> = Vec::new();
        let max_cards = if self.legacy_demo() { 6 } else { 50 };
        for (idx, sess) in filtered_sessions.into_iter().take(max_cards) {
            let waiting_unlock = !self.legacy_demo() && session_waits_for_unlock(&sess.items);
            session_cards.push(self.session_card(
                idx,
                session_title(&sess.title),
                sess.agent.clone(),
                if waiting_unlock {
                    "等待解锁".into()
                } else {
                    sess.status.clone()
                },
                if waiting_unlock { 1 } else { sess.status_kind },
                sess.meta.clone(),
            ));
        }

        let left_bottom = Container::new(
            Flex::row()
                .with_main_axis_size(MainAxisSize::Max)
                .with_spacing(6.)
                .with_child(
                    Expanded::new(
                        1.,
                        self.button(
                            "amoo-skill-store",
                            if self.amoo_skill_manage {
                                "✓ 完成管理"
                            } else {
                                "⊞ 技能市场"
                            },
                            if self.legacy_demo() {
                                Action::AmooToggleSkillManage
                            } else {
                                Action::Work(super::super::WorkAction::Tab(2))
                            },
                            if self.amoo_skill_manage {
                                Btn::Primary
                            } else {
                                Btn::Secondary
                            },
                        ),
                    )
                    .finish(),
                )
                .with_child(
                    Expanded::new(
                        1.,
                        self.button(
                            "amoo-npu-models",
                            "◈ 模型目录",
                            if self.legacy_demo() {
                                Action::AmooOpenModels
                            } else {
                                Action::Work(super::super::WorkAction::Tab(1))
                            },
                            Btn::Secondary,
                        ),
                    )
                    .finish(),
                )
                .finish(),
        )
        .with_uniform_padding(12.)
        .with_background_color(if p.dark { p.sidebar } else { p.surface })
        .with_border(Border::top(1.).with_border_color(p.border))
        .finish();

        let left_col = if self.legacy_demo() {
            let mut session_els = vec![sessions_header];
            session_els.extend(session_cards);
            let sessions_block = self.vstack(6., session_els);
            let left_top =
                Container::new(self.vstack(12., vec![agent_selector, search_box, sessions_block]))
                    .with_uniform_padding(14.)
                    .finish();
            Container::new(
                Flex::column()
                    .with_main_axis_size(MainAxisSize::Max)
                    .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
                    .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                    .with_child(left_top)
                    .with_child(left_bottom)
                    .finish(),
            )
            .with_border(Border::right(1.).with_border_color(p.border))
            .finish()
        } else {
            let top_fixed =
                Container::new(self.vstack(10., vec![agent_selector, search_box, sessions_header]))
                    .with_padding_left(14.)
                    .with_padding_right(14.)
                    .with_padding_top(14.)
                    .with_padding_bottom(6.)
                    .finish();
            let scrollable_sessions = Expanded::new(
                1.,
                Container::new(
                    self.pane_scroll(self.scroll_aux.clone(), self.vstack(6., session_cards)),
                )
                .with_padding_left(14.)
                .with_padding_right(14.)
                .with_padding_bottom(8.)
                .finish(),
            )
            .finish();
            Container::new(
                Flex::column()
                    .with_main_axis_size(MainAxisSize::Max)
                    .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                    .with_child(top_fixed)
                    .with_child(scrollable_sessions)
                    .with_child(left_bottom)
                    .finish(),
            )
            .with_border(Border::right(1.).with_border_color(p.border))
            .finish()
        };

        // 2. Col 2 (6 cols): Main Multimodal Conversation + Interactive Composer
        let scene_pills: Vec<El> = CAPABILITY_PILLS
            .iter()
            .enumerate()
            .map(|(i, (label, tpl))| {
                let state = self.mouse_state(format!("amoo-cap-pill:{i}"));
                let act = Action::AmooSelectCap(i, (*tpl).into());
                let is_active = self.amoo_cap_idx == i;
                Hoverable::new(state, move |mouse| {
                    let hovered = mouse.is_mouse_over_element();
                    let bg = if is_active {
                        if p.dark {
                            rgb(0x27, 0x27, 0x2A)
                        } else {
                            rgb(0xF4, 0xF4, 0xF6)
                        }
                    } else if hovered {
                        p.hover
                    } else {
                        p.bg
                    };
                    let border = if is_active { p.border_strong } else { p.border };
                    Container::new(self.line_w(
                        *label,
                        CAPTION,
                        p.text,
                        if is_active {
                            Weight::Semibold
                        } else {
                            Weight::Normal
                        },
                    ))
                    .with_horizontal_padding(10.)
                    .with_vertical_padding(4.)
                    .with_background_color(bg)
                    .with_border(Border::all(1.).with_border_color(border))
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(12.)))
                    .finish()
                })
                .with_cursor(Cursor::PointingHand)
                .on_click(move |ctx, _, _| ctx.dispatch_typed_action(act.clone()))
                .finish()
            })
            .collect();

        let tts_state = self.mouse_state("amoo-tts-toggle");
        let tts_on = self.amoo_tts_on;
        let tts_toggle = Hoverable::new(tts_state, move |mouse| {
            let fg = if tts_on {
                p.success
            } else if mouse.is_mouse_over_element() {
                p.text
            } else {
                p.text2
            };
            self.line_w(
                if tts_on {
                    "🔊 朗读: 开"
                } else {
                    "朗读: 关"
                },
                11.,
                fg,
                if tts_on {
                    Weight::Semibold
                } else {
                    Weight::Normal
                },
            )
        })
        .with_cursor(Cursor::PointingHand)
        .on_click(|ctx, _, _| ctx.dispatch_typed_action(Action::AmooToggleTts))
        .finish();

        let model_selector_row = self.between(
            self.hstack(
                6.,
                vec![
                    self.line_w("模型:", 11., p.text2, Weight::Medium),
                    self.line(self.active_amoo_model_short(), 11., p.text),
                    self.link_muted(
                        "amoo-real-models",
                        "模型目录",
                        Action::Work(super::super::WorkAction::Tab(1)),
                    ),
                ],
            ),
            if self.legacy_demo() {
                tts_toggle
            } else {
                self.line("手机后台同步", 11., p.text3)
            },
        );

        let phone_mode = self.amoo_ui.screen_mode;
        let mode_row = self.hstack(
            6.,
            vec![
                self.button(
                    "amoo-mode-chat",
                    "后台对话",
                    Action::AmooUi(AmooUiAction::GuiMode(false)),
                    if phone_mode {
                        Btn::Ghost
                    } else {
                        Btn::Secondary
                    },
                ),
                self.button(
                    "amoo-mode-phone",
                    if self.amoo_ui.gui_running {
                        "操作手机 · 等待结果"
                    } else if self.amoo_ui.gui_unconfirmed {
                        "操作手机 · 待确认"
                    } else {
                        "操作手机"
                    },
                    Action::AmooUi(AmooUiAction::GuiMode(true)),
                    if phone_mode {
                        Btn::Secondary
                    } else {
                        Btn::Ghost
                    },
                ),
            ],
        );
        let mut top_rows = vec![self.between(mode_row, if !phone_mode && !self.legacy_demo() {
            self.hstack(4., [("pause", "暂停手机"), ("resume", "继续"), ("stop", "停止")].into_iter().map(|(op, label)|
                self.button(format!("amoo-native-task-{op}"), label, Action::AmooUi(AmooUiAction::GuiPhoneControl(op.into())), Btn::Ghost)
            ).collect())
        } else { Empty::new().finish() })];
        if phone_mode {
            top_rows.push(self.amoo_gui_controls());
        } else {
            top_rows.push(model_selector_row);
            top_rows.push(self.between(
                self.wrap(scene_pills),
                if self.legacy_demo() {
                    self.link_action(
                        "amoo-top-express",
                        if self.legacy_demo() {
                            "📦 快递取件码 (4-2-1086)"
                        } else {
                            "后台文字对话"
                        },
                        Action::AmooToggleExpressCard,
                    )
                } else {
                    Empty::new().finish()
                },
            ));
        }
        let top_cap_bar = Container::new(self.vstack(6., top_rows))
            .with_padding_bottom(6.)
            .with_border(Border::bottom(1.).with_border_color(p.border))
            .finish();

        let stream = if phone_mode {
            self.amoo_gui_records()
        } else if self.amoo_show_schedules {
            self.amoo_schedule_center()
        } else {
            self.amoo_conversation()
        };

        let mut bottom_stack: Vec<El> = Vec::new();
        if !phone_mode {
            if let Some(panel) = self.screen_text_panel() {
                bottom_stack.push(panel);
            }
        }

        if self.amoo_plus_open && !phone_mode {
            let pop_item =
                |id: &str, icon: UiIcon, color: ColorU, label: &'static str, act: Action| -> El {
                    let state = self.mouse_state(format!("amoo-pop:{id}"));
                    let row = self.hstack(
                        10.,
                        vec![
                            self.ui_icon(icon, 18., color),
                            self.line_w(label, CAPTION, p.text, Weight::Medium),
                        ],
                    );
                    Hoverable::new(state, move |mouse| {
                        Container::new(row)
                            .with_horizontal_padding(10.)
                            .with_vertical_padding(7.)
                            .with_background_color(if mouse.is_mouse_over_element() {
                                p.hover
                            } else {
                                ColorU::transparent_black()
                            })
                            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_CONTROL)))
                            .finish()
                    })
                    .with_cursor(Cursor::PointingHand)
                    .on_click(move |ctx, _, _| ctx.dispatch_typed_action(act.clone()))
                    .finish()
                };
            let popover = ConstrainedBox::new(
                Container::new(self.vstack(
                    2.,
                    vec![
                        pop_item(
                            "img",
                            UiIcon::Image,
                            p.blue,
                            "上传本地图片",
                            Action::AmooAttachFile(true),
                        ),
                        pop_item(
                            "doc",
                            UiIcon::Attach,
                            p.success,
                            "添加文档与附件",
                            Action::AmooAttachFile(false),
                        ),
                        pop_item(
                            "album",
                            UiIcon::Smartphone,
                            p.warning,
                            "从手机相册选取",
                            Action::AmooAttachPhoneAlbum,
                        ),
                    ],
                ))
                .with_uniform_padding(6.)
                .with_background_color(p.bg)
                .with_border(Border::all(1.).with_border_color(p.border))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
                .finish(),
            )
            .with_width(208.)
            .finish();
            bottom_stack.push(popover);
        }

        // Pending Attachments Strip inside Composer (driven by self.amoo_attachments)
        let att_chips: Vec<El> = self
            .amoo_attachments
            .iter()
            .enumerate()
            .map(|(idx, att)| {
                let (icon, icon_bg, icon_fg) = if att.is_image {
                    (UiIcon::Image, p.success, ColorU::white())
                } else {
                    (
                        UiIcon::File,
                        if p.dark {
                            rgb(0x27, 0x27, 0x2A)
                        } else {
                            rgb(0xEF, 0xEF, 0xF2)
                        },
                        p.text,
                    )
                };
                let rm_state = self.mouse_state(format!("amoo-att-rm:{idx}"));
                let rm_btn = Hoverable::new(rm_state, move |mouse| {
                    self.line(
                        "×",
                        CAPTION,
                        if mouse.is_mouse_over_element() {
                            p.danger
                        } else {
                            p.text3
                        },
                    )
                })
                .with_cursor(Cursor::PointingHand)
                .on_click(move |ctx, _, _| {
                    ctx.dispatch_typed_action(Action::AmooRemoveAttachment(idx))
                })
                .finish();

                Container::new(self.hstack(
                    6.,
                    vec![
                            ConstrainedBox::new(
                                Container::new(
                                    Flex::row()
                                        .with_main_axis_size(MainAxisSize::Max)
                                        .with_main_axis_alignment(MainAxisAlignment::Center)
                                        .with_cross_axis_alignment(CrossAxisAlignment::Center)
                                        .with_child(self.ui_icon(icon, 14., icon_fg))
                                        .finish(),
                                )
                                .with_background_color(icon_bg)
                                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
                                .finish(),
                            )
                            .with_width(20.)
                            .with_height(20.)
                            .finish(),
                            ConstrainedBox::new(self.line_w(
                                att.name.clone(),
                                CAPTION,
                                p.text,
                                Weight::Medium,
                            ))
                            .with_max_width(190.)
                            .finish(),
                            self.mono(att.size_label.clone(), 11., p.text2),
                            rm_btn,
                        ],
                ))
                .with_padding_left(4.)
                .with_padding_right(8.)
                .with_vertical_padding(4.)
                .with_background_color(p.surface)
                .with_border(Border::all(1.).with_border_color(p.surface_border))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_CONTROL)))
                .finish()
            })
            .collect();

        let composer_input_row = if self.amoo_recording {
            let wave_bars: Vec<El> = [
                6., 12., 16., 9., 14., 16., 10., 7., 13., 15., 11., 8., 14., 10.,
            ]
            .into_iter()
            .map(|h| {
                ConstrainedBox::new(
                    Rect::new()
                        .with_background_color(p.danger)
                        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(1.5)))
                        .finish(),
                )
                .with_width(2.)
                .with_height(h)
                .finish()
            })
            .collect();

            Container::new(self.between(
                self.hstack(
                    8.,
                    vec![
                        self.dot(p.danger),
                        self.mono(
                            format!("正在录音 00:{:02}", self.amoo_record_secs),
                            CAPTION,
                            p.danger,
                        ),
                        self.hstack(2.5, wave_bars),
                    ],
                ),
                self.hstack(
                    6.,
                    vec![
                        self.button(
                            "amoo-rec-cancel",
                            "取消",
                            Action::AmooCancelRecording,
                            Btn::Secondary,
                        ),
                        self.button(
                            "amoo-rec-send",
                            "发送语音",
                            Action::AmooSendRecording,
                            Btn::Primary,
                        ),
                    ],
                ),
            ))
            .with_horizontal_padding(8.)
            .with_vertical_padding(4.)
            .with_background_color(rgba(0xEF, 0x44, 0x44, 24))
            .with_border(Border::all(1.).with_border_color(rgba(0xEF, 0x44, 0x44, 76)))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
            .finish()
        } else {
            let plus_state = self.mouse_state("amoo-plus-circle");
            let plus_open = self.amoo_plus_open;
            let plus_btn = Hoverable::new(plus_state, move |mouse| {
                let hovered = mouse.is_mouse_over_element();
                let bg = if plus_open {
                    p.accent
                } else if hovered {
                    p.hover
                } else {
                    p.surface
                };
                let fg = if plus_open { p.accent_fg } else { p.text };
                let border = if plus_open {
                    p.accent
                } else {
                    p.surface_border
                };
                ConstrainedBox::new(
                    Container::new(
                        Flex::row()
                            .with_main_axis_size(MainAxisSize::Max)
                            .with_main_axis_alignment(MainAxisAlignment::Center)
                            .with_cross_axis_alignment(CrossAxisAlignment::Center)
                            .with_child(self.ui_icon(UiIcon::Plus, 16., fg))
                            .finish(),
                    )
                    .with_background_color(bg)
                    .with_border(Border::all(1.).with_border_color(border))
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(16.)))
                    .finish(),
                )
                .with_width(32.)
                .with_height(32.)
                .finish()
            })
            .with_cursor(Cursor::PointingHand)
            .on_click(|ctx, _, _| ctx.dispatch_typed_action(Action::AmooTogglePlus))
            .finish();

            let mic_btn = self.amoo_action_icon(
                "amoo-speech-input",
                if self.legacy_demo() {
                    "语音输入（演示）"
                } else {
                    "语音输入（暂未接入语音转文字）"
                },
                UiIcon::Mic,
                Action::AmooToggleRecording,
            );

            Flex::row()
                .with_main_axis_size(MainAxisSize::Max)
                .with_spacing(8.)
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_child(if phone_mode {
                    Empty::new().finish()
                } else {
                    plus_btn
                })
                .with_child(
                    Expanded::new(
                        1.,
                        self.input(
                            F_AMOO,
                            if phone_mode {
                                "描述手机上要完成的操作…"
                            } else {
                                "输入消息或添加图片、文档…"
                            },
                            None,
                        ),
                    )
                    .finish(),
                )
                .with_child(if self.legacy_demo() && !phone_mode {
                    mic_btn
                } else {
                    Empty::new().finish()
                })
                .with_child(
                    if phone_mode
                        && (self.amoo_ui.gui_running
                            || self.amoo_ui.gui_checking
                            || self.amoo_ui.gui_unconfirmed
                            || (!self.amoo_ui.gui_desktop
                                && self.amoo_ui.gui_unavailable_reason.is_some())
                            || self.busy
                            || self.work.running)
                    {
                        self.disabled_button(
                            if !self.amoo_ui.gui_desktop
                                && self.amoo_ui.gui_unavailable_reason.is_some()
                            {
                                "系统未开放"
                            } else if self.amoo_ui.gui_running {
                                "等待结果"
                            } else {
                                "开始操作"
                            },
                        )
                    } else {
                        self.button(
                            "amoo-send",
                            if phone_mode { "开始操作" } else { "发送" },
                            Action::AmooSend,
                            Btn::Primary,
                        )
                    },
                )
                .finish()
        };

        let mut composer_children = Vec::new();
        if !att_chips.is_empty() {
            composer_children.push(self.wrap(att_chips));
        }
        if !phone_mode && self.amoo_ui.gui_running {
            composer_children.push(self.hstack(8., vec![
                self.text(&self.amoo_ui.gui_message, MICRO, p.text2),
                self.button("amoo-chat-gui-pause", if self.amoo_ui.gui_paused {"继续"} else {"暂停"}, Action::AmooUi(AmooUiAction::GuiPauseResume), Btn::Secondary),
                self.button("amoo-chat-gui-stop", "停止操作", Action::AmooUi(AmooUiAction::GuiStopWaiting), Btn::Secondary),
            ]));
        } else if !phone_mode && !self.amoo_ui.gui_message.is_empty() {
            composer_children.push(self.text(&self.amoo_ui.gui_message, MICRO, p.text2));
        }
        composer_children.push(composer_input_row);
        if phone_mode {
            composer_children.push(self.text(
                if self.amoo_attachments.is_empty() {
                    "执行期间手机前台界面会变化，请保持解锁。不会自动通过锁屏或重发失败的操作。"
                } else {
                    "附件草稿已保留，请切回“后台对话”发送图片与文档；操作手机使用文字目标。"
                },
                MICRO,
                p.text3,
            ));
        } else {
            composer_children.push(self.amoo_usage_footer());
        }
        if !phone_mode && (self.amoo_plus_open || !self.amoo_attachments.is_empty()) {
            composer_children.push(self.text(
                "支持图片、文档与表格 · 最多 8 个文件 / 8 MB · 发送后同步到手机会话",
                MICRO,
                p.text3,
            ));
        }

        let docked_composer = Container::new(self.vstack(8., composer_children))
            .with_uniform_padding(10.)
            .with_background_color(p.bg)
            .with_border(Border::all(1.).with_border_color(p.border))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(16.)))
            .finish();

        bottom_stack.push(docked_composer);

        let mut center_children: Vec<El> = vec![top_cap_bar];

        if !phone_mode
            && !self.amoo_show_schedules
            && self
                .amoo_sessions
                .get(self.amoo_session_idx)
                .is_some_and(|session| session_waits_for_unlock(&session.items))
        {
            center_children.push(Container::new(self.vstack(5., vec![
                self.line_w("等待解锁", CAPTION, p.warning, Weight::Semibold),
                self.text("Amoo 最近的回复说明手机操作被锁屏阻止，任务尚未完成。请手动解锁手机后再下达操作；桌面不会自动重试。", CAPTION, p.text2),
            ])).with_uniform_padding(S3).with_background_color(p.surface)
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_CONTROL))).finish());
        }

        if !phone_mode && self.amoo_express_open && !self.amoo_lightbox_open {
            let mut parcel_cols: Vec<El> = Vec::new();
            for (p_idx, parcel) in self.express_parcels.iter().enumerate() {
                let code = parcel.pickup_code.clone();
                let copy_str = format!(
                    "{} · 运单 {} · 取件码 {} ({})",
                    parcel.carrier, parcel.tracking_no, parcel.pickup_code, parcel.station
                );
                let short_station: String = parcel.station.chars().take(9).collect();
                let cell = Container::new(self.vstack(
                    3.,
                    vec![
                        self.between(
                            self.line_w(&parcel.carrier, CAPTION, p.text, Weight::Semibold),
                            self.pill_badge(&parcel.status, p.success_text, p.success_soft),
                        ),
                        self.hstack(
                            6.,
                            vec![
                                self.line("取件码:", MICRO, p.text2),
                                self.mono(&parcel.pickup_code, SUBHEAD, p.accent),
                                self.line(format!("· {short_station}"), MICRO, p.text2),
                            ],
                        ),
                        self.between(
                            self.mono(format!("单号 {}", parcel.tracking_no), MICRO, p.text3),
                            self.hstack(
                                6.,
                                vec![
                                    self.link_action(
                                        format!("exp-copy-{p_idx}"),
                                        "复制",
                                        Action::CopyText(
                                            copy_str,
                                            "已复制快递取件码与驿站位置".into(),
                                        ),
                                    ),
                                    self.link_action(
                                        format!("exp-push-{p_idx}"),
                                        "❖ 投背屏",
                                        Action::AmooPushPickupCode(code),
                                    ),
                                ],
                            ),
                        ),
                    ],
                ))
                .with_horizontal_padding(9.)
                .with_vertical_padding(5.)
                .with_background_color(p.surface)
                .with_border(Border::all(1.).with_border_color(p.surface_border))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_CONTROL)))
                .finish();
                parcel_cols.push(Expanded::new(1., cell).finish());
            }

            let express_card = Container::new(self.vstack(
                5.,
                vec![
                    self.between(
                        self.hstack(
                            6.,
                            vec![
                                self.line_w(
                                    "📦 PC 端快速获取快递 · 实时物流与取件码",
                                    CAPTION,
                                    p.text,
                                    Weight::Semibold,
                                ),
                                self.pill_badge("已同步手机短信 & Amoo", p.accent, p.accent_soft),
                            ],
                        ),
                        self.link_muted(
                            "exp-card-hide",
                            "收起卡片",
                            Action::AmooToggleExpressCard,
                        ),
                    ),
                    Flex::row()
                        .with_main_axis_size(MainAxisSize::Max)
                        .with_spacing(8.)
                        .with_children(parcel_cols)
                        .finish(),
                ],
            ))
            .with_horizontal_padding(10.)
            .with_vertical_padding(7.)
            .with_background_color(p.bg)
            .with_border(Border::all(1.).with_border_color(p.border))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(12.)))
            .finish();

            center_children.push(Container::new(express_card).with_padding_top(6.).finish());
        }

        center_children.push(
            Expanded::new(
                1.,
                Container::new(self.pane_scroll(self.scroll.clone(), stream))
                    .with_padding_top(8.)
                    .with_padding_bottom(6.)
                    .finish(),
            )
            .finish(),
        );
        center_children.push(self.vstack(6., bottom_stack));

        let center_col = Container::new(
            Flex::column()
                .with_main_axis_size(MainAxisSize::Max)
                .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                .with_children(center_children)
                .finish(),
        )
        .with_uniform_padding(S4)
        .with_border(Border::right(1.).with_border_color(p.border))
        .finish();

        // 3. Col 3 (3 cols): Pure Amoo Space (Memory, Skills & Safety)
        let mem_item = |idx: usize, title: String, sub: String| -> El {
            let state = self.mouse_state(format!("amoo-mem-item:{idx}"));
            let t_clone = title.clone();
            let content = self.vstack(
                2.,
                vec![
                    self.line_w(title, CAPTION, p.text, Weight::Medium),
                    self.line(sub, 11., p.text2),
                ],
            );
            Hoverable::new(state, move |mouse| {
                Container::new(content)
                    .with_uniform_padding(8.)
                    .with_background_color(if mouse.is_mouse_over_element() {
                        p.hover
                    } else {
                        p.surface
                    })
                    .with_border(Border::all(1.).with_border_color(p.surface_border))
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_CONTROL)))
                    .finish()
            })
            .with_cursor(Cursor::PointingHand)
            .on_click(move |ctx, _, _| {
                ctx.dispatch_typed_action(Action::AmooUseMemory(t_clone.clone()))
            })
            .finish()
        };

        let clickable_pref_row =
            |id: &str, k: &'static str, v: &'static str, v_fg: ColorU, act: Action| -> El {
                let state = self.mouse_state(id);
                let row = self.between(
                    self.line(k, CAPTION, p.text2),
                    self.line_w(v, CAPTION, v_fg, Weight::Medium),
                );
                Hoverable::new(state, move |_| row)
                    .with_cursor(Cursor::PointingHand)
                    .on_click(move |ctx, _, _| ctx.dispatch_typed_action(act.clone()))
                    .finish()
            };

        let style_label = match self.amoo_style_idx {
            0 => "直接精炼 · 不长篇解释",
            1 => "温和详细 · 分步拆解",
            _ => "极客专业 · 附带命令",
        };
        let commute_label = match self.amoo_commute_idx {
            0 => "08:30 播报路况",
            1 => "09:00 播报日程",
            _ => "18:30 晚高峰提醒",
        };

        let mem_count_state = self.mouse_state("amoo-mem-badge");
        let mem_total = if self.legacy_demo() {
            26 + self.amoo_memories.len()
        } else {
            self.amoo_bm25_entries.max(self.amoo_memories.len())
        };
        let mem_badge = Hoverable::new(mem_count_state, move |_| {
            Container::new(self.line_w(
                if self.legacy_demo() || mem_total > 0 {
                    format!("{mem_total} 条")
                } else {
                    "未同步".into()
                },
                11.,
                p.success_text,
                Weight::Medium,
            ))
            .with_horizontal_padding(6.)
            .with_vertical_padding(2.)
            .with_background_color(p.success_soft)
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
            .finish()
        })
        .with_cursor(Cursor::PointingHand)
        .on_click(|ctx, _, _| ctx.dispatch_typed_action(Action::AmooOpenMemory))
        .finish();

        let filtered_memories: Vec<(usize, &super::super::AmooMemoryEntry)> = self
            .amoo_memories
            .iter()
            .enumerate()
            .filter(|(_, m)| {
                query.is_empty()
                    || m.title.to_lowercase().contains(&query)
                    || m.sub.to_lowercase().contains(&query)
            })
            .take(2)
            .collect();

        let mut mem_list_els = vec![self.between(
            self.line("为你记录的重要内容", 11., p.text2),
            self.link_muted("amoo-add-mem", "＋ 记录", Action::AmooAddMemory),
        )];
        for (i, m) in filtered_memories {
            mem_list_els.push(mem_item(i, m.title.clone(), m.sub.clone()));
        }

        let memory_card = Container::new(self.vstack(
            8.,
            vec![
                self.between(
                    self.hstack(
                        6.,
                        vec![
                            self.line("💡", CAPTION, p.success),
                            self.heading("Amoo 记忆与偏好"),
                        ],
                    ),
                    mem_badge,
                ),
                self.vstack(
                    5.,
                    vec![
                        clickable_pref_row(
                            "amoo-pref-mem",
                            "跨会话记忆",
                            if !self.legacy_demo() {
                                "未同步"
                            } else if self.amoo_memory_enabled {
                                "已开启"
                            } else {
                                "已关闭"
                            },
                            if self.amoo_memory_enabled {
                                p.success
                            } else {
                                p.text2
                            },
                            Action::AmooToggleMemory,
                        ),
                        clickable_pref_row(
                            "amoo-pref-style",
                            "交流风格",
                            if self.legacy_demo() { style_label } else { "未同步" },
                            p.text,
                            Action::AmooCycleStyle,
                        ),
                        clickable_pref_row(
                            "amoo-pref-commute",
                            "通勤偏好",
                            if self.legacy_demo() { commute_label } else { "未同步" },
                            p.text,
                            Action::AmooCycleCommute,
                        ),
                    ],
                ),
                Container::new(self.vstack(6., mem_list_els))
                    .with_padding_top(8.)
                    .with_border(Border::top(1.).with_border_color(p.border))
                    .finish(),
            ],
        ))
        .with_uniform_padding(12.)
        .with_background_color(p.bg)
        .with_border(Border::all(1.).with_border_color(p.border))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
        .finish();

        let skill_colors = [p.blue, p.success, p.indigo, p.warning, p.pink, p.accent];
        let skill_pill = |idx: usize| -> El {
            let entry = &self.amoo_skills[idx];
            let enabled = entry.enabled;
            let dot_color = if enabled {
                skill_colors[idx % skill_colors.len()]
            } else {
                p.text3
            };
            let fg = if enabled { p.text } else { p.text3 };
            let name = entry.name.clone();
            let state = self.mouse_state(format!("amoo-skill-pill:{idx}"));
            Expanded::new(
                1.,
                Hoverable::new(state, move |mouse| {
                    Container::new(self.lead(
                        6.,
                        self.dot(dot_color),
                        self.line_w(name.clone(), CAPTION, fg, Weight::Medium),
                    ))
                    .with_horizontal_padding(10.)
                    .with_vertical_padding(6.)
                    .with_background_color(if mouse.is_mouse_over_element() {
                        p.hover
                    } else {
                        p.surface
                    })
                    .with_border(Border::all(1.).with_border_color(p.surface_border))
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_CONTROL)))
                    .finish()
                })
                .with_cursor(Cursor::PointingHand)
                .on_click(move |ctx, _, _| ctx.dispatch_typed_action(Action::AmooClickSkill(idx)))
                .finish(),
            )
            .finish()
        };

        let enabled_skills_count = if self.legacy_demo() {
            2 + self.amoo_skills.iter().filter(|s| s.enabled).count()
        } else {
            self.amoo_skills.iter().filter(|s| s.enabled).count()
        };

        let mut skill_rows: Vec<El> = vec![self.between(
            self.heading(if self.legacy_demo() || !self.amoo_skills.is_empty() {
                format!("已启用技能 ({enabled_skills_count})")
            } else {
                "技能尚未同步".into()
            }),
            self.link_muted(
                "amoo-skill-mgr",
                if self.amoo_skill_manage {
                    "完成"
                } else {
                    "管理"
                },
                Action::AmooToggleSkillManage,
            ),
        )];
        let skill_len = self.amoo_skills.len().min(4);
        let mut s_idx = 0;
        while s_idx < skill_len {
            let left_pill = skill_pill(s_idx);
            let right_pill = if s_idx + 1 < skill_len {
                skill_pill(s_idx + 1)
            } else {
                Expanded::new(1., Empty::new().finish()).finish()
            };
            skill_rows.push(
                Flex::row()
                    .with_main_axis_size(MainAxisSize::Max)
                    .with_spacing(6.)
                    .with_child(left_pill)
                    .with_child(right_pill)
                    .finish(),
            );
            s_idx += 2;
        }

        let skills_card = Container::new(self.vstack(8., skill_rows))
            .with_uniform_padding(12.)
            .with_background_color(p.bg)
            .with_border(Border::all(1.).with_border_color(p.border))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
            .finish();

        let policy_label = match self.amoo_safety_policy_idx {
            0 => "始终确认",
            1 => "仅高危询问",
            _ => "严格拦截",
        };

        let safety_card = Container::new(self.vstack(
            8.,
            vec![
                self.between(
                    self.heading("安全守护"),
                    self.line_w(
                        format!("今日守护 {} 次", self.amoo_guard_count),
                        11.,
                        p.success,
                        Weight::Medium,
                    ),
                ),
                clickable_pref_row(
                    "amoo-safety-policy",
                    "敏感操作前询问",
                    policy_label,
                    p.text,
                    Action::AmooCycleSafetyPolicy,
                ),
                Flex::row()
                    .with_main_axis_size(MainAxisSize::Max)
                    .with_spacing(6.)
                    .with_child(
                        Expanded::new(
                            1.,
                            self.button(
                                "amoo-safety-undo",
                                "撤回操作",
                                Action::AmooUndoSafety,
                                Btn::Secondary,
                            ),
                        )
                        .finish(),
                    )
                    .with_child(
                        Expanded::new(
                            1.,
                            self.button(
                                "amoo-mem-export",
                                "导出记忆",
                                Action::AmooExportMemory,
                                Btn::Secondary,
                            ),
                        )
                        .finish(),
                    )
                    .finish(),
            ],
        ))
        .with_uniform_padding(12.)
        .with_background_color(p.bg)
        .with_border(Border::all(1.).with_border_color(p.border))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
        .finish();

        let engine_title = match self.amoo_agent_idx {
            0 => {
                if self.legacy_demo() {
                    "端侧 Agent 引擎"
                } else {
                    "Amoo 后台连接"
                }
            }
            1 => "桌面跨端协同引擎",
            2 => "飞书 / 企微网关",
            _ => "Claude / OpenClaw",
        };
        let engine_ver_label = if self.legacy_demo() {
            "v1.0.7 就绪".to_string()
        } else {
            let short_ver = self
                .amoo_gateway_version
                .split('-')
                .next()
                .unwrap_or(&self.amoo_gateway_version);
            format!(
                "v{short_ver} {}",
                if self.amoo_gateway_online {
                    "在线"
                } else {
                    "未连接"
                }
            )
        };
        let engine_state = self.mouse_state("amoo-engine-bar");
        let engine_bar = Hoverable::new(engine_state, move |mouse| {
            Container::new(self.between(
                self.line(engine_title, CAPTION, p.text2),
                self.mono(engine_ver_label.clone(), CAPTION, p.success),
            ))
            .with_horizontal_padding(12.)
            .with_vertical_padding(8.)
            .with_background_color(if mouse.is_mouse_over_element() {
                p.hover
            } else {
                p.bg
            })
            .with_border(Border::all(1.).with_border_color(p.border))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
            .finish()
        })
        .with_cursor(Cursor::PointingHand)
        .on_click(|ctx, _, _| ctx.dispatch_typed_action(Action::AmooOpenModels))
        .finish();

        let pc_ctrl_card = Container::new(self.vstack(
            6.,
            vec![
                self.between(
                    self.hstack(
                        6.,
                        vec![
                            self.line_w("💻 手机控电脑 & 常驻", CAPTION, p.text, Weight::Semibold),
                            self.pill_badge(
                                if self.amoo_pc_control_enabled {
                                    "双向远控已开启"
                                } else {
                                    "已暂停"
                                },
                                p.success_text,
                                p.success_soft,
                            ),
                        ],
                    ),
                    self.link_action(
                        "amoo-pc-toggle",
                        if self.amoo_pc_control_enabled {
                            "暂停"
                        } else {
                            "开启"
                        },
                        Action::AmooTogglePcControl,
                    ),
                ),
                self.line(
                    "手机端对 Amoo 说话可截取电脑屏幕、同步剪贴板、静音或锁屏（Root 保活 oom -800）",
                    10.5,
                    p.text2,
                ),
                Flex::row()
                    .with_main_axis_size(MainAxisSize::Max)
                    .with_spacing(6.)
                    .with_child(
                        Expanded::new(
                            1.,
                            self.button(
                                "pc-cmd-shot",
                                "🖥 截屏发手机",
                                Action::AmooExecPcCommand("screenshot_to_phone".into()),
                                Btn::Secondary,
                            ),
                        )
                        .finish(),
                    )
                    .with_child(
                        Expanded::new(
                            1.,
                            self.button(
                                "pc-cmd-clip",
                                "📋 同步剪贴板",
                                Action::AmooExecPcCommand("sync_clipboard".into()),
                                Btn::Secondary,
                            ),
                        )
                        .finish(),
                    )
                    .finish(),
                Flex::row()
                    .with_main_axis_size(MainAxisSize::Max)
                    .with_spacing(6.)
                    .with_child(
                        Expanded::new(
                            1.,
                            self.button(
                                "pc-cmd-mute",
                                "🔊 电脑静音",
                                Action::AmooExecPcCommand("pc_mute_toggle".into()),
                                Btn::Secondary,
                            ),
                        )
                        .finish(),
                    )
                    .with_child(
                        Expanded::new(
                            1.,
                            self.button(
                                "pc-cmd-lock",
                                "🔒 远程锁屏",
                                Action::AmooExecPcCommand("pc_lock_screen".into()),
                                Btn::Secondary,
                            ),
                        )
                        .finish(),
                    )
                    .finish(),
            ],
        ))
        .with_uniform_padding(10.)
        .with_background_color(p.bg)
        .with_border(Border::all(1.).with_border_color(p.border))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
        .finish();

        let right_col = Container::new(
            Flex::column()
                .with_main_axis_size(MainAxisSize::Max)
                .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
                .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                .with_child(self.vstack(
                    7.,
                    vec![pc_ctrl_card, memory_card, skills_card, safety_card],
                ))
                .with_child(Container::new(engine_bar).with_padding_top(6.).finish())
                .finish(),
        )
        .with_uniform_padding(12.)
        .with_background_color(if p.dark { p.sidebar } else { p.surface })
        .finish();

        let right_col = if self.legacy_demo() {
            right_col
        } else {
            let current = self.amoo_sessions.get(self.amoo_session_idx);
            let mut detail_parts = vec![
                self.heading("会话详情"),
                self.text(
                    current
                        .map(|s| session_title(&s.title))
                        .unwrap_or_else(|| "尚未选择会话".into()),
                    BODY,
                    p.text,
                ),
            ];
            if let Some(session) = current {
                detail_parts.push(self.text(&session.meta, CAPTION, p.text2));
                detail_parts.push(self.line(format!("状态：{}", session.status), CAPTION, p.text2));
            }
            detail_parts.push(self.divider());
            detail_parts.push(self.hstack(
                S2,
                vec![
                    self.dot(if self.amoo_gateway_online {
                        p.success
                    } else {
                        p.text3
                    }),
                    self.line(
                        if self.amoo_gateway_online {
                            "手机 Amoo 已连接"
                        } else {
                            "手机 Amoo 未连接"
                        },
                        CAPTION,
                        p.text,
                    ),
                ],
            ));
            detail_parts.push(self.text(
                "文字、图片和文档由手机 Amoo 处理，并同步到当前会话。",
                CAPTION,
                p.text2,
            ));
            detail_parts.push(self.button(
                "amoo-details-schedules",
                "查看定时任务",
                Action::AmooOpenTasks,
                Btn::Secondary,
            ));
            detail_parts.push(self.divider());
            detail_parts.push(self.heading("更多能力"));
            detail_parts.push(self.text(
                "需要处理工作文件、生成交付物时，进入 Amoo Work。",
                CAPTION,
                p.text2,
            ));
            detail_parts.push(self.button(
                "amoo-details-work",
                "打开 Amoo Work",
                Action::Work(super::super::WorkAction::Tab(0)),
                Btn::Secondary,
            ));
            detail_parts.push(self.button(
                "amoo-details-skills",
                "浏览与安装技能",
                Action::Work(super::super::WorkAction::Tab(2)),
                Btn::Ghost,
            ));
            detail_parts.push(self.button(
                "amoo-details-models",
                "查看可用模型",
                Action::Work(super::super::WorkAction::Tab(1)),
                Btn::Ghost,
            ));
            Container::new(self.vstack(S3, detail_parts))
                .with_uniform_padding(S4)
                .with_background_color(p.surface)
                .with_border(Border::left(1.).with_border_color(p.border))
                .finish()
        };
        let mut layout = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_child(ConstrainedBox::new(left_col).with_width(260.).finish())
            .with_child(Expanded::new(1., center_col).finish());
        if self.legacy_demo() || self.amoo_show_details {
            layout = layout.with_child(ConstrainedBox::new(right_col).with_width(286.).finish());
        }
        layout.finish()
    }

    fn amoo_schedule_center(&self) -> El {
        let p = self.p();
        let enabled = self.amoo_cron_jobs.iter().filter(|job| job.enabled).count();
        let mut sections = vec![
            self.between(
                self.hstack(S2, vec![
                    self.ui_icon(UiIcon::Calendar, 22., p.text),
                    self.heading("定时任务"),
                    self.pill_badge(
                        format!("{} 项 · {} 项已启用", self.amoo_cron_jobs.len(), enabled),
                        p.text2,
                        p.surface,
                    ),
                ]),
                self.button("amoo-schedule-refresh", "刷新", Action::AmooSnapshot, Btn::Secondary),
            ),
            self.text(
                "查看手机 Amoo 的任务安排与执行结果。进入任务会话，可以继续对话、修改要求或查看历史回复。",
                CAPTION,
                p.text2,
            ),
            self.wrap(vec![
                self.button(
                    "amoo-schedule-create",
                    "新建定时任务",
                    Action::AmooTemplate("请帮我创建一个定时任务：\n任务内容：\n执行时间和重复规则：\n请先确认安排，再创建任务。".into()),
                    Btn::Primary,
                ),
                self.button("amoo-schedule-back", "返回对话", Action::AmooToggleSchedules, Btn::Secondary),
            ]),
        ];
        if self.amoo_cron_jobs.is_empty() {
            sections.push(
                Container::new(self.vstack(
                    S3,
                    vec![
                        self.ui_icon(UiIcon::Clock, 32., p.text3),
                        self.heading(if self.busy {
                            "正在同步定时任务…"
                        } else if !self.amoo_gateway_online {
                            "连接手机后查看定时任务"
                        } else {
                            "还没有定时任务"
                        }),
                        self.text(
                            if self.amoo_gateway_online {
                                "例如：每天早上 8 点汇总 AI 新闻，或每周五提醒我整理工作总结。"
                            } else {
                                "保持手机连接，点击刷新即可从 Amoo 获取任务。"
                            },
                            CAPTION,
                            p.text2,
                        ),
                    ],
                ))
                .with_uniform_padding(S5)
                .with_background_color(p.surface)
                .with_border(Border::all(1.).with_border_color(p.border))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
                .finish(),
            );
        } else {
            let query = self.fields[F_AMOO_SEARCH].text.trim().to_lowercase();
            let mut count = 0;
            for (index, job) in self.amoo_cron_jobs.iter().enumerate() {
                if !query.is_empty()
                    && !job.name.to_lowercase().contains(&query)
                    && !job.schedule.to_lowercase().contains(&query)
                {
                    continue;
                }
                count += 1;
                let session = self
                    .amoo_sessions
                    .iter()
                    .find(|s| s.session_key == job.session_key);
                let name = if job.name.trim().is_empty() {
                    session.map(|s| s.title.as_str()).unwrap_or("定时任务")
                } else {
                    &job.name
                };
                let mut parts = vec![
                    self.between(
                        self.hstack(
                            S2,
                            vec![
                                self.ui_icon(UiIcon::Clock, 18., p.text2),
                                self.heading(session_title(name)),
                            ],
                        ),
                        self.pill_badge(
                            if job.enabled {
                                "已启用"
                            } else {
                                match job.status.as_str() {
                                    "paused" | "disabled" | "inactive" => "已暂停",
                                    "done" | "completed" => "已完成",
                                    "error" | "failed" => "执行失败",
                                    "" => "状态未返回",
                                    status => status,
                                }
                            },
                            if job.enabled { p.success_text } else { p.text3 },
                            if job.enabled {
                                p.success_soft
                            } else {
                                p.surface
                            },
                        ),
                    ),
                    self.text(
                        if job.schedule.trim().is_empty() {
                            "手机尚未提供执行时间"
                        } else {
                            &job.schedule
                        },
                        BODY,
                        p.text,
                    ),
                ];
                if let Some(session) = session {
                    parts.push(self.line(
                        format!("最近会话状态：{}", session.status),
                        MICRO,
                        p.text3,
                    ));
                }
                if !job.session_key.is_empty() {
                    parts.push(self.wrap(vec![self.button(
                        format!("amoo-schedule-session-{index}"),
                        "查看任务与执行记录",
                        Action::AmooOpenScheduledSession(job.session_key.clone()),
                        Btn::Secondary,
                    )]));
                }
                sections.push(
                    Container::new(self.vstack(S3, parts))
                        .with_uniform_padding(S4)
                        .with_background_color(p.bg)
                        .with_border(Border::all(1.).with_border_color(p.border))
                        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
                        .finish(),
                );
            }
            if count == 0 {
                sections.push(self.text("没有匹配的定时任务，试试其他关键词。", CAPTION, p.text3));
            }
        }
        self.vstack(S4, sections)
    }

    fn amoo_gui_controls(&self) -> El {
        let p = self.p();
        let state = &self.amoo_ui;
        let locked = state
            .gui_status
            .as_ref()
            .is_some_and(|s| s.locked != Some(false));
        let status = if state.gui_checking {
            "检查中"
        } else if state.gui_running {
            "等待手机返回"
        } else if !state.gui_desktop && state.gui_unavailable_reason.is_some() {
            "当前手机系统未开放屏幕自动操作"
        } else if state.gui_unconfirmed {
            "需确认手机状态"
        } else if let Some(status) = &state.gui_status {
            if !status.supported && !state.gui_desktop {
                "未发现操作接口"
            } else if status.locked != Some(false) {
                "请手动解锁"
            } else if status.interactive != Some(true) {
                "请点亮屏幕"
            } else if state.gui_desktop {
                "已解锁 · 开始前会再次检查"
            } else {
                "已解锁 · 接口待验证"
            }
        } else {
            "以手机实际返回为准"
        };
        self.vstack(
            6.,
            vec![
                self.hstack(6., [false, true].into_iter().map(|desktop| {
                    let label = if desktop { "GELab-Zero Agent" } else { "手机原生" };
                    if state.gui_running || state.gui_unconfirmed {
                        self.disabled_button(label)
                    } else {
                        self.button(format!("amoo-gui-method:{desktop}"), label,
                            Action::AmooUi(AmooUiAction::GuiDesktopMode(desktop)),
                            if state.gui_desktop == desktop { Btn::Secondary } else { Btn::Ghost })
                    }
                }).collect()),
                self.text(if state.gui_desktop {
                    "手机端 GELab-Zero 适配器使用 Step Plan 视觉模型独立规划、操作与核验。关闭 desktop 或断开连接后，任务仍由手机继续执行。"
                } else {
                    "使用手机系统提供的原生执行器；是否开放以手机实际返回为准。"
                }, MICRO, p.text2),
                Flex::row()
                    .with_main_axis_size(MainAxisSize::Max)
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_spacing(8.)
                    .with_child(self.ui_icon(UiIcon::Smartphone, 16., p.text2))
                    .with_child(
                        Expanded::new(
                            1.,
                            self.input(F_AMOO_GUI_APP, "手机应用名称，例如：设置、天气", None),
                        )
                        .finish(),
                    )
                    .with_child(if state.gui_checking || state.gui_running {
                        self.disabled_button("检查手机")
                    } else {
                        self.button(
                            "amoo-gui-check",
                            if locked {
                                "解锁后检查"
                            } else {
                                "检查手机"
                            },
                            Action::AmooUi(AmooUiAction::GuiRefresh),
                            Btn::Secondary,
                        )
                    })
                    .finish(),
                self.button("amoo-gui-enable-tools", "查看手机 Agent 状态", Action::AmooUi(AmooUiAction::GuiEnableTools), Btn::Secondary),
                self.hstack(6., [("pause", "暂停手机任务"), ("resume", "继续手机任务"), ("stop", "停止手机任务")].into_iter().map(|(op,label)| self.button(format!("gui-phone-{op}"),label,Action::AmooUi(AmooUiAction::GuiPhoneControl(op.into())),Btn::Ghost)).collect()),
                self.line(
                    status,
                    MICRO,
                    if locked || state.gui_unconfirmed {
                        p.warning
                    } else {
                        p.text3
                    },
                ),
            ],
        )
    }

    fn amoo_gui_step_list(&self, steps: &[(usize, String)], key: &str) -> El {
        let p = self.p();
        let expanded = self.amoo_expanded_tool_records.contains(key);
        let mut rows = vec![self.link_muted(
            format!("{key}-toggle"),
            format!(
                "{} 执行记录 · {} 步",
                if expanded { "收起" } else { "展开" },
                steps.len()
            ),
            Action::AmooToggleToolRecords(key.into()),
        )];
        if expanded {
            rows.extend(steps.iter().map(|(number, detail)| {
                self.text(format!("{number}. {detail}"), CAPTION, p.text2)
            }));
        } else if let Some((number, detail)) = steps.last() {
            let short: String = detail.chars().take(140).collect();
            rows.push(self.text(
                format!(
                    "{number}. {short}{}",
                    if detail.chars().count() > 140 {
                        "…"
                    } else {
                        ""
                    }
                ),
                CAPTION,
                p.text2,
            ));
        }
        self.vstack(5., rows)
    }

    fn amoo_gui_screenshot(&self, path: &std::path::Path) -> El {
        let p = self.p();
        self.vstack(
            5.,
            vec![
                self.line("最近一次手机画面 · 已缓存在本次操作中", MICRO, p.text3),
                ConstrainedBox::new(
                    Image::new(
                        AssetSource::LocalFile {
                            path: path.to_string_lossy().into_owned(),
                        },
                        CacheOption::BySize,
                    )
                    .contain()
                    .finish(),
                )
                .with_height(220.)
                .with_max_width(180.)
                .finish(),
            ],
        )
    }

    fn amoo_gui_records(&self) -> El {
        let p = self.p();
        let state = &self.amoo_ui;
        let mut rows = Vec::new();
        if state.gui_records.is_empty() && !state.gui_running {
            rows.push(Container::new(self.vstack(10., vec![
                self.hstack(8., vec![self.ui_icon(UiIcon::Smartphone, 22., p.text), self.heading("让 Amoo 操作手机应用")]),
                self.text(if state.gui_desktop {
                    "桌面会把真实手机画面交给 Amoo 分析，逐步执行导航并重新截图检查。支持连续导航、中文搜索和跨页面任务，遇到问题会重新观察并调整操作。"
                } else {
                    "填写手机应用名称和具体目标，Amoo 将请求手机原生能力查看界面并执行操作。是否开放取决于当前手机系统的实际支持情况。"
                }, BODY, p.text2),
                self.text("需要手机保持亮屏并由你解锁。这里单独保留本次桌面的操作记录，不会显示为已同步到手机的聊天消息。", CAPTION, p.text3),
            ])).with_uniform_padding(S4).with_background_color(p.surface)
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL))).finish());
        }
        if !state.gui_message.is_empty() {
            let mut message = vec![self.text(
                &state.gui_message,
                CAPTION,
                if state.gui_unconfirmed {
                    p.warning
                } else {
                    p.text2
                },
            )];
            if state.gui_running {
                if let Some((app, task)) = &state.gui_current {
                    message.insert(0, self.heading(format!("正在请求 · {app}")));
                    message.insert(1, self.text(task, BODY, p.text));
                }
                let elapsed = state
                    .gui_started
                    .map(|t| t.elapsed().as_secs())
                    .unwrap_or(0);
                message.push(self.between(
                    self.line(
                        format!(
                            "已等待 {}分{:02}秒 · 最长 20 分钟",
                            elapsed / 60,
                            elapsed % 60
                        ),
                        MICRO,
                        p.text3,
                    ),
                    self.button(
                        "amoo-gui-stop-wait",
                        if state.gui_desktop {
                            "停止操作"
                        } else {
                            "停止等待"
                        },
                        Action::AmooUi(AmooUiAction::GuiStopWaiting),
                        Btn::Secondary,
                    ),
                ));
                message.push(self.text(
                    if state.gui_desktop {
                        "停止后不再执行下一步操作，当前步骤收尾后会显示结果。"
                    } else {
                        "停止等待只关闭桌面等待，手机上的操作可能继续。"
                    },
                    MICRO,
                    p.text3,
                ));
                if state.gui_desktop {
                    message.push(self.button("amoo-gui-pause", if state.gui_paused { "继续任务" } else { "暂停任务" }, Action::AmooUi(AmooUiAction::GuiPauseResume), Btn::Secondary));
                }
                if !state.gui_steps.is_empty() {
                    message.push(self.amoo_gui_step_list(&state.gui_steps, "amoo-gui-live-steps"));
                }
                if let Some(path) = &state.gui_screenshot {
                    message.push(self.amoo_gui_screenshot(path));
                }
            }
            if state.gui_unconfirmed {
                message.push(self.text("先在手机上查看并结束上一次操作。检查手机只能读取锁屏与接口状态，不能证明任务已结束。", CAPTION, p.text2));
                message.push(self.button(
                    "amoo-gui-ack-finished",
                    "已在手机确认结束",
                    Action::AmooUi(AmooUiAction::GuiAcknowledgeFinished),
                    Btn::Secondary,
                ));
            }
            if !state.gui_desktop && state.gui_unavailable_reason.is_some() {
                message.push(self.text("当前手机系统拒绝了原生屏幕自动操作，刷新接口或再次提交无法解除限制。可以先通过投屏手动查看手机。", CAPTION, p.text2));
            }
            if (state.gui_unconfirmed
                || (!state.gui_desktop && state.gui_unavailable_reason.is_some()))
                && !self.busy
            {
                message.push(self.button(
                    "amoo-gui-open-mirror",
                    "投屏查看手机",
                    Action::Mirror,
                    Btn::Secondary,
                ));
            }
            rows.push(
                Container::new(self.vstack(8., message))
                    .with_uniform_padding(S3)
                    .with_background_color(p.bg)
                    .with_border(Border::all(1.).with_border_color(if state.gui_unconfirmed {
                        p.warning
                    } else {
                        p.border
                    }))
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
                    .finish(),
            );
        }
        for (index, record) in state.gui_records.iter().enumerate().rev() {
            let result = &record.result;
            let (label, color) = match result.state {
                GuiRunState::Completed => (
                    if record.desktop {
                        "模型报告完成"
                    } else {
                        "原生报告完成"
                    },
                    p.success,
                ),
                GuiRunState::Failed => ("执行失败", p.danger),
                GuiRunState::Submitted => ("已接收 · 未确认完成", p.warning),
                GuiRunState::WaitingForUnlock => ("等待手动解锁", p.warning),
                GuiRunState::ScreenOff => ("等待点亮屏幕", p.warning),
                GuiRunState::Unavailable => ("手机暂未开放", p.warning),
                GuiRunState::CancelledBeforeStart => ("发送前已取消", p.text3),
                GuiRunState::WaitingCancelled => (
                    if record.desktop {
                        "已停止后续步骤"
                    } else {
                        "已停止桌面等待"
                    },
                    p.warning,
                ),
                GuiRunState::TimedOut => ("等待超时", p.warning),
                GuiRunState::Disconnected => ("连接中断", p.warning),
                GuiRunState::Unknown => ("结果待确认", p.warning),
            };
            let mut content = vec![
                self.between(
                    self.line_w(&record.app_name, BODY, p.text, Weight::Semibold),
                    self.line(label, MICRO, color),
                ),
                self.text(&record.task, BODY, p.text),
                self.text(&result.message, CAPTION, p.text2),
            ];
            if !record.steps.is_empty() {
                content.push(
                    self.amoo_gui_step_list(&record.steps, &format!("amoo-gui-steps:{index}")),
                );
            }
            if let Some(path) = &record.screenshot {
                content.push(self.amoo_gui_screenshot(path));
            }
            if let Some(raw) = &result.raw_result {
                let key = format!("amoo-gui-result:{index}:{}", result.invocation_id);
                let expanded = self.amoo_expanded_tool_records.contains(&key);
                let raw = serde_json::to_string_pretty(raw).unwrap_or_default();
                content.push(self.between(
                    self.link_muted(
                        format!("{key}-toggle"),
                        if expanded {
                            "收起执行详情"
                        } else {
                            "查看执行详情"
                        },
                        Action::AmooToggleToolRecords(key.clone()),
                    ),
                    self.amoo_action_icon(
                        format!("{key}-copy"),
                        "复制执行结果",
                        UiIcon::Copy,
                        Action::CopyText(raw.clone(), "已复制执行结果".into()),
                    ),
                ));
                if expanded {
                    let visible: String = raw.chars().take(20000).collect();
                    content.push(self.text(visible, MICRO, p.text2));
                    if raw.chars().count() > 20000 {
                        content.push(self.text(
                            "预览已截取前 20000 字；复制可获取完整结果。",
                            MICRO,
                            p.text3,
                        ));
                    }
                }
            }
            content.push(self.line(
                format!(
                    "{} · 耗时 {:.1} 秒 · 本次桌面记录",
                    if record.desktop {
                        "Amoo GUI Agent"
                    } else {
                        "手机原生"
                    },
                    result.elapsed_ms as f64 / 1000.
                ),
                MICRO,
                p.text3,
            ));
            rows.push(
                Container::new(self.vstack(8., content))
                    .with_uniform_padding(S4)
                    .with_background_color(p.bg)
                    .with_border(Border::all(1.).with_border_color(p.border))
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
                    .finish(),
            );
        }
        self.vstack(12., rows)
    }

    fn amoo_conversation(&self) -> El {
        let p = self.p();
        let Some(sess) = self.amoo_sessions.get(self.amoo_session_idx) else {
            return self.text(
                "点击“新建对话”即可从后台使用 Amoo，无需打开手机界面。",
                CAPTION,
                p.text3,
            );
        };
        if sess.items.is_empty() {
            return self.text("此会话暂无消息，可在下方直接输入文字。", CAPTION, p.text3);
        }

        let mut rows: Vec<El> = Vec::new();
        for (idx, item) in sess.items.iter().enumerate() {
            match item {
                AmooChatItem::Attachment {
                    name,
                    kind,
                    mime,
                    url,
                    path,
                    size_bytes,
                } => {
                    rows.push(self.amoo_phone_attachment_card(
                        name,
                        kind,
                        mime,
                        url,
                        path,
                        *size_bytes,
                    ));
                }
                AmooChatItem::ToolRecords { key, records } => {
                    rows.push(self.amoo_execution_records(key, records));
                }
                AmooChatItem::UserText { attachment, text } => {
                    let Some(text) = stepx_companion::work_protocol::display_text(text, true)
                    else {
                        continue;
                    };
                    let mut col = Flex::column()
                        .with_spacing(4.)
                        .with_cross_axis_alignment(CrossAxisAlignment::End);
                    if let Some((att_name, att_meta, is_img)) = attachment {
                        let img_card = Container::new(self.hstack(
                            8.,
                            vec![
                                self.icon_box(
                                    if *is_img { "🖼" } else { "📄" },
                                    28.,
                                    if *is_img { p.blue } else { p.success },
                                    ColorU::white(),
                                    None,
                                ),
                                Flex::column()
                                    .with_spacing(1.)
                                    .with_cross_axis_alignment(CrossAxisAlignment::Start)
                                    .with_child(self.line_w(
                                        att_name.clone(),
                                        CAPTION,
                                        p.text,
                                        Weight::Medium,
                                    ))
                                    .with_child(self.mono(att_meta.clone(), 11., p.text2))
                                    .finish(),
                            ],
                        ))
                        .with_padding_left(6.)
                        .with_padding_right(10.)
                        .with_vertical_padding(6.)
                        .with_background_color(p.bg)
                        .with_border(Border::all(1.).with_border_color(p.border))
                        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
                        .finish();
                        col = col.with_child(img_card);
                    }
                    let user_text_bubble =
                        Container::new(self.text(text.clone(), CAPTION, p.accent_fg))
                            .with_horizontal_padding(14.)
                            .with_vertical_padding(8.)
                            .with_background_color(p.accent)
                            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(14.)))
                            .finish();
                    col = col.with_child(
                        ConstrainedBox::new(user_text_bubble)
                            .with_max_width(430.)
                            .finish(),
                    );
                    rows.push(
                        Flex::row()
                            .with_main_axis_size(MainAxisSize::Max)
                            .with_main_axis_alignment(MainAxisAlignment::End)
                            .with_child(col.finish())
                            .finish(),
                    );
                }
                AmooChatItem::UserVoice {
                    transcript,
                    duration_sec,
                } => {
                    let voice_bars: Vec<El> =
                        [5., 11., 14., 8., 13., 15., 9., 6., 12., 14., 10., 7.]
                            .into_iter()
                            .map(|h| {
                                ConstrainedBox::new(
                                    Rect::new()
                                        .with_background_color(p.accent_fg)
                                        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(
                                            1.,
                                        )))
                                        .finish(),
                                )
                                .with_width(2.)
                                .with_height(h)
                                .finish()
                            })
                            .collect();

                    let play_icon = if self.amoo_voice_playing {
                        UiIcon::Pause
                    } else {
                        UiIcon::Play
                    };
                    let dur_label = format!("0:{duration_sec:02}\"");
                    let v_state = self.mouse_state(format!("amoo-voice-pill:{idx}"));
                    let voice_pill = Hoverable::new(v_state, move |_| {
                        Container::new(self.hstack(
                            8.,
                            vec![
                                ConstrainedBox::new(
                                    Container::new(
                                        Flex::row()
                                            .with_main_axis_size(MainAxisSize::Max)
                                            .with_main_axis_alignment(MainAxisAlignment::Center)
                                            .with_cross_axis_alignment(CrossAxisAlignment::Center)
                                            .with_child(self.ui_icon(play_icon, 12., p.accent_fg))
                                            .finish(),
                                    )
                                    .with_background_color(rgba(0xFF, 0xFF, 0xFF, 38))
                                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(
                                        10.,
                                    )))
                                    .finish(),
                                )
                                .with_width(20.)
                                .with_height(20.)
                                .finish(),
                                self.hstack(2., voice_bars),
                                self.mono(dur_label, CAPTION, p.accent_fg),
                            ],
                        ))
                        .with_horizontal_padding(12.)
                        .with_vertical_padding(6.)
                        .with_background_color(p.accent)
                        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(14.)))
                        .finish()
                    })
                    .with_cursor(Cursor::PointingHand)
                    .on_click(|ctx, _, _| ctx.dispatch_typed_action(Action::AmooToggleVoicePlay))
                    .finish();

                    rows.push(
                        Flex::row()
                            .with_main_axis_size(MainAxisSize::Max)
                            .with_main_axis_alignment(MainAxisAlignment::End)
                            .with_child(self.hstack(
                                8.,
                                vec![self.line(transcript.clone(), CAPTION, p.text2), voice_pill],
                            ))
                            .finish(),
                    );
                }
                AmooChatItem::AssistantRich {
                    thinking_time,
                    thinking_steps,
                    tools_summary,
                    text,
                    has_bolt_artifact,
                    has_invoice_artifact,
                    has_safety_confirm,
                    time_label,
                } => {
                    let think_state = self.mouse_state(format!("amoo-think-btn:{idx}"));
                    let expanded = self.amoo_thinking_expanded;
                    let think_label = if self.legacy_demo() {
                        format!("已深度思考 {thinking_time}")
                    } else if !thinking_steps.is_empty() {
                        "思考过程".into()
                    } else {
                        thinking_time.clone()
                    };
                    let think_chip = if thinking_steps.is_empty() {
                        self.line_w(
                            think_label,
                            11.,
                            if thinking_time == "请求失败" {
                                p.danger
                            } else {
                                p.text2
                            },
                            Weight::Medium,
                        )
                    } else {
                        Hoverable::new(think_state, move |mouse| {
                            Container::new(self.hstack(
                                4.,
                                vec![
                                    self.line_w(think_label, 11., p.text2, Weight::Medium),
                                    self.ui_icon(
                                        if expanded {
                                            UiIcon::ChevronDown
                                        } else {
                                            UiIcon::ChevronRight
                                        },
                                        12.,
                                        p.text2,
                                    ),
                                ],
                            ))
                            .with_horizontal_padding(6.)
                            .with_vertical_padding(2.)
                            .with_background_color(if mouse.is_mouse_over_element() {
                                p.hover
                            } else {
                                p.surface
                            })
                            .with_border(Border::all(1.).with_border_color(p.surface_border))
                            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
                            .finish()
                        })
                        .with_cursor(Cursor::PointingHand)
                        .on_click(|ctx, _, _| ctx.dispatch_typed_action(Action::AmooToggleThinking))
                        .finish()
                    };

                    let thinking_bar = Container::new(self.between(
                        self.hstack(
                            6.,
                            vec![
                                think_chip,
                                self.line_w(
                                    if self.legacy_demo() {
                                        format!("✓ 已调用 {} 项工具", thinking_steps.len())
                                    } else {
                                        String::new()
                                    },
                                    11.,
                                    p.success,
                                    Weight::Medium,
                                ),
                            ],
                        ),
                        self.line(tools_summary.clone(), 11., p.text2),
                    ))
                    .with_padding_bottom(6.)
                    .with_border(Border::bottom(1.).with_border_color(p.border))
                    .finish();

                    let mut card_children: Vec<El> = vec![thinking_bar];

                    if expanded && !thinking_steps.is_empty() {
                        let step_lines: Vec<El> = thinking_steps
                            .iter()
                            .map(|s| self.text(s.clone(), 11., p.text2))
                            .collect();
                        card_children.push(
                            Container::new(self.vstack(4., step_lines))
                                .with_uniform_padding(8.)
                                .with_background_color(p.surface)
                                .with_border(Border::all(1.).with_border_color(p.surface_border))
                                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(
                                    R_CONTROL,
                                )))
                                .finish(),
                        );
                    }

                    if let Some(body_txt) = text
                        .as_deref()
                        .and_then(|body| stepx_companion::work_protocol::display_text(body, false))
                    {
                        let marker = "🖼 [AI 生成图片卡片 · 手机端已渲染高清写真]";
                        if self.legacy_demo() && body_txt.contains(marker) {
                            let parts: Vec<&str> = body_txt.splitn(2, marker).collect();
                            let before = parts.first().copied().unwrap_or("").trim();
                            let after = parts.get(1).copied().unwrap_or("").trim();
                            if !before.is_empty() {
                                card_children.push(
                                    self.work_rich_text(before, &format!("amoo-{idx}-before")),
                                );
                            }
                            let img_path = ensure_amoo_tti_portrait_path();
                            let tti_state = self.mouse_state(format!("tti-img-zoom:{idx}"));
                            let is_open = self.amoo_lightbox_open;
                            let zoom_pct = self.amoo_lightbox_zoom;
                            let img_sz = if is_open {
                                if zoom_pct > 100 {
                                    330.
                                } else {
                                    275.
                                }
                            } else {
                                186.
                            };
                            let tti_image = Hoverable::new(tti_state, move |mouse| {
                                let hovered = mouse.is_mouse_over_element();
                                Container::new(self.vstack(
                                    6.,
                                    vec![
                                        ConstrainedBox::new(
                                            Image::new(
                                                AssetSource::LocalFile {
                                                    path: img_path.clone(),
                                                },
                                                CacheOption::BySize,
                                            )
                                            .contain()
                                            .finish(),
                                        )
                                        .with_width(img_sz)
                                        .with_height(img_sz)
                                        .finish(),
                                        self.between(
                                            self.line_w(
                                                if is_open {
                                                    format!(
                                                        "🔍 高清暗房检视已展开 · {}% (1024×1024)",
                                                        zoom_pct
                                                    )
                                                } else {
                                                    "🔍 点击放大高清原图 (1024×1024)".into()
                                                },
                                                11.,
                                                if is_open || hovered {
                                                    p.accent
                                                } else {
                                                    p.text2
                                                },
                                                Weight::Medium,
                                            ),
                                            self.mono("2K PNG", 10.5, p.accent),
                                        ),
                                    ],
                                ))
                                .with_uniform_padding(if is_open { 10. } else { 6. })
                                .with_background_color(if is_open {
                                    rgb(0x10, 0x12, 0x18)
                                } else {
                                    p.surface
                                })
                                .with_border(
                                    Border::all(if is_open { 1.5 } else { 1. }).with_border_color(
                                        if is_open || hovered {
                                            p.accent
                                        } else {
                                            p.border
                                        },
                                    ),
                                )
                                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(12.)))
                                .finish()
                            })
                            .with_cursor(Cursor::PointingHand)
                            .on_click(|ctx, _, _| {
                                ctx.dispatch_typed_action(Action::AmooToggleLightbox)
                            })
                            .finish();

                            let mut action_btns = vec![self.button(
                                format!("tti-btn-zoom:{idx}"),
                                if self.amoo_lightbox_open {
                                    "✕ 收起大图预览"
                                } else {
                                    "🔍 放大检视原图"
                                },
                                Action::AmooToggleLightbox,
                                Btn::Primary,
                            )];
                            if self.amoo_lightbox_open {
                                action_btns.push(self.button(
                                    format!("tti-btn-scale:{idx}"),
                                    if self.amoo_lightbox_zoom > 100 {
                                        "🔎 100% 原始比例"
                                    } else {
                                        "🔎 150% 细节放大"
                                    },
                                    Action::AmooToggleLightboxZoom,
                                    Btn::Secondary,
                                ));
                            }
                            action_btns.extend([
                                self.button(
                                    format!("tti-btn-save:{idx}"),
                                    "💾 保存到电脑",
                                    Action::AmooSaveLightboxImage,
                                    Btn::Secondary,
                                ),
                                self.button(
                                    format!("tti-btn-matrix:{idx}"),
                                    "❖ 转背屏点阵",
                                    Action::AmooPushArtifactBolt,
                                    Btn::Secondary,
                                ),
                            ]);
                            let img_actions = self.hstack(6., action_btns);
                            card_children.push(self.vstack(
                                6.,
                                vec![
                                        Flex::row()
                                            .with_main_axis_size(MainAxisSize::Min)
                                            .with_child(tti_image)
                                            .finish(),
                                        img_actions,
                                    ],
                            ));
                            if !after.is_empty() {
                                card_children
                                    .push(self.work_rich_text(after, &format!("amoo-{idx}-after")));
                            }
                        } else {
                            card_children.push(self.work_rich_text(
                                &body_txt,
                                &format!("amoo-{}-{idx}", sess.session_key),
                            ));
                        }
                    }

                    if *has_bolt_artifact || *has_invoice_artifact {
                        let mut art_row = Flex::row()
                            .with_main_axis_size(MainAxisSize::Max)
                            .with_spacing(8.);

                        if *has_bolt_artifact {
                            let bolt_frame = BackScreenPattern::Bolt.frame_391(7, false);
                            let mini_bolt_box = ConstrainedBox::new(
                                Container::new(
                                    Flex::row()
                                        .with_main_axis_size(MainAxisSize::Max)
                                        .with_main_axis_alignment(MainAxisAlignment::Center)
                                        .with_cross_axis_alignment(CrossAxisAlignment::Center)
                                        .with_child(self.matrix_grid(&bolt_frame, 0.9, 0.25, false))
                                        .finish(),
                                )
                                .with_background_color(rgb(0x09, 0x09, 0x0C))
                                .with_border(
                                    Border::all(1.).with_border_color(rgb(0x2E, 0x2E, 0x38)),
                                )
                                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
                                .finish(),
                            )
                            .with_width(28.)
                            .with_height(32.)
                            .finish();

                            let art_1 = Container::new(self.between(
                                self.hstack(
                                    8.,
                                    vec![
                                        mini_bolt_box,
                                        Flex::column()
                                            .with_spacing(1.)
                                            .with_cross_axis_alignment(CrossAxisAlignment::Start)
                                            .with_child(self.line_w(
                                                "赛博闪电",
                                                CAPTION,
                                                p.text,
                                                Weight::Semibold,
                                            ))
                                            .with_child(self.line("23×17 点阵", 11., p.text2))
                                            .finish(),
                                    ],
                                ),
                                self.button(
                                    format!("amoo-art-push:{idx}"),
                                    if self.amoo_bolt_pushed {
                                        "✓ 已投背屏"
                                    } else {
                                        "投到背屏"
                                    },
                                    Action::AmooPushArtifactBolt,
                                    Btn::Primary,
                                ),
                            ))
                            .with_uniform_padding(8.)
                            .with_background_color(p.surface)
                            .with_border(Border::all(1.).with_border_color(p.surface_border))
                            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
                            .finish();
                            art_row = art_row.with_child(Expanded::new(1., art_1).finish());
                        }

                        if *has_invoice_artifact {
                            let invoice_icon_box = ConstrainedBox::new(
                                Container::new(
                                    Flex::row()
                                        .with_main_axis_size(MainAxisSize::Max)
                                        .with_main_axis_alignment(MainAxisAlignment::Center)
                                        .with_cross_axis_alignment(CrossAxisAlignment::Center)
                                        .with_child(self.line("📊", CAPTION, p.success_text))
                                        .finish(),
                                )
                                .with_background_color(p.success_soft)
                                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(
                                    R_CONTROL,
                                )))
                                .finish(),
                            )
                            .with_width(28.)
                            .with_height(32.)
                            .finish();

                            let art_2 = Container::new(self.between(
                                self.hstack(
                                    8.,
                                    vec![
                                        invoice_icon_box,
                                        Flex::column()
                                            .with_spacing(1.)
                                            .with_cross_axis_alignment(CrossAxisAlignment::Start)
                                            .with_child(self.line_w(
                                                "10月报销单",
                                                CAPTION,
                                                p.text,
                                                Weight::Semibold,
                                            ))
                                            .with_child(self.line("已记 ¥386.00", 11., p.text2))
                                            .finish(),
                                    ],
                                ),
                                self.button(
                                    format!("amoo-art-open:{idx}"),
                                    if self.amoo_invoice_open {
                                        "收起"
                                    } else {
                                        "打开"
                                    },
                                    Action::AmooToggleInvoiceDetail,
                                    Btn::Secondary,
                                ),
                            ))
                            .with_uniform_padding(8.)
                            .with_background_color(p.surface)
                            .with_border(Border::all(1.).with_border_color(p.surface_border))
                            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
                            .finish();
                            art_row = art_row.with_child(Expanded::new(1., art_2).finish());
                        }

                        card_children.push(art_row.finish());
                    }

                    if *has_invoice_artifact && self.amoo_invoice_open {
                        let inv_detail = Container::new(self.vstack(
                            4.,
                            vec![
                                self.between(
                                    self.line_w(
                                        "10月差旅与日常报销台账（共 3 笔）",
                                        CAPTION,
                                        p.text,
                                        Weight::Semibold,
                                    ),
                                    self.link_action(
                                        format!("amoo-inv-copy:{idx}"),
                                        "复制台账",
                                        Action::CopyText(
                                            "10月报销单：\n1. 高德打车电子发票 ¥68.00\n2. 差旅餐饮发票 ¥128.00\n3. 办公耗材发票 ¥190.00\n合计：¥386.00".into(),
                                            "已复制 10 月报销单台账明细".into(),
                                        ),
                                    ),
                                ),
                                self.mono(
                                    "1. 10-21 高德打车电子发票 · ¥68.00   2. 10-19 差旅餐饮 · ¥128.00   3. 10-15 办公耗材 · ¥190.00",
                                    11.,
                                    p.text2,
                                ),
                            ],
                        ))
                        .with_uniform_padding(8.)
                        .with_background_color(p.surface)
                        .with_border(Border::all(1.).with_border_color(p.surface_border))
                        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_CONTROL)))
                        .finish();
                        card_children.push(inv_detail);
                    }

                    if *has_safety_confirm {
                        let safety_card_el = match self.amoo_safety_decision {
                            SafetyDecision::Pending => Container::new(
                                self.between(
                                    Flex::column()
                                        .with_spacing(2.)
                                        .with_cross_axis_alignment(CrossAxisAlignment::Start)
                                        .with_child(self.lead(
                                            6.,
                                            self.dot(p.warning),
                                            self.line_w(
                                                "待确认 · 发送短信给李明",
                                                CAPTION,
                                                p.text,
                                                Weight::Semibold,
                                            ),
                                        ))
                                        .with_child(self.line(
                                            "138 0013 8000 · “我取完快递马上过去。”",
                                            11.,
                                            p.text2,
                                        ))
                                        .finish(),
                                    self.hstack(
                                        6.,
                                        vec![
                                            self.button(
                                                format!("amoo-confirm-reject:{idx}"),
                                                "拒绝",
                                                Action::AmooConfirmSafety(false),
                                                Btn::Secondary,
                                            ),
                                            self.button(
                                                format!("amoo-confirm-allow:{idx}"),
                                                "允许执行",
                                                Action::AmooConfirmSafety(true),
                                                Btn::Primary,
                                            ),
                                        ],
                                    ),
                                ),
                            )
                            .with_uniform_padding(10.)
                            .with_background_color(p.warning_soft)
                            .with_border(Border::all(1.).with_border_color(p.warning_border))
                            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
                            .finish(),
                            SafetyDecision::Approved => Container::new(self.between(
                                self.lead(
                                    6.,
                                    self.dot(p.success),
                                    self.line_w(
                                        "✓ 已允许执行 · 已向李明 (138 0013 8000) 发送短信",
                                        CAPTION,
                                        p.success_text,
                                        Weight::Semibold,
                                    ),
                                ),
                                self.link_muted(
                                    format!("amoo-safety-undo-inline:{idx}"),
                                    "撤回",
                                    Action::AmooUndoSafety,
                                ),
                            ))
                            .with_uniform_padding(10.)
                            .with_background_color(p.success_soft)
                            .with_border(Border::all(1.).with_border_color(p.success))
                            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
                            .finish(),
                            SafetyDecision::Rejected => Container::new(self.between(
                                self.lead(
                                    6.,
                                    self.dot(p.text3),
                                    self.line_w(
                                        "✕ 已拒绝执行 · 已拦截向李明发送短信",
                                        CAPTION,
                                        p.text2,
                                        Weight::Medium,
                                    ),
                                ),
                                self.link_muted(
                                    format!("amoo-safety-reopen:{idx}"),
                                    "重新审议",
                                    Action::AmooUndoSafety,
                                ),
                            ))
                            .with_uniform_padding(10.)
                            .with_background_color(p.surface)
                            .with_border(Border::all(1.).with_border_color(p.surface_border))
                            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
                            .finish(),
                        };
                        card_children.push(safety_card_el);
                    }

                    let copy_payload = text
                        .as_deref()
                        .and_then(|body| stepx_companion::work_protocol::display_text(body, false))
                        .unwrap_or_else(|| thinking_steps.join("\n"));
                    let mut reply_actions = vec![self.amoo_action_icon(
                        format!("amoo-reply-copy:{idx}"),
                        "复制回复",
                        UiIcon::Copy,
                        Action::CopyText(copy_payload, "已复制 Amoo 回复内容".into()),
                    )];
                    if self.legacy_demo() {
                        reply_actions.push(self.amoo_action_icon(
                            format!("amoo-reply-tts:{idx}"),
                            if self.amoo_tts_on {
                                "停止播报"
                            } else {
                                "语音播报"
                            },
                            UiIcon::Speaker,
                            Action::AmooToggleTts,
                        ));
                    }
                    if let Some(text) = retry_prompt(&sess.items, idx) {
                        reply_actions.push(self.amoo_action_icon(
                            format!("amoo-reply-retry:{idx}"),
                            "重新发送此回复对应的文字问题",
                            UiIcon::Retry,
                            Action::AmooRetryMessage {
                                session_key: sess.session_key.clone(),
                                text,
                            },
                        ));
                    }
                    let reply_footer = self.between(
                        self.hstack(2., reply_actions),
                        self.mono(time_label.clone(), 11., p.text2),
                    );
                    card_children.push(reply_footer);

                    let reply_card = Container::new(self.vstack(8., card_children))
                        .with_uniform_padding(12.)
                        .with_background_color(p.bg)
                        .with_border(Border::all(1.).with_border_color(p.border))
                        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(16.)))
                        .finish();

                    let avatar_a = ConstrainedBox::new(
                        Container::new(
                            Flex::row()
                                .with_main_axis_size(MainAxisSize::Max)
                                .with_main_axis_alignment(MainAxisAlignment::Center)
                                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                                .with_child(self.line_w(
                                    "A",
                                    CAPTION,
                                    ColorU::white(),
                                    Weight::Semibold,
                                ))
                                .finish(),
                        )
                        .with_background_color(if p.dark {
                            rgb(0x27, 0x27, 0x2A)
                        } else {
                            rgb(0x18, 0x18, 0x1B)
                        })
                        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_CONTROL)))
                        .finish(),
                    )
                    .with_width(24.)
                    .with_height(24.)
                    .finish();

                    rows.push(
                        Flex::row()
                            .with_main_axis_size(MainAxisSize::Max)
                            .with_cross_axis_alignment(CrossAxisAlignment::Start)
                            .with_spacing(10.)
                            .with_child(avatar_a)
                            .with_child(Expanded::new(1., reply_card).finish())
                            .finish(),
                    );
                }
            }
        }

        self.vstack(10., rows)
    }

    /// Lines read from the phone screen; click a line to add it to the question.
    fn screen_text_panel(&self) -> Option<El> {
        if self.content.is_empty() || self.output_page != super::super::Page::Amoo {
            return None;
        }
        let p = self.p();
        let lines: Vec<&str> = self
            .content
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        let shown: Vec<El> = lines
            .iter()
            .take(3)
            .enumerate()
            .map(|(i, line)| {
                self.list_row(
                    format!("screen-{i}"),
                    self.line(*line, CAPTION, p.text2),
                    false,
                    Some(Action::AppendVisibleLine((*line).to_string())),
                    None,
                )
            })
            .collect();
        let header = self.between(
            self.line("屏幕文字 · 点击一行加入问题", CAPTION, p.text2),
            self.hstack(
                S3,
                vec![
                    self.link("screen-copy", "复制全部", Action::Copy),
                    self.link("screen-close", "关闭", Action::ClearContent),
                ],
            ),
        );
        Some(self.vstack(S2, vec![header, self.list(shown)]))
    }
}

#[cfg(test)]
mod gui_display_tests {
    use super::*;

    #[test]
    fn explicit_lock_blocks_are_not_shown_as_task_completion() {
        assert!(reply_reports_lock_block(
            "手机还锁着，截图被拦截了，天气任务无法继续。"
        ));
        assert!(reply_reports_lock_block("请先解锁手机，然后我再查询天气。"));
        assert!(!reply_reports_lock_block("已查到天气，无需解锁手机。"));
        assert!(!reply_reports_lock_block("解锁后已完成，天气是晴天。"));
        assert!(!session_waits_for_unlock(&[AmooChatItem::ToolRecords {
            key: "logs".into(),
            records: Vec::new(),
        }]));
    }
}

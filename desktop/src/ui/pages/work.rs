//! Amoo Work: task execution with a persistent deliverables and preview pane.
use super::super::{
    amoo_display::session_title, icons::UiIcon, theme::*, widgets::*, Action, AmooChatItem,
    Companion, Page, WorkAction, F_WORK_AGENT_NAME, F_WORK_CAP_SEARCH, F_WORK_PROMPT,
    F_WORK_SEARCH,
};
use facloud_ui::{elements::*, fonts::Weight, platform::Cursor};
use pathfinder_geometry::vector::vec2f;
use std::collections::HashSet;
use stepx_companion::storage::format_bytes;

// Shared with the desktop harness, including mixed protocol + Markdown replies.
fn work_message_text(text: &str, user: bool) -> Option<String> {
    stepx_companion::work_protocol::display_text(text, user)
}

const WORK_SCENARIOS: [&str; 6] = [
    "资料总结",
    "表格分析",
    "文档修改",
    "图片创作",
    "编写游戏",
    "制作 PPT",
];

fn compact_work_text(text: &str, max: usize) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() > max {
        format!("{}…", text.chars().take(max).collect::<String>())
    } else {
        text
    }
}

fn work_step_name(name: &str) -> &str {
    match name {
        "exec" | "system.run" | "stepclaw.exec" => "执行任务步骤",
        "ability" | "ability.call" => "调用手机能力",
        "list_files" => "查看工作资料",
        "read_file" => "阅读资料",
        "csv_summary" => "分析表格数据",
        "write_file" => "保存工作成果",
        "task_create" | "sessions_spawn" => "创建子任务",
        "text_to_image" => "生成图片",
        "create_document" => "生成文档",
        "edit_document" => "修订文档",
        "edit_text" => "修改文件",
        "create_spreadsheet" => "生成表格",
        "create_presentation" | "write_presentation" => "制作演示文稿",
        _ => name,
    }
}

impl Companion {
    fn work_button(&self, id: &str, label: &str, action: WorkAction, kind: Btn) -> El {
        self.button(format!("work-{id}"), label, Action::Work(action), kind)
    }

    pub(in super::super) fn work_action_icon(
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
            Stack::new()
                .with_child(button)
                .with_positioned_child(
                    Container::new(self.line(label, MICRO, p.text))
                        .with_horizontal_padding(S2)
                        .with_vertical_padding(S1)
                        .with_background_color(p.bg)
                        .with_border(Border::all(1.).with_border_color(p.border))
                        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_CONTROL)))
                        .finish(),
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

    pub(in super::super) fn work_actions(&self) -> Vec<El> {
        vec![
            if self.busy || self.work.catalog_loading {
                self.disabled_button("正在同步")
            } else {
                self.work_button("refresh", "同步手机", WorkAction::Refresh, Btn::Secondary)
            },
            if self.busy || self.work.running {
                self.disabled_button("+ 新建工作")
            } else {
                self.work_button("new", "+ 新建工作", WorkAction::NewChat, Btn::Primary)
            },
        ]
    }

    fn work_title(&self, icon: UiIcon, title: impl Into<String>) -> El {
        let p = self.p();
        self.hstack(
            S2,
            vec![
                self.ui_icon(icon, 16., p.text2),
                Expanded::new(1., self.line_w(title, HEADING, p.text, Weight::Semibold)).finish(),
            ],
        )
    }

    fn work_session_pane(&self) -> El {
        let p = self.p();
        let query = self.fields[F_WORK_SEARCH].text.trim().to_lowercase();
        let mut rows = Vec::new();
        for (index, session) in self.amoo_sessions.iter().enumerate() {
            if !query.is_empty()
                && !session_title(&session.title)
                    .to_lowercase()
                    .contains(&query)
                && !session.agent.to_lowercase().contains(&query)
            {
                continue;
            }
            let active = self.amoo_session_idx == index;
            let pending = self.amoo_pending_reply.contains_key(&session.session_key);
            let title = session_title(&session.title);
            let subtitle = if pending {
                "正在处理".to_string()
            } else {
                session.meta.clone()
            };
            let row = self.vstack(
                S1,
                vec![
                    self.line_w(title, BODY, p.text, Weight::Medium),
                    self.line(subtitle, MICRO, if pending { p.success } else { p.text3 }),
                ],
            );
            rows.push(
                Hoverable::new(
                    self.mouse_state(format!("work-session-{index}")),
                    move |mouse| {
                        Container::new(row)
                            .with_uniform_padding(S3)
                            .with_background_color(if active {
                                p.selected
                            } else if mouse.is_mouse_over_element() {
                                p.hover
                            } else {
                                p.bg
                            })
                            .with_border(Border::all(1.).with_border_color(if active {
                                p.border_strong
                            } else {
                                p.border
                            }))
                            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_CONTROL)))
                            .finish()
                    },
                )
                .with_cursor(Cursor::PointingHand)
                .on_click(move |ctx, _, _| {
                    ctx.dispatch_typed_action(Action::Work(WorkAction::Session(index)))
                })
                .finish(),
            );
        }
        if rows.is_empty() {
            rows.push(self.text(
                if query.is_empty() {
                    "手机会话将在同步后显示。可以新建对话开始工作。"
                } else {
                    "没有匹配的会话"
                },
                CAPTION,
                p.text3,
            ));
        }
        Container::new(
            Flex::column()
                .with_main_axis_size(MainAxisSize::Max)
                .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                .with_spacing(S3)
                .with_child(self.work_title(UiIcon::Tools, "工作记录"))
                .with_child(self.input(F_WORK_SEARCH, "搜索工作与会话…", None))
                .with_child(
                    Expanded::new(
                        1.,
                        self.pane_scroll(self.scroll_aux.clone(), self.vstack(S2, rows)),
                    )
                    .finish(),
                )
                .with_child(self.text("后台连接手机 Amoo，无需唤起手机界面。", MICRO, p.text3))
                .finish(),
        )
        .with_uniform_padding(S3)
        .with_background_color(p.sidebar)
        .with_border(Border::right(1.).with_border_color(p.border))
        .finish()
    }

    fn work_chat_bubble(&self, index: usize, item: &AmooChatItem) -> El {
        let p = self.p();
        if let AmooChatItem::Attachment {
            name,
            kind,
            size_bytes,
            ..
        } = item
        {
            return self.panel(vec![self.between(
                self.hstack(
                    S2,
                    vec![
                        self.ui_icon(
                            if kind == "image" {
                                UiIcon::Image
                            } else {
                                UiIcon::File
                            },
                            18.,
                            p.text2,
                        ),
                        Expanded::new(
                            1.,
                            self.vstack(
                                S1,
                                vec![
                                    self.line_w(name, BODY, p.text, Weight::Medium),
                                    self.line(
                                        size_bytes
                                            .map(format_bytes)
                                            .unwrap_or_else(|| "手机任务附件".into()),
                                        MICRO,
                                        p.text3,
                                    ),
                                ],
                            ),
                        )
                        .finish(),
                    ],
                ),
                self.work_button(
                    &format!("preview-result-{index}"),
                    "预览",
                    WorkAction::PreviewRemote(index),
                    Btn::Secondary,
                ),
            )]);
        }
        if let AmooChatItem::ToolRecords { key, records } = item {
            let expanded = self.amoo_expanded_tool_records.contains(key);
            let mut names = Vec::new();
            for record in records {
                let name = work_step_name(&record.name);
                if !names.contains(&name) {
                    names.push(name);
                }
            }
            let mut children = vec![self.between(
                self.button(
                    format!("work-records-{key}"),
                    format!(
                        "{} 执行过程 · {} 步",
                        if expanded { "▾" } else { "›" },
                        records.len()
                    ),
                    Action::AmooToggleToolRecords(key.clone()),
                    Btn::Ghost,
                ),
                ConstrainedBox::new(self.line(
                    compact_work_text(&names.join(" · "), 30),
                    MICRO,
                    p.text3,
                ))
                .with_max_width(180.)
                .finish(),
            )];
            if expanded {
                for (record_index, record) in records.iter().enumerate() {
                    let mut details = vec![self.between(
                        self.line(
                            format!("{:02}  {}", record_index + 1, work_step_name(&record.name)),
                            CAPTION,
                            p.text2,
                        ),
                        self.line(&record.time_label, MICRO, p.text3),
                    )];
                    if let Some(detail) = record.detail.as_ref().filter(|text| !text.is_empty()) {
                        let detail_key = format!("work-record-detail-{key}-{record_index}");
                        let full = self.amoo_expanded_tool_records.contains(&detail_key);
                        details.push(self.text(
                            if full {
                                detail.clone()
                            } else {
                                compact_work_text(detail, 160)
                            },
                            MICRO,
                            p.text3,
                        ));
                        if detail.chars().count() > 160 {
                            details.push(self.link_muted(
                                format!("{detail_key}-toggle"),
                                if full { "收起详情" } else { "查看详情" },
                                Action::AmooToggleToolRecords(detail_key),
                            ));
                        }
                    }
                    children.push(
                        Container::new(self.vstack(S1, details))
                            .with_uniform_padding(S2)
                            .with_border(Border::top(1.).with_border_color(p.border))
                            .finish(),
                    );
                }
            }
            return Container::new(self.vstack(S2, children))
                .with_uniform_padding(S2)
                .with_background_color(p.surface)
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
                .finish();
        }
        let (user, body, attachment, time) = match item {
            AmooChatItem::UserText { text, attachment } => (
                true,
                text.clone(),
                attachment
                    .as_ref()
                    .map(|(name, size, _)| format!("{name} · {size}")),
                String::new(),
            ),
            AmooChatItem::UserVoice { transcript, .. } => {
                (true, transcript.clone(), None, String::new())
            }
            AmooChatItem::AssistantRich {
                text, time_label, ..
            } => (
                false,
                text.clone().unwrap_or_default(),
                None,
                time_label.clone(),
            ),
            AmooChatItem::ToolRecords { .. } | AmooChatItem::Attachment { .. } => unreachable!(),
        };
        let Some(body) = work_message_text(&body, user) else {
            return Empty::new().finish();
        };
        if body.trim().is_empty() && attachment.is_none() {
            return Empty::new().finish();
        }
        let mut meta = vec![self.line(time, MICRO, p.text3)];
        if !user && !body.is_empty() {
            meta.push(self.work_action_icon(
                format!("work-copy-message-{index}"),
                "复制回复",
                UiIcon::Copy,
                Action::CopyText(body.clone(), "已复制回复".into()),
            ));
        }
        if !user && !body.is_empty() {
            meta.push(self.speech_icon(
                format!("work-speech:{}:{index}", self.amoo_session_idx),
                body.clone(),
            ));
        }
        let mut children = vec![self.between(
            self.hstack(
                S2,
                vec![
                    self.ui_icon(
                        if user { UiIcon::Chat } else { UiIcon::Star },
                        14.,
                        if user { p.text2 } else { p.accent },
                    ),
                    self.line_w(
                        if user { "任务要求" } else { "Amoo" },
                        CAPTION,
                        p.text2,
                        Weight::Medium,
                    ),
                ],
            ),
            self.hstack(S1, meta),
        )];
        if let Some(attachment) = attachment {
            children.push(self.hstack(
                S2,
                vec![
                    self.ui_icon(UiIcon::Attach, 14., p.text3),
                    Expanded::new(1., self.line(attachment, CAPTION, p.text2)).finish(),
                ],
            ));
        }
        if !body.is_empty() {
            children.push(if user {
                self.text(body.clone(), BODY, p.text)
            } else {
                self.work_rich_text(
                    &self.live_reply_text(index, &body),
                    &format!("message-{index}"),
                )
            });
        }
        Container::new(self.vstack(S2, children))
            .with_uniform_padding(S3)
            .with_background_color(if user { p.surface } else { p.bg })
            .with_border(Border::all(if user { 1. } else { 0. }).with_border_color(p.border))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
            .finish()
    }

    fn work_prompt_input(&self) -> El {
        let p = self.p();
        let field = &self.fields[F_WORK_PROMPT];
        let focused = self.focus == Some(F_WORK_PROMPT);
        let mut value = field.text.clone();
        let mut caret = 0;
        if focused {
            let cursor = field.cursor.min(value.len());
            caret = value[..cursor].chars().count() + field.preedit.chars().count();
            value.insert_str(cursor, &field.preedit);
        }
        let is_empty = field.text.is_empty() && field.preedit.is_empty();
        if value.is_empty() {
            value = if self.work.mode == 2 {
                "描述要生成的图片，可添加参考图、指定风格与尺寸…"
            } else {
                "描述要完成的工作，可附上文件、目标和交付要求…"
            }
            .into();
        }
        Hoverable::new(self.mouse_state("work-prompt"), move |_| {
            ConstrainedBox::new(
                Container::new(self.pane_scroll(
                    self.work.prompt_scroll.clone(),
                    self.multiline_field_text(
                        &value,
                        caret,
                        F_WORK_PROMPT,
                        if is_empty { p.text3 } else { p.text },
                    ),
                ))
                .with_horizontal_padding(S1)
                .with_vertical_padding(S2)
                .with_background_color(p.bg)
                .finish(),
            )
            .with_min_height(48.)
            .with_max_height(112.)
            .finish()
        })
        .with_cursor(Cursor::IBeam)
        .with_defer_events_to_children()
        .on_mouse_down(|ctx, _, _| ctx.dispatch_typed_action(Action::Focus(F_WORK_PROMPT)))
        .on_right_click(|ctx, _, at| {
            ctx.dispatch_typed_action(Action::ContextOpen(
                crate::ui::context_menu::Target::Input(F_WORK_PROMPT),
                at,
            ))
        })
        .finish()
    }

    fn work_conversation_pane(&self) -> El {
        let p = self.p();
        let session = self.amoo_sessions.get(self.amoo_session_idx);
        let mut stream = Vec::new();
        if let Some(session) = session.filter(|s| !s.items.is_empty()) {
            stream.extend(
                session
                    .items
                    .iter()
                    .enumerate()
                    .map(|(i, item)| self.work_chat_bubble(i, item)),
            );
        } else {
            stream.push(self.panel(vec![
                    self.work_title(UiIcon::Tools, "今天想完成什么工作？"),
                    self.text(
                        "添加资料，说明目标和交付格式。Amoo 会逐步处理，并把成果文件放在右侧。",
                        BODY,
                        p.text2,
                    ),
                    self.wrap(
                        WORK_SCENARIOS
                            .iter()
                            .enumerate()
                            .map(|(index, name)| {
                                self.work_button(
                                    &format!("template-{index}"),
                                    name,
                                    WorkAction::Template(index as u8),
                                    Btn::Secondary,
                                )
                            })
                            .collect(),
                    ),
                ]));
        }
        if !self.work.events.is_empty() {
            let key = "amoo-work-desktop-events";
            let expanded = self.amoo_expanded_tool_records.contains(key);
            let mut events = vec![self.button(
                "work-desktop-events",
                format!(
                    "{} 工作过程 · {} 条记录",
                    if expanded { "▾" } else { "›" },
                    self.work.events.len()
                ),
                Action::AmooToggleToolRecords(key.into()),
                Btn::Ghost,
            )];
            if expanded {
                events.extend(self.work.events.iter().enumerate().map(|(index, event)| {
                    let detail_key = format!("work-event-detail-{index}");
                    let full = self.amoo_expanded_tool_records.contains(&detail_key);
                    let mut details = vec![self.text(
                        if full {
                            event.clone()
                        } else {
                            compact_work_text(event, 180)
                        },
                        CAPTION,
                        p.text2,
                    )];
                    if event.chars().count() > 180 {
                        details.push(self.link_muted(
                            format!("{detail_key}-toggle"),
                            if full { "收起详情" } else { "查看详情" },
                            Action::AmooToggleToolRecords(detail_key),
                        ));
                    }
                    Container::new(self.vstack(S1, details))
                        .with_uniform_padding(S2)
                        .with_border(Border::top(1.).with_border_color(p.border))
                        .finish()
                }));
            } else if let Some(last) = self.work.events.last() {
                events.push(self.line(compact_work_text(last, 110), CAPTION, p.text2));
            }
            stream.push(self.panel(events));
        }
        if let Some(error) = &self.work.error {
            stream.push(self.panel(vec![self.text(error, BODY, p.danger)]));
        }
        #[cfg(feature = "ui-capture")]
        if std::env::args().any(|arg| arg == "--work-format-fixture") {
            stream = vec![self.panel(vec![
                self.line("格式验收样例 · 本地渲染测试，不发送到手机", MICRO, p.text3),
                self.work_rich_text(
                    concat!(
                        "## 项目周报\n\n",
                        "本周完成 **3 项交付**，执行目录为 `reports/本周`。\n\n",
                        "| 交付项 | 状态 | 数量 |\n| :--- | :---: | ---: |\n",
                        "| 分析报告 | **已完成** | 2 |\n| 核对表 | 待复核 | 1 |\n\n",
                        "> 核对结论：保留来源和计算口径，缺失数据不按零处理。\n\n",
                        "1. 读取资料并核对字段。\n2. 生成报告，保留可追溯结果。\n\n",
                        "```json\n{\"result\": \"已保存\", \"files\": 3}\n```\n\n",
                        "查看 [参考资料](https://example.com/report)。\n"
                    ),
                    "format-fixture",
                ),
            ])];
            if std::env::args().any(|arg| arg == "--work-preview-file") {
                stream.push(self.amoo_attachment_card(
                    "手机实测生成图片 · 本地预览验收",
                    "image",
                    "image/png",
                    "https://example.invalid/amoo-work-native-preview.png",
                    None,
                ));
            }
        }
        if self.work.running
            || session.is_some_and(|s| self.amoo_pending_reply.contains_key(&s.session_key))
        {
            stream.push(self.hstack(
                S2,
                vec![
                    self.dot(p.success),
                    self.line(
                        if self.work.status.is_empty() {
                            "正在处理…"
                        } else {
                            &self.work.status
                        },
                        CAPTION,
                        p.text2,
                    ),
                ],
            ));
        }
        let mut composer = Vec::new();
        if self.work.attach_menu_open {
            composer.push(self.panel(vec![
                self.wrap(vec![
                    self.work_button(
                        "attach-image",
                        "🖼 上传本地图片",
                        WorkAction::AttachImages,
                        Btn::Ghost,
                    ),
                    self.work_button(
                        "attach-document",
                        "📎 添加文档与附件",
                        WorkAction::Attach,
                        Btn::Ghost,
                    ),
                ]),
                self.line(
                    if self.work.mode == 1 {
                        "资料复制到工作文件夹 · 图片将同步给手机识别"
                    } else {
                        "支持图片、文档与表格 · 最多 8 个文件 / 8 MB"
                    },
                    MICRO,
                    p.text3,
                ),
            ]));
        }
        if !self.work.attachments.is_empty() {
            composer.push(
                self.wrap(
                    self.work
                        .attachments
                        .iter()
                        .enumerate()
                        .map(|(index, path)| {
                            let image =
                                stepx_companion::amoo_media::mime_type(path).starts_with("image/");
                            Container::new(self.hstack(
                                S2,
                                vec![
                                        self.ui_icon(
                                            if image { UiIcon::Image } else { UiIcon::File },
                                            15.,
                                            if image { p.success } else { p.text2 },
                                        ),
                                        ConstrainedBox::new(
                                            self.line(
                                                path.file_name()
                                                    .map(|s| s.to_string_lossy())
                                                    .unwrap_or_default(),
                                                CAPTION,
                                                p.text,
                                            ),
                                        )
                                        .with_max_width(180.)
                                        .finish(),
                                        self.work_action_icon(
                                            format!("work-remove-attachment-{index}"),
                                            "移除附件",
                                            UiIcon::Close,
                                            Action::Work(WorkAction::RemoveAttachment(index)),
                                        ),
                                    ],
                            ))
                            .with_padding_left(S2)
                            .with_padding_right(S1)
                            .with_background_color(p.surface)
                            .with_border(Border::all(1.).with_border_color(p.border))
                            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_CONTROL)))
                            .finish()
                        })
                        .collect(),
                ),
            );
        }
        composer.push(self.work_prompt_input());
        let needs_workspace = self.work.mode == 1 && self.work.workspace.is_none();
        let phone_pending =
            session.is_some_and(|s| self.amoo_pending_reply.contains_key(&s.session_key));
        let can_send = !self.work.running
            && !phone_pending
            && !self.busy
            && self.selected.is_some()
            && !needs_workspace
            && !self.fields[F_WORK_PROMPT].text.trim().is_empty();
        let mut composer_actions = vec![self.work_button(
            "attach",
            if self.work.attach_menu_open {
                "× 收起附件"
            } else {
                "+ 添加文件"
            },
            WorkAction::ToggleAttachMenu,
            Btn::Ghost,
        )];
        if needs_workspace {
            composer_actions.push(self.work_button(
                "composer-workspace",
                "选择工作文件夹",
                WorkAction::ChooseWorkspace,
                Btn::Secondary,
            ));
        }
        composer_actions.push(
            Expanded::new(
                1.,
                self.line("Enter 发送 · Shift+Enter 换行", MICRO, p.text3),
            )
            .finish(),
        );
        composer_actions.push(if self.work.running {
            self.work_button("cancel", "停止任务", WorkAction::Cancel, Btn::Secondary)
        } else if phone_pending {
            self.work_button(
                "cancel-wait",
                "停止等待",
                WorkAction::Cancel,
                Btn::Secondary,
            )
        } else if can_send {
            self.work_button(
                "send",
                if self.work.mode == 2 {
                    "生成图片"
                } else {
                    "开始工作"
                },
                WorkAction::Send,
                Btn::Primary,
            )
        } else {
            self.disabled_button(if self.selected.is_none() {
                "请连接手机"
            } else if needs_workspace {
                "请选择文件夹"
            } else if self.fields[F_WORK_PROMPT].text.trim().is_empty() {
                "开始工作"
            } else {
                "正在同步"
            })
        });
        composer.push(self.hstack(S2, composer_actions));
        let header = self.vstack(
            S2,
            vec![
                self.between(
                    self.line_w(
                        session
                            .map(|s| session_title(&s.title))
                            .unwrap_or_else(|| "新工作".into()),
                        HEADING,
                        p.text,
                        Weight::Semibold,
                    ),
                    self.tag(
                        if self.work.running || phone_pending {
                            "进行中"
                        } else {
                            "工作台"
                        },
                        if self.work.running || phone_pending {
                            p.success
                        } else {
                            p.text2
                        },
                    ),
                ),
                self.segmented(
                    "work-mode",
                    vec![
                        (
                            "手机处理".into(),
                            self.work.mode == 0,
                            Action::Work(WorkAction::Mode(0)),
                        ),
                        (
                            "桌面协作".into(),
                            self.work.mode == 1,
                            Action::Work(WorkAction::Mode(1)),
                        ),
                        (
                            "图片创作".into(),
                            self.work.mode == 2,
                            Action::Work(WorkAction::Mode(2)),
                        ),
                    ],
                ),
            ],
        );
        let children = vec![
            header,
            Expanded::new(
                1.,
                self.chat_scroll(self.selectable_messages(self.vstack(S3, stream))),
            )
            .finish(),
            Container::new(self.vstack(S2, composer))
                .with_uniform_padding(S3)
                .with_background_color(p.bg)
                .with_border(Border::all(1.).with_border_color(
                    if self.focus == Some(F_WORK_PROMPT) {
                        p.border_strong
                    } else {
                        p.border
                    },
                ))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(16.)))
                .finish(),
            self.amoo_usage_footer(),
        ];
        Container::new(
            Flex::column()
                .with_main_axis_size(MainAxisSize::Max)
                .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                .with_spacing(S3)
                .with_children(children)
                .finish(),
        )
        .with_horizontal_padding(S4)
        .with_vertical_padding(S4)
        .finish()
    }

    fn work_workspace_pane(&self) -> El {
        let p = self.p();
        let mut seen = HashSet::new();
        let remote_files: Vec<_> = self
            .amoo_sessions
            .get(self.amoo_session_idx)
            .into_iter()
            .flat_map(|session| session.items.iter().enumerate())
            .filter(|(_, item)| match item {
                AmooChatItem::Attachment { url, path, .. } => {
                    let identity = if url.is_empty() { path } else { url };
                    !identity.is_empty() && seen.insert(identity.clone())
                }
                _ => false,
            })
            .collect();
        let mut items = vec![self.between(
            self.work_title(UiIcon::File, "成果预览"),
            self.work_action_icon(
                "work-preview-hide",
                "收起预览",
                UiIcon::Close,
                Action::Work(WorkAction::Panel(2)),
            ),
        )];
        if let Some(path) = &self.work.local_preview_loading {
            items.push(self.panel(vec![
                self.line("正在读取文件…", CAPTION, p.text2),
                self.line(
                    path.file_name().unwrap_or_default().to_string_lossy(),
                    MICRO,
                    p.text3,
                ),
            ]));
        } else if let Some(preview) = &self.work.local_preview {
            let mut content = vec![
                self.between(
                    self.line_w(&preview.name, BODY, p.text, Weight::Semibold),
                    self.work_button("preview-close", "×", WorkAction::ClosePreview, Btn::Ghost),
                ),
                self.line(format_bytes(preview.bytes), MICRO, p.text3),
            ];
            if preview.image {
                content.push(
                    ConstrainedBox::new(
                        Image::new(
                            facloud_ui::assets::asset_cache::AssetSource::LocalFile {
                                path: preview.path.to_string_lossy().into_owned(),
                            },
                            CacheOption::BySize,
                        )
                        .contain()
                        .finish(),
                    )
                    .with_height(216.)
                    .finish(),
                );
            }
            if let Some(text) = &preview.text {
                let key = "work-local-preview-full";
                let expanded = self.amoo_expanded_tool_records.contains(key);
                let long = text.chars().count() > 1400;
                let visible = if long && !expanded {
                    format!("{}\n\n…", text.chars().take(1400).collect::<String>())
                } else {
                    text.clone()
                };
                content.push(if preview.markdown {
                    self.work_rich_text(&visible, "workspace-preview")
                } else {
                    self.text(visible, MICRO, p.text)
                });
                if long {
                    content.push(self.link_muted(
                        "work-preview-show-full",
                        if expanded {
                            "收起正文"
                        } else {
                            "展开完整正文"
                        },
                        Action::AmooToggleToolRecords(key.into()),
                    ));
                }
            }
            content.push(self.text(&preview.note, MICRO, p.text3));
            content.push(self.wrap(vec![
                    self.work_button(
                        "preview-open",
                        if preview
                            .path
                            .extension()
                            .and_then(|e| e.to_str())
                            .is_some_and(|e| {
                                e.eq_ignore_ascii_case("html") || e.eq_ignore_ascii_case("htm")
                            })
                        {
                            "↗ 在浏览器运行"
                        } else {
                            "↗ 打开原文件"
                        },
                        WorkAction::OpenLocalPreview,
                        Btn::Secondary,
                    ),
                    self.work_button(
                        "preview-export",
                        "↓ 导出",
                        WorkAction::ExportLocalPreview,
                        Btn::Secondary,
                    ),
                ]));
            items.push(self.panel(content));
        } else if let Some(error) = &self.work.local_preview_error {
            items.push(self.panel(vec![
                self.text(error, CAPTION, p.danger),
                self.work_button(
                    "preview-error-close",
                    "关闭预览",
                    WorkAction::ClosePreview,
                    Btn::Ghost,
                ),
            ]));
        } else {
            let remote = self
                .amoo_sessions
                .get(self.amoo_session_idx)
                .into_iter()
                .flat_map(|session| session.items.iter().rev())
                .find(|item| match item {
                    AmooChatItem::Attachment { url, path, .. } => self
                        .work
                        .remote_preview_url
                        .as_ref()
                        .is_none_or(|selected| selected == if url.is_empty() { path } else { url }),
                    _ => false,
                });
            if let Some(AmooChatItem::Attachment {
                name,
                kind,
                mime,
                url,
                size_bytes,
                path,
            }) = remote
            {
                items.push(self.amoo_phone_attachment_card(
                    name,
                    kind,
                    mime,
                    url,
                    path,
                    *size_bytes,
                ));
            } else {
                items.push(self.panel(vec![
                    self.ui_icon(UiIcon::Image, 30., p.text3),
                    self.line_w("把成果放在眼前", HEADING, p.text, Weight::Medium),
                    self.text(
                        "选中交付文件即可预览。图片、Markdown、表格和文档内容会显示在这里。",
                        CAPTION,
                        p.text3,
                    ),
                ]));
            }
        }
        items.push(self.between(
            self.work_title(
                UiIcon::Download,
                format!(
                    "成果文件 · {}",
                    self.work.artifacts.len() + remote_files.len()
                ),
            ),
            self.work_button("files-refresh", "↻", WorkAction::RefreshFiles, Btn::Ghost),
        ));
        if self.work.artifacts.is_empty() && remote_files.is_empty() {
            items.push(self.text("任务产生的图片、文档和表格会归集到这里。", MICRO, p.text3));
        }
        for (index, item) in remote_files {
            if let AmooChatItem::Attachment {
                name,
                kind,
                url,
                path,
                size_bytes,
                ..
            } = item
            {
                let identity = if url.is_empty() { path } else { url };
                let selected = self.work.local_preview.is_none()
                    && self
                        .work
                        .remote_preview_url
                        .as_ref()
                        .is_some_and(|value| value == identity);
                let cached = self.work.image_previews.contains_key(identity);
                items.push(self.list_row(
                    format!("work-remote-file-{index}"),
                    self.hstack(
                        S2,
                        vec![
                                self.ui_icon(
                                    if kind == "image" {
                                        UiIcon::Image
                                    } else {
                                        UiIcon::File
                                    },
                                    16.,
                                    p.text2,
                                ),
                                Expanded::new(
                                    1.,
                                    self.vstack(
                                        S1,
                                        vec![
                                            self.line_w(name, CAPTION, p.text, Weight::Medium),
                                            self.line(
                                                format!(
                                                    "{}{}",
                                                    if cached {
                                                        "已缓存到电脑"
                                                    } else {
                                                        "手机任务附件"
                                                    },
                                                    size_bytes
                                                        .map(|size| format!(
                                                            " · {}",
                                                            format_bytes(size)
                                                        ))
                                                        .unwrap_or_default()
                                                ),
                                                MICRO,
                                                p.text3,
                                            ),
                                        ],
                                    ),
                                )
                                .finish(),
                                self.ui_icon(UiIcon::ChevronRight, 14., p.text3),
                            ],
                    ),
                    selected,
                    Some(Action::Work(WorkAction::PreviewRemote(index))),
                    None,
                ));
            }
        }
        for (index, artifact) in self.work.artifacts.iter().enumerate() {
            let selected = self
                .work
                .local_preview
                .as_ref()
                .is_some_and(|preview| preview.path == artifact.path);
            items.push(self.list_row(
                format!("work-artifact-{index}"),
                self.hstack(
                    S2,
                    vec![
                            self.ui_icon(UiIcon::File, 16., p.text2),
                            Expanded::new(
                                1.,
                                self.vstack(
                                    S1,
                                    vec![
                                        self.line_w(
                                            artifact
                                                .path
                                                .file_name()
                                                .unwrap_or_default()
                                                .to_string_lossy(),
                                            CAPTION,
                                            p.text,
                                            Weight::Medium,
                                        ),
                                        self.line(
                                            format!("本地文件 · {}", artifact.kind),
                                            MICRO,
                                            p.text3,
                                        ),
                                    ],
                                ),
                            )
                            .finish(),
                            self.ui_icon(UiIcon::ChevronRight, 14., p.text3),
                        ],
                ),
                selected,
                Some(Action::Work(WorkAction::PreviewArtifact(index))),
                Some(Action::Work(WorkAction::OpenArtifact(index))),
            ));
        }
        items.push(self.divider());
        items.push(self.work_title(UiIcon::Folder, "工作资料"));
        if let Some(path) = &self.work.workspace {
            items.push(self.line(path.to_string_lossy(), MICRO, p.text3));
            items.push(self.wrap(vec![
                self.work_button(
                    "workspace-open",
                    "打开文件夹",
                    WorkAction::OpenWorkspace,
                    Btn::Ghost,
                ),
                self.work_button(
                    "workspace-change",
                    "更换",
                    WorkAction::ChooseWorkspace,
                    Btn::Ghost,
                ),
            ]));
            let files_key = "work-workspace-files-expanded";
            let expanded = self.amoo_expanded_tool_records.contains(files_key);
            items.push(self.button(
                "work-workspace-files-toggle",
                format!(
                    "{} 浏览资料 · {} 项",
                    if expanded { "▾" } else { "›" },
                    self.work.files.len()
                ),
                Action::AmooToggleToolRecords(files_key.into()),
                Btn::Ghost,
            ));
            if expanded && self.work.files.is_empty() {
                items.push(self.text("文件夹为空，可以添加资料开始工作。", MICRO, p.text3));
            }
            for (index, file) in self.work.files.iter().enumerate().filter(|_| expanded) {
                let selected = self
                    .work
                    .local_preview
                    .as_ref()
                    .is_some_and(|preview| preview.path == file.path);
                items.push(self.list_row(
                    format!("work-file-{index}"),
                    self.hstack(
                        S2,
                        vec![
                            self.ui_icon(
                                if file.is_dir {
                                    UiIcon::Folder
                                } else {
                                    UiIcon::File
                                },
                                14.,
                                p.text2,
                            ),
                            Expanded::new(1., self.line(&file.name, CAPTION, p.text)).finish(),
                            self.line(
                                if file.is_dir {
                                    "文件夹".into()
                                } else {
                                    format_bytes(file.bytes)
                                },
                                MICRO,
                                p.text3,
                            ),
                        ],
                    ),
                    selected,
                    Some(Action::Work(WorkAction::PreviewFile(index))),
                    Some(Action::Work(WorkAction::OpenFile(index))),
                ));
            }
        } else {
            items.push(self.text(
                "选择文件夹，供桌面协作读取资料并保存成果。",
                CAPTION,
                p.text3,
            ));
            items.push(self.work_button(
                "workspace-choose",
                "📁 选择工作文件夹",
                WorkAction::ChooseWorkspace,
                Btn::Secondary,
            ));
        }
        Container::new(self.pane_scroll(self.work.files_scroll.clone(), self.vstack(S3, items)))
            .with_uniform_padding(S3)
            .with_background_color(p.sidebar)
            .with_border(Border::left(1.).with_border_color(p.border))
            .finish()
    }

    fn work_desktop_workflows(&self) -> El {
        let p = self.p();
        let query = self.fields[F_WORK_CAP_SEARCH].text.trim().to_lowercase();
        let matches = |text: &str| query.is_empty() || text.to_lowercase().contains(&query);
        let mut desktop_workflows = vec![
            self.between(
                self.work_title(UiIcon::Folder, "桌面工作流"),
                if self.work.workspace.is_some() {
                    self.work_button(
                        "import-workflow",
                        "导入工作流",
                        WorkAction::ImportDesktopSkill,
                        Btn::Secondary,
                    )
                } else {
                    self.work_button(
                        "workflow-choose-workspace",
                        "选择工作文件夹",
                        WorkAction::ChooseWorkspace,
                        Btn::Secondary,
                    )
                },
            ),
            self.text(
                "导入 SKILL.md，为文件任务补充处理流程。读取流程说明，不自动运行附带脚本。",
                CAPTION,
                p.text2,
            ),
        ];
        for skill in &self.work.desktop_skills {
            if !matches(&format!("{} {}", skill.name, skill.description)) {
                continue;
            }
            desktop_workflows.push(self.vstack(
                S1,
                vec![
                    self.line_w(&skill.name, BODY, p.text, Weight::Medium),
                    self.text(&skill.description, CAPTION, p.text2),
                    self.line(skill.path.to_string_lossy(), MICRO, p.text3),
                ],
            ));
        }
        if self.work.desktop_skills.is_empty() {
            desktop_workflows.push(self.text("当前工作文件夹尚未导入工作流。", CAPTION, p.text3));
        }
        self.panel(desktop_workflows)
    }

    pub(in super::super) fn work_catalog_pane(&self) -> El {
        let p = self.p();
        let Some(catalog) = &self.work.catalog else {
            let unavailable = self.empty(
                if self.work.catalog_loading {
                    "正在读取手机能力"
                } else {
                    "能力尚未同步"
                },
                self.work
                    .error
                    .as_deref()
                    .unwrap_or("连接手机后同步模型目录、技能、智能体与 AI 应用。"),
                Some(self.work_button(
                    "catalog-refresh",
                    "重新同步",
                    WorkAction::Refresh,
                    Btn::Secondary,
                )),
            );
            return if self.work.tab == 2 {
                Container::new(self.pane_scroll(
                    self.scroll.clone(),
                    self.vstack(S4, vec![self.work_desktop_workflows(), unavailable]),
                ))
                .with_uniform_padding(S4)
                .finish()
            } else {
                unavailable
            };
        };
        let query = self.fields[F_WORK_CAP_SEARCH].text.trim().to_lowercase();
        let matches = |text: &str| query.is_empty() || text.to_lowercase().contains(&query);
        let availability = match self.work.tab {
            1 => &catalog.models_status,
            2 => &catalog.skills_status,
            3 => &catalog.agents_status,
            _ => &catalog.ai_apps_status,
        };
        let search = self.input(F_WORK_CAP_SEARCH, "搜索名称、说明或标识…", None);
        let mut rows = vec![if self.work.tab == 2 {
            self.between(
                search,
                if self.busy {
                    self.disabled_button("正在搜索")
                } else {
                    self.work_button(
                        "search-skill-market",
                        "搜索市场",
                        WorkAction::SearchSkills,
                        Btn::Secondary,
                    )
                },
            )
        } else {
            search
        }];
        rows.push(self.text(
            if self.work.tab == 1 {
                "来源：手机 Amoo 云端。只展示当前账号通过真实请求验证的模型；验证结果有效期为 30 分钟。"
            } else {
                "来源：手机 OpenClaw。技能与智能体属于手机运行环境，和 Amoo 云端会话能力分别展示。"
            },
            CAPTION, p.text3,
        ));
        if !availability.available || !availability.message.is_empty() {
            rows.push(self.panel(vec![self.text(&availability.message, BODY, p.text2)]));
        }
        if !self.work.status.is_empty() {
            rows.push(self.text(&self.work.status, CAPTION, p.text3));
        }
        if let Some(error) = &self.work.error {
            rows.push(self.panel(vec![self.text(error, BODY, p.danger)]));
        }
        let mut count = 0;
        match self.work.tab {
            1 => {
                rows.push(self.panel(vec![self.between(
                    self.vstack(
                        S1,
                        vec![
                            self.heading("当前账号的模型权限"),
                            self.text(
                                "验证会发送一条短测试请求，Token 计入手机账号用量。",
                                CAPTION,
                                p.text2,
                            ),
                        ],
                    ),
                    if self.busy || self.work.running {
                        self.disabled_button("处理中")
                    } else {
                        self.work_button(
                            "verify-models",
                            "验证可用模型",
                            WorkAction::VerifyModels,
                            Btn::Secondary,
                        )
                    },
                )]));
                rows.push(self.panel(vec![self.between(
                    self.vstack(
                        S1,
                        vec![
                            self.heading("跟随手机默认"),
                            self.text(
                                "由手机 Amoo 自动选择；下方显示实际调用记录，当前不支持手动指定。",
                                CAPTION,
                                p.text2,
                            ),
                        ],
                    ),
                    self.work_button(
                        "model-default",
                        if self.work.selected_model.is_none() {
                            "已选择"
                        } else {
                            "使用默认"
                        },
                        WorkAction::SelectModel(None),
                        Btn::Secondary,
                    ),
                )]));
                let mut models: Vec<_> = catalog
                    .models
                    .iter()
                    .filter(|model| {
                        model.is_verified_for(self.amoo_ui.account_scope.as_deref())
                            && matches(&format!("{} {} {}", model.name, model.id, model.provider))
                    })
                    .collect();
                models.sort_by(|a, b| b.available.cmp(&a.available).then(a.name.cmp(&b.name)));
                let total = models.len();
                let last_page = total.saturating_sub(1) / 40;
                let page = self.work.catalog_page.min(last_page);
                if total > 40 {
                    rows.push(self.between(
                        self.line(
                            format!(
                                "{} 个已验证模型 · 第 {} / {} 页",
                                total,
                                page + 1,
                                last_page + 1
                            ),
                            CAPTION,
                            p.text2,
                        ),
                        self.wrap(vec![
                            if page > 0 {
                                self.work_button(
                                    "models-prev",
                                    "上一页",
                                    WorkAction::CatalogPage(page - 1),
                                    Btn::Secondary,
                                )
                            } else {
                                self.disabled_button("上一页")
                            },
                            if page < last_page {
                                self.work_button(
                                    "models-next",
                                    "下一页",
                                    WorkAction::CatalogPage(page + 1),
                                    Btn::Secondary,
                                )
                            } else {
                                self.disabled_button("下一页")
                            },
                        ]),
                    ));
                } else {
                    rows.push(self.line(
                        format!("{total} 个通过当前账号验证的模型"),
                        CAPTION,
                        p.text2,
                    ));
                }
                for model in models.into_iter().skip(page * 40).take(40) {
                    count += 1;
                    let selected = self.work.selected_model.as_deref() == Some(model.id.as_str());
                    rows.push(self.panel(vec![
                        self.between(
                            self.work_title(UiIcon::Cpu, &model.name),
                            if catalog.can_select_model
                                && model.is_verified_for(self.amoo_ui.account_scope.as_deref())
                            {
                                self.work_button(
                                    &format!("model-{}", model.id),
                                    if selected {
                                        "已选择"
                                    } else {
                                        "使用模型"
                                    },
                                    WorkAction::SelectModel(Some(model.id.clone())),
                                    Btn::Secondary,
                                )
                            } else {
                                self.tag("手机自动选择", p.text3)
                            },
                        ),
                        self.mono(&model.id, CAPTION, p.text2),
                        self.wrap(vec![
                            self.tag(&model.provider, p.text3),
                            self.tag(
                                if model.current {
                                    "最近验证通过"
                                } else if model.available {
                                    "已验证可用"
                                } else {
                                    "目录模型"
                                },
                                p.text3,
                            ),
                        ]),
                        self.text(
                            if model.capabilities.is_empty() {
                                "手机未提供能力标签".into()
                            } else {
                                model.capabilities.join(" · ")
                            },
                            CAPTION,
                            p.text3,
                        ),
                        self.text(&model.availability_reason, CAPTION, p.text3),
                    ]));
                }
            }
            2 => {
                rows.push(self.work_desktop_workflows());
                for (market, skills) in [(true, &self.work.market_skills), (false, &catalog.skills)]
                {
                    if market {
                        rows.push(self.work_title(UiIcon::Search, "手机技能市场"));
                        rows.push(self.text("在上方输入关键词，再搜索手机 OpenClaw 的 ClawHub 技能目录。安装前会显示目标并确认。", CAPTION, p.text3));
                        if self.work.market_skills.is_empty() {
                            rows.push(self.text("尚无市场搜索结果。", CAPTION, p.text3));
                        }
                    } else {
                        rows.push(self.work_title(UiIcon::Smartphone, "已安装手机技能"));
                    }
                    for skill in skills {
                        if !market
                            && !matches(&format!(
                                "{} {} {}",
                                skill.name, skill.id, skill.description
                            ))
                        {
                            continue;
                        }
                        count += 1;
                        let mut actions = Vec::new();
                        if skill.installed {
                            actions.push(self.work_button(
                                &format!("skill-toggle-{}", skill.id),
                                if skill.enabled { "停用" } else { "启用" },
                                WorkAction::ToggleSkill {
                                    key: skill.id.clone(),
                                    enabled: !skill.enabled,
                                },
                                Btn::Secondary,
                            ));
                            actions.push(if self.busy || self.work.running {
                                self.disabled_button("用于桌面任务")
                            } else {
                                self.work_button(
                                    &format!("skill-use-{}", skill.id),
                                    "用于桌面任务",
                                    WorkAction::UsePhoneSkill(skill.id.clone()),
                                    Btn::Primary,
                                )
                            });
                        } else {
                            for installer in &skill.installers {
                                actions.push(self.work_button(
                                    &format!("skill-install-{}-{}", skill.id, installer.id),
                                    &format!("安装 · {}", installer.label),
                                    WorkAction::InstallSkill {
                                        key: skill.id.clone(),
                                        install_id: installer.id.clone(),
                                    },
                                    Btn::Secondary,
                                ));
                            }
                        }
                        let description_key = format!("work-skill-description-{}", skill.id);
                        let description_expanded =
                            self.amoo_expanded_tool_records.contains(&description_key);
                        let long_description = skill.description.chars().count() > 180;
                        let description = if long_description && !description_expanded {
                            format!(
                                "{}…",
                                skill.description.chars().take(180).collect::<String>()
                            )
                        } else {
                            skill.description.clone()
                        };
                        let mut details = vec![
                            self.between(
                                self.work_title(UiIcon::Tools, &skill.name),
                                self.tag(
                                    if skill.installed {
                                        if skill.enabled {
                                            "已启用"
                                        } else {
                                            "已停用"
                                        }
                                    } else {
                                        "未安装"
                                    },
                                    p.text2,
                                ),
                            ),
                            self.text(description, BODY, p.text2),
                            self.mono(&skill.id, MICRO, p.text3),
                            self.line(format!("来源：{}", skill.source), MICRO, p.text3),
                            if actions.is_empty() {
                                self.text("手机未提供可用安装方式", CAPTION, p.text3)
                            } else {
                                self.wrap(actions)
                            },
                        ];
                        if long_description {
                            details.push(self.button(
                                format!("{description_key}-toggle"),
                                if description_expanded {
                                    "收起说明"
                                } else {
                                    "展开说明"
                                },
                                Action::AmooToggleToolRecords(description_key),
                                Btn::Ghost,
                            ));
                        }
                        if let Some((key, installer)) = self
                            .work
                            .pending_install
                            .as_ref()
                            .filter(|(key, _)| *key == skill.id)
                        {
                            details.push(self.confirm_bar(
                                format!(
                                    "将通过 {installer} 在手机 OpenClaw 安装 {}。来源：{} · {}",
                                    skill.name,
                                    skill.source,
                                    if skill.install_reference.is_empty() {
                                        &skill.id
                                    } else {
                                        &skill.install_reference
                                    }
                                ),
                                false,
                                vec![
                                    self.work_button(
                                        &format!("skill-cancel-{key}"),
                                        "取消",
                                        WorkAction::CancelSkillInstall,
                                        Btn::Secondary,
                                    ),
                                    self.work_button(
                                        &format!("skill-confirm-{key}"),
                                        "确认安装",
                                        WorkAction::ConfirmSkillInstall,
                                        Btn::Primary,
                                    ),
                                ],
                            ));
                        }
                        rows.push(self.panel(details));
                    }
                }
                let key = "amoo-work-phone-tool-catalog";
                let expanded = self.amoo_expanded_tool_records.contains(key) || !query.is_empty();
                let tools: Vec<_> = catalog
                    .tools
                    .iter()
                    .filter(|tool| matches(&format!("{} {} {}", tool.id, tool.name, tool.group)))
                    .collect();
                let mut tool_rows = vec![
                    self.button(
                        "work-phone-tools",
                        format!(
                            "{} 手机工具目录 · {} 项",
                            if expanded { "▾" } else { "›" },
                            tools.len()
                        ),
                        Action::AmooToggleToolRecords(key.into()),
                        Btn::Ghost,
                    ),
                    self.text(&catalog.tools_status.message, CAPTION, p.text3),
                ];
                if expanded {
                    for tool in tools {
                        tool_rows.push(self.vstack(
                            S1,
                            vec![
                                self.line_w(&tool.name, BODY, p.text, Weight::Medium),
                                self.mono(&tool.id, MICRO, p.text2),
                                self.line(
                                    format!("{} · {}", tool.group, tool.source),
                                    MICRO,
                                    p.text3,
                                ),
                            ],
                        ));
                    }
                }
                rows.push(self.panel(tool_rows));
            }
            3 => {
                rows.push(self.panel(vec![
                    self.work_title(UiIcon::Plus, "新建手机智能体"),
                    self.text(
                        "新建在手机 OpenClaw 工作空间。当前 Amoo 云端会话仍使用原有通道。",
                        CAPTION,
                        p.text2,
                    ),
                    self.between(
                        self.input(F_WORK_AGENT_NAME, "输入智能体名称…", None),
                        if self.busy || !catalog.agents_status.available {
                            self.disabled_button("新建智能体")
                        } else {
                            self.work_button(
                                "create-agent",
                                "新建智能体",
                                WorkAction::CreateAgent,
                                Btn::Primary,
                            )
                        },
                    ),
                ]));
                for agent in &catalog.agents {
                    if !matches(&format!(
                        "{} {} {}",
                        agent.name, agent.id, agent.description
                    )) {
                        continue;
                    }
                    count += 1;
                    rows.push(self.panel(vec![
                        self.between(
                            self.work_title(UiIcon::Star, &agent.name),
                            self.work_button(
                                &format!("agent-{}", agent.id),
                                "查看信息",
                                WorkAction::SelectAgent(agent.id.clone()),
                                Btn::Secondary,
                            ),
                        ),
                        self.text(&agent.description, BODY, p.text2),
                        self.mono(&agent.id, MICRO, p.text3),
                        self.line(
                            if agent.model.is_empty() {
                                "使用智能体默认模型"
                            } else {
                                &agent.model
                            },
                            CAPTION,
                            p.text3,
                        ),
                    ]));
                }
            }
            _ => {
                rows.push(self.panel(vec![
                    self.work_title(UiIcon::Apps, "安装与部署"),
                    self.text(
                        "安装本地 APK 到手机，或在 Studio 管理并部署 AI 应用。",
                        BODY,
                        p.text2,
                    ),
                    self.wrap(vec![
                        self.button(
                            "work-app-install",
                            "安装 APK",
                            Action::InstallApk,
                            Btn::Secondary,
                        ),
                        self.button(
                            "work-app-list",
                            "查看已安装应用",
                            Action::Page(Page::Apps),
                            Btn::Secondary,
                        ),
                        self.button(
                            "work-app-studio",
                            "打开 Studio",
                            Action::Page(Page::Studio),
                            Btn::Secondary,
                        ),
                    ]),
                ]));
                for app in &catalog.ai_apps {
                    if !matches(&format!("{} {} {}", app.name, app.id, app.description)) {
                        continue;
                    }
                    count += 1;
                    rows.push(self.panel(vec![
                        self.between(
                            self.work_title(UiIcon::Apps, &app.name),
                            self.tag(
                                if app.installed {
                                    "手机已安装"
                                } else {
                                    "未安装"
                                },
                                p.text2,
                            ),
                        ),
                        self.text(&app.description, BODY, p.text2),
                        self.mono(&app.id, MICRO, p.text3),
                    ]));
                }
            }
        }
        if count == 0 && availability.available {
            rows.push(self.text(
                if query.is_empty() {
                    "手机当前未返回此类能力。"
                } else {
                    "没有匹配的结果。"
                },
                BODY,
                p.text3,
            ));
        }
        Container::new(self.pane_scroll(self.scroll.clone(), self.vstack(S3, rows)))
            .with_uniform_padding(S4)
            .finish()
    }

    pub(in super::super) fn work_page(&self) -> El {
        let p = self.p();
        let capabilities = self.work.capabilities_open;
        let header = Container::new(self.between(
            if capabilities {
                self.hstack(
                    S2,
                    vec![
                        self.work_button(
                            "back-work",
                            "← 返回工作",
                            WorkAction::Capabilities,
                            Btn::Secondary,
                        ),
                        self.segmented(
                            "work-capability-tabs",
                            ["模型", "技能", "智能体", "AI 应用"]
                                .iter()
                                .enumerate()
                                .map(|(index, label)| {
                                    (
                                        (*label).to_string(),
                                        self.work.tab == index as u8 + 1,
                                        Action::Work(WorkAction::Tab(index as u8 + 1)),
                                    )
                                })
                                .collect(),
                        ),
                    ],
                )
            } else {
                self.hstack(
                    S2,
                    vec![
                        self.work_button(
                            "toggle-sessions",
                            if self.work.panel == 1 {
                                "☰ 收起记录"
                            } else {
                                "☰ 工作记录"
                            },
                            WorkAction::Panel(1),
                            if self.work.panel == 1 {
                                Btn::Secondary
                            } else {
                                Btn::Ghost
                            },
                        ),
                        self.work_button(
                            "toggle-files",
                            if self.work.panel == 2 {
                                "📄 收起预览"
                            } else {
                                "📄 成果与预览"
                            },
                            WorkAction::Panel(2),
                            if self.work.panel == 2 {
                                Btn::Secondary
                            } else {
                                Btn::Ghost
                            },
                        ),
                        self.work_button(
                            "capabilities",
                            "⊟ 能力中心",
                            WorkAction::Capabilities,
                            Btn::Ghost,
                        ),
                    ],
                )
            },
            self.hstack(
                S2,
                vec![
                    self.dot(if self.amoo_gateway_online {
                        p.success
                    } else {
                        p.text3
                    }),
                    self.line(
                        if self.amoo_gateway_online {
                            "手机已连接"
                        } else if self.work.catalog_loading {
                            "正在同步"
                        } else {
                            "等待连接"
                        },
                        CAPTION,
                        p.text2,
                    ),
                ],
            ),
        ))
        .with_horizontal_padding(S4)
        .with_vertical_padding(S2)
        .with_border(Border::bottom(1.).with_border_color(p.border))
        .finish();
        let body = if capabilities {
            self.work_catalog_pane()
        } else {
            let mut panes = Vec::new();
            if self.work.panel == 1 {
                panes.push(self.fixed_w(220., self.work_session_pane()));
            }
            panes.push(Expanded::new(1., self.work_conversation_pane()).finish());
            if self.work.panel == 2 {
                panes.push(self.fixed_w(308., self.work_workspace_pane()));
            }
            Flex::row()
                .with_main_axis_size(MainAxisSize::Max)
                .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                .with_children(panes)
                .finish()
        };
        Flex::column()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_child(header)
            .with_child(Expanded::new(1., body).finish())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::work_message_text;

    #[test]
    fn desktop_protocol_preserves_task_and_hides_tool_feedback() {
        let task = "<amoo_work_protocol>\n[Amoo Work 桌面协作]\n用户任务：请整理会议纪要\n保留负责人\n用户已选择一个本地工作目录。附件相对路径：[]\n内部工具说明\n</amoo_work_protocol>";
        assert_eq!(
            work_message_text(task, true).as_deref(),
            Some("请整理会议纪要\n保留负责人")
        );
        assert!(work_message_text(
            "<amoo_work_protocol>\nAmoo Work 桌面执行结果（第 1 轮）\n{}\n</amoo_work_protocol>",
            true
        )
        .is_none());
    }

    #[test]
    fn desktop_plan_displays_real_summary_without_protocol() {
        let text = "<amoo_work_protocol>\n{\"amoo_work\":1,\"summary\":\"已生成会议纪要\",\"done\":true,\"tools\":[]}\n</amoo_work_protocol>";
        assert_eq!(
            work_message_text(text, false).as_deref(),
            Some("已生成会议纪要")
        );
        assert_eq!(
            work_message_text("{\"amoo_work\":1,\"summary\":\"正在读取资料\"}", false).as_deref(),
            Some("正在读取资料")
        );
    }

    #[test]
    fn ordinary_messages_and_json_are_not_filtered() {
        let plain = "请解释 amoo_work 字段的意义";
        assert_eq!(work_message_text(plain, true).as_deref(), Some(plain));
        let json = "{\"summary\":\"普通 JSON 示例\"}";
        assert_eq!(work_message_text(json, false).as_deref(), Some(json));
    }
}

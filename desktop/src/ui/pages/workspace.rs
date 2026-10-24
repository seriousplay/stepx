//! Amoo: one task flow, contextual resources and a dismissible result preview.
use super::super::workspace_state::WorkspaceAction as W;
use super::super::{icons::UiIcon, theme::*, widgets::*, *};
use facloud_ui::{assets::asset_cache::AssetSource, fonts::Weight};
use stepx_companion::amoo_workspace::{Executor, Phase};

impl Companion {
    fn workspace_column(&self, content: El, compact: bool, id: &str) -> El {
        Align::new(ConstrainedBox::new(Container::new(SavePosition::new(content, id).for_single_frame().finish())
            .with_horizontal_padding(if compact { 12. } else { 24. }).finish())
            .with_max_width(928.).finish()).top_center().finish()
    }
    fn ws_button(&self, id: &str, label: &str, icon: UiIcon, action: W, kind: Btn) -> El {
        self.button_icon(
            format!("workspace-{id}"),
            label,
            icon,
            Action::Workspace(action),
            kind,
        )
    }
    pub(in super::super) fn workspace_actions(&self) -> Vec<El> {
        if self.amoo_ui.screen_mode {
            return vec![self.button_icon(
                "workspace-back-gui",
                "返回对话",
                UiIcon::ArrowLeft,
                Action::AmooUi(AmooUiAction::GuiMode(false)),
                Btn::Secondary,
            )];
        }
        vec![
            self.ws_button(
                "history",
                "会话记录",
                UiIcon::Clock,
                W::History,
                Btn::Secondary,
            ),
            self.ws_button(
                "resources",
                "资料与能力",
                UiIcon::Attach,
                W::Menu(2),
                Btn::Ghost,
            ),
            self.ws_button("new", "新对话", UiIcon::Plus, W::New, Btn::Secondary),
        ]
    }
    pub(in super::super) fn workspace_page(&self, compact: bool) -> El {
        let p = self.p();
        if self.amoo_ui.screen_mode {
            return self.amoo_page();
        }
        if let Some(album) = self.amoo_album_picker() {
            return self.pane_scroll(
                self.scroll_aux.clone(),
                Container::new(album).with_uniform_padding(S4).finish(),
            );
        }
        if self.amoo_show_schedules {
            return self.pane_scroll(
                self.scroll_aux.clone(),
                Container::new(self.amoo_schedule_center())
                    .with_uniform_padding(S4)
                    .finish(),
            );
        }
        if (compact || self.workspace.viewport.x() < 1080.) && (self.workspace.panel != 0 || self.workspace.history) {
            return self.workspace_side(compact);
        }
        let active = self.workspace_task();
        let started = active
            .is_some_and(|t| !t.turns.is_empty() || t.session.is_some() || !t.outputs.is_empty())
            || self.workspace.proposal.is_some();
        let mut column = Flex::column()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        if started {
            let title = active.map(|t| t.title.as_str()).unwrap_or("新对话");
            column = column.with_child(
                self.workspace_column(Container::new(self.between(
                    self.line_w(
                        Self::truncate_chars(title, if compact { 20 } else { 32 }),
                        SUBHEAD,
                        p.text,
                        Weight::Semibold,
                    ),
                    self.ws_button(
                        "task-model",
                        "模型与账号",
                        UiIcon::ChevronDown,
                        W::Menu(1),
                        Btn::Ghost,
                    ),
                ))
                .with_vertical_padding(if compact && self.workspace.viewport.y() < 520. { S2 } else { S3 })
                .finish(), compact, "workspace-chat-heading"),
            );
            let content = self.workspace_thread();
            column = column.with_child(
                Expanded::new(
                    1.,
                    self.chat_scroll(
                        self.workspace_column(Container::new(self.selectable_messages(content)).with_padding_bottom(S4).finish(), compact, "workspace-chat-messages"),
                    ),
                )
                .finish(),
            );
            column = column.with_child(
                Container::new(self.workspace_column(self.amoo_message_composer(compact, false), compact, "workspace-chat-input"))
                    .with_vertical_padding(if compact && self.workspace.viewport.y() < 520. { S2 } else { S3 })
                    .with_border(Border::top(1.).with_border_color(p.border))
                    .finish(),
            );
        } else {
            let mut content = vec![
                Align::new(self.line_w("你好，想聊些什么？", if compact { 24. } else { 30. }, p.text, Weight::Semibold)).finish(),
                Align::new(self.text(if compact { "发消息，也可以添加图片和文件。" } else { "与手机 Amoo 对话，也可以发图片和文件一起聊。" }, BODY, p.text2)).finish(),
                self.amoo_message_composer(compact, false),
            ];
            if let Some(error) = &self.workspace.error {
                content.push(self.text(error, BODY, p.danger));
            }
            // Anchor against the viewport, not the draft's changing measured height.
            // Reserve enough growth room so adding attachments never shifts the field.
            let available = self.workspace.viewport.y() - if compact { 124. } else { 80. };
            let intro = if compact { 80. } else { 88. };
            let (editor, attachments) = self.composer_limits(compact);
            let minimum = if compact { 114. } else { 134. };
            let maximum = editor + attachments + 98.;
            let top = ((available-intro-minimum)/2.).min(available-intro-maximum-16.).max(16.);
            let welcome = self.workspace_column(Container::new(self.vstack(S4, content))
                .with_padding_top(top).with_padding_bottom(16.).finish(), compact, "workspace-chat-welcome");
            column = column.with_child(
                Expanded::new(
                    1.,
                    self.chat_pane_scroll(welcome, false),
                )
                .finish(),
            );
        }
        let main = column.finish();
        if !compact && self.workspace.viewport.x() >= 1080. && (self.workspace.panel != 0 || self.workspace.history) {
            Flex::row()
                .with_main_axis_size(MainAxisSize::Max)
                .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                .with_child(Expanded::new(1., main).finish())
                .with_child(self.fixed_w(352., self.workspace_side(false)))
                .finish()
        } else {
            main
        }
    }
    fn workspace_recent(&self, max: usize) -> Vec<El> {
        let p = self.p();
        let mut tasks = self
            .workspace
            .journal
            .tasks
            .iter()
            .filter(|t| {
                t.meaningful()
                    && self.workspace_visible(t)
                    && t.title
                        .to_lowercase()
                        .contains(&self.fields[F_AMOO_SEARCH].text.to_lowercase())
            })
            .collect::<Vec<_>>();
        tasks.sort_by_key(|t| std::cmp::Reverse(t.updated));
        let mut rows = tasks
            .into_iter()
            .take(max)
            .map(|t| {
                self.list_row(
                    format!("workspace-recent-{}", t.id),
                    self.hstack(
                        S3,
                        vec![
                            self.ui_icon(
                                if t.outputs.is_empty() {
                                    UiIcon::Chat
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
                                        self.line_w(&t.title, BODY, p.text, Weight::Medium),
                                        self.line(
                                            if !t.draft.is_empty() {
                                                "有草稿".into()
                                            } else if t.outputs.is_empty() {
                                                "继续对话".into()
                                            } else {
                                                format!("{} 个成果 · 已保存在电脑", t.outputs.len())
                                            },
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
                    self.workspace.active.as_ref() == Some(&t.id),
                    Some(Action::Workspace(W::Select(t.id.clone()))),
                    None,
                )
            })
            .collect::<Vec<_>>();
        if rows.len() < max {
            for (i, s) in self
                .amoo_sessions
                .iter()
                .enumerate()
                .filter(|(_, s)| {
                    s.title
                        .to_lowercase()
                        .contains(&self.fields[F_AMOO_SEARCH].text.to_lowercase())
                        && !self.workspace.journal.tasks.iter().any(|t| {
                            self.workspace_visible(t) && t.session.as_ref() == Some(&s.session_key)
                        })
                })
                .take(max - rows.len())
            {
                rows.push(self.list_row(
                    format!("workspace-phone-session-{i}"),
                    self.hstack(
                        S3,
                        vec![
                                self.ui_icon(UiIcon::Smartphone, 18., p.text2),
                                Expanded::new(
                                    1.,
                                    self.vstack(
                                        S1,
                                        vec![
                                            self.line(
                                                amoo_display::session_title(&s.title),
                                                BODY,
                                                p.text,
                                            ),
                                            self.line("手机会话", MICRO, p.text3),
                                        ],
                                    ),
                                )
                                .finish(),
                            ],
                    ),
                    false,
                    Some(Action::Workspace(W::PhoneSession(i))),
                    None,
                ));
            }
        }
        if rows.is_empty() {
            rows.push(self.text(
                "还没有对话。发条消息，也可以添加图片和文件。",
                BODY,
                p.text3,
            ));
        }
        rows
    }
    fn workspace_thread(&self) -> El {
        let p = self.p();
        let mut rows = Vec::new();
        if let Some(task) = self.workspace_task() {
            let phone_loaded = !task.steps.iter().any(|s| s.executor == Executor::App) && task.session.as_ref().is_some_and(|key| {
                    self.amoo_sessions
                        .get(self.amoo_session_idx)
                        .is_some_and(|s| &s.session_key == key && !s.items.is_empty())
                });
            if phone_loaded {
                rows.push(self.amoo_conversation());
            } else {
                for (i, turn) in task.turns.iter().enumerate() {
                    rows.push(
                        Container::new(self.work_rich_text(
                            &turn.text,
                            &format!("workspace-turn-{}-{i}", task.id),
                        ))
                        .with_uniform_padding(S4)
                        .with_background_color(if turn.user { p.surface } else { p.bg })
                        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
                        .finish(),
                    );
                }
            }
            let running = self.workspace.running.as_ref().is_some_and(|r| r.task == task.id);
            let failed = task.steps.iter().any(|s| matches!(s.phase, Phase::Failed | Phase::Stopped | Phase::Unconfirmed));
            let has_work = task.steps.iter().any(|s| s.executor != Executor::Phone);
            // Ordinary chat has no workflow card. Pending is already shown in the composer.
            if has_work || failed {
                let key = format!("workspace-log-{}", task.id);
                let expanded = self.amoo_expanded_tool_records.contains(&key);
                let label = if failed { "未完成 · 查看详情" } else if running { "正在处理 · 查看进度" } else { "处理完成 · 查看记录" };
                let mut details = vec![self.between(
                    self.button_icon("workspace-progress-details", label, if expanded { UiIcon::ChevronDown } else { UiIcon::ChevronRight }, Action::AmooToggleToolRecords(key), Btn::Ghost),
                    if running { self.ws_button("stop", "停止", UiIcon::Close, W::Stop, Btn::Ghost) } else { self.line("", MICRO, p.text3) },
                )];
                if running && !expanded {
                    if let Some(run)=&self.workspace.running {
                        let status=match run.executor {Executor::Files=>self.work.status.as_str(),Executor::App=>self.studio_lab.status.as_str(),Executor::Image=>if self.work.status.contains("保存原图") {self.work.status.as_str()}else{"手机正在生成图片，收到原图后自动保存…"},Executor::Phone=>"手机正在回复…"};
                        let brief=status.chars().take(140).collect::<String>();
                        details.push(self.text(brief,CAPTION,p.text2));
                    }
                }
                if expanded || failed {
                    for step in &task.steps {
                        let label = match step.phase { Phase::Ready => "待开始", Phase::Running => "进行中", Phase::Done => "已完成", Phase::Failed => "未完成", Phase::Stopped => "已停止", Phase::Unconfirmed => "待核对" };
                        details.push(self.text(format!("{} · {}", step.executor.label(), label), CAPTION, p.text2));
                        if !step.note.is_empty() { details.push(self.text(&step.note, CAPTION, if failed { p.warning } else { p.text3 })); }
                        if step.phase == Phase::Running {
                            let text = match step.executor { Executor::Files => &self.work.status, Executor::App => &self.studio_lab.status, _ => "正在等待手机返回结果" };
                            details.push(self.text(text, CAPTION, p.text2));
                        }
                    }
                }
                if failed { details.push(self.ws_button("retry", "核对后重试", UiIcon::Retry, W::Retry, Btn::Secondary)); }
                rows.push(Container::new(self.vstack(S2, details)).with_uniform_padding(S2).with_background_color(p.surface).with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_CONTROL))).finish());
            }
            if !task.outputs.is_empty() {
                rows.push(self.heading("生成的文件"));
                for (i, o) in task.outputs.iter().enumerate() {
                    rows.push(self.list_row(
                        format!("workspace-output-{i}"),
                        self.hstack(
                            S3,
                            vec![
                                    self.ui_icon(
                                        if o.lab_id.is_some() {
                                            UiIcon::Apps
                                        } else {
                                            UiIcon::File
                                        },
                                        20.,
                                        p.text2,
                                    ),
                                    Expanded::new(
                                        1.,
                                        self.vstack(
                                            S1,
                                            vec![
                                                self.line_w(&o.name, BODY, p.text, Weight::Medium),
                                                self.line(
                                                    format!("{} · 已保存在电脑", o.kind),
                                                    MICRO,
                                                    p.text3,
                                                ),
                                            ],
                                        ),
                                    )
                                    .finish(),
                                    self.line("预览", CAPTION, p.text2),
                                    self.ui_icon(UiIcon::ChevronRight, 14., p.text3),
                                ],
                        ),
                        self.workspace.selected_output == Some(i),
                        Some(Action::Workspace(W::Preview(i))),
                        None,
                    ));
                }
            }
        }
        if let Some(proposal) = &self.workspace.proposal {
            let mut plan = vec![
                self.heading("确认后开始处理"),
                self.text(&proposal.text, BODY, p.text),
            ];
            for (i, step) in proposal.steps.iter().enumerate() {
                plan.push(self.hstack(
                    S2,
                    vec![
                            self.line(format!("{}", i + 1), CAPTION, p.text3),
                            Expanded::new(
                                1.,
                                self.vstack(
                                    S1,
                                    vec![
                                        self.line_w(step.label(), BODY, p.text, Weight::Medium),
                                        self.line(if *step == Executor::App && !self.studio_lab.config.has_key { "手机 Amoo · 电脑生成与验证" } else { step.source() }, MICRO, p.text2),
                                    ],
                                ),
                            )
                            .finish(),
                        ],
                ));
            }
            if !proposal.files.is_empty() {
                plan.push(self.text(
                    format!(
                            "资料：{}",
                            proposal
                                .files
                                .iter()
                                .map(|p| p.file_name().unwrap_or_default().to_string_lossy())
                                .collect::<Vec<_>>()
                                .join("、")
                        ),
                    CAPTION,
                    p.text2,
                ));
            }
            if proposal
                .steps
                .iter()
                .any(|s| matches!(s, Executor::Files | Executor::App | Executor::Image))
            {
                plan.push(
                    self.between(
                        self.text(
                            self.workspace_task()
                                .and_then(|t| t.workspace.as_ref())
                                .map(|p| format!("保存到：{}", p.display()))
                                .unwrap_or("保存到：电脑上的独立任务文件夹".into()),
                            CAPTION,
                            p.text2,
                        ),
                        self.ws_button(
                            "folder",
                            "更改",
                            UiIcon::Folder,
                            W::ChooseFolder,
                            Btn::Ghost,
                        ),
                    ),
                );
            }
            if proposal.steps.contains(&Executor::App) {
                plan.push(self.text(if self.studio_lab.config.has_key {"应用创作使用你的 API 及独立额度。本任务资料会提供给该模型；完成后在电脑运行检查，发送手机与同步官网需另行确认。"} else {"使用已连接手机的 Amoo 模型与账号生成应用，电脑负责保存、运行检查和自动修复。完成后可预览，再确认发送手机。"},CAPTION,p.text2));
                if !self.studio_lab.config.has_key {
                    plan.push(self.ws_button(
                        "configure-api",
                        "改用自己的模型 API",
                        UiIcon::Settings,
                        W::Settings,
                        Btn::Secondary,
                    ));
                }
            }
            plan.push(self.between(
                self.ws_button(
                    "adjust",
                    "返回修改",
                    UiIcon::ArrowLeft,
                    W::EditPlan,
                    Btn::Ghost,
                ),
                if self.busy {
                    self.disabled_button("正在读取配置")
                } else {
                    self.ws_button(
                        "confirm",
                        "确认开始",
                        UiIcon::Play,
                        W::Confirm,
                        Btn::Primary,
                    )
                },
            ));
            rows.push(self.panel(plan));
        }
        if let Some(error) = &self.workspace.error {
            rows.push(self.panel(vec![self.hstack(
                S2,
                vec![
                    self.ui_icon(UiIcon::Info, 18., p.warning),
                    Expanded::new(1., self.text(error, BODY, p.text)).finish(),
                ],
            )]));
        }
        self.vstack(S4, rows)
    }
    fn workspace_side(&self, compact: bool) -> El {
        let p = self.p();
        let title = if self.workspace.history {
            "会话记录"
        } else {
            match self.workspace.panel {
                1 => "模型与账号",
                2 => "添加到当前任务",
                3 => "手机能力",
                4 => "Studio 项目",
                5 => "模型服务设置",
                6 => "成果与预览",
                7 => "处理方式",
                _ => "Amoo",
            }
        };
        let header = self.between(
            self.heading(title),
            self.ws_button(
                "close-side",
                "关闭",
                UiIcon::Close,
                W::ClosePanel,
                Btn::Ghost,
            ),
        );
        let body = if self.workspace.history {
            {
                let mut rows = vec![self.input(F_AMOO_SEARCH, "搜索任务…", None)];
                rows.push(self.button_icon(
                    "workspace-sync-history",
                    "同步手机记录",
                    UiIcon::Refresh,
                    Action::Refresh,
                    Btn::Secondary,
                ));
                rows.extend(self.workspace_recent(100));
                self.vstack(S2, rows)
            }
        } else {
            match self.workspace.panel {
                1 => self.workspace_models(),
                2 => self.workspace_resources(),
                3 => self.workspace_capabilities(),
                4 => self.workspace_projects(),
                5 => self.studio_lab_settings(),
                6 => self.workspace_preview(),
                _ => self.workspace_resources(),
            }
        };
        motion::enter(
            Container::new(
                Flex::column()
                    .with_main_axis_size(MainAxisSize::Max)
                    .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                    .with_spacing(S4)
                    .with_child(header)
                    .with_child(
                        Expanded::new(
                            1.,
                            self.pane_scroll(self.workspace.preview_scroll.clone(), body),
                        )
                        .finish(),
                    )
                    .finish(),
            )
            .with_uniform_padding(if compact { S4 } else { S3 })
            .with_background_color(p.sidebar)
            .with_border(Border::left(1.).with_border_color(p.border))
            .finish(),
            self.workspace.panel_entered,
            !self.settings.value.reduced_motion,
        )
    }
    fn workspace_models(&self) -> El {
        let p = self.p();
        self.vstack(
            S4,
            vec![
                self.panel(vec![
                    self.hstack(
                        S2,
                        vec![
                            self.ui_icon(UiIcon::Smartphone, 20., p.text),
                            self.heading("手机 Amoo"),
                        ],
                    ),
                    self.text(
                        "对话、识图、文件处理与图片生成。使用手机账号，由手机自动选择模型。",
                        BODY,
                        p.text2,
                    ),
                    self.button(
                        "workspace-usage",
                        "查看账号与用量",
                        Action::Page(Page::Account),
                        Btn::Ghost,
                    ),
                ]),
                self.panel(vec![
                    self.hstack(
                        S2,
                        vec![
                            self.ui_icon(UiIcon::Cpu, 20., p.text),
                            self.heading("我的 API"),
                        ],
                    ),
                    self.text(
                        if self.studio_lab.config.has_key {
                            format!("{} · 用于应用创作和本机验证", self.studio_lab.config.model)
                        } else {
                            "尚未配置。应用创作需要你自己的模型服务。".into()
                        },
                        BODY,
                        p.text2,
                    ),
                    self.ws_button(
                        "api-settings",
                        "管理模型服务",
                        UiIcon::Settings,
                        W::Settings,
                        Btn::Secondary,
                    ),
                ]),
                self.text(
                    "两种服务的账号与额度独立。每次复杂任务会先说明实际使用哪一项。",
                    CAPTION,
                    p.text3,
                ),
            ],
        )
    }
    fn workspace_resources(&self) -> El {
        let p = self.p();
        let mut rows = vec![
            self.button_icon(
                "workspace-add-local",
                "本机文件",
                UiIcon::Attach,
                Action::AmooAttachFile(false),
                Btn::Secondary,
            ),
            self.button_icon(
                "workspace-add-photo",
                "手机相册",
                UiIcon::Image,
                Action::AmooAttachPhoneAlbum,
                Btn::Secondary,
            ),
            self.ws_button(
                "resource-folder",
                "任务文件夹",
                UiIcon::Folder,
                W::ChooseFolder,
                Btn::Secondary,
            ),
            self.divider(),
            self.ws_button("resource-model", "模型与账号", UiIcon::Cpu, W::Menu(1), Btn::Ghost),
            self.ws_button(
                "resource-capabilities",
                "技能、智能体与 AI 应用",
                UiIcon::Tools,
                W::Menu(3),
                Btn::Ghost,
            ),
            self.ws_button(
                "resource-studio",
                "Studio 官网项目",
                UiIcon::Apps,
                W::Menu(4),
                Btn::Ghost,
            ),
            self.button_icon(
                "workspace-resource-schedules",
                "手机定时任务",
                UiIcon::Calendar,
                Action::AmooToggleSchedules,
                Btn::Ghost,
            ),
            self.button_icon(
                "workspace-resource-gui",
                "让 Agent 操作手机",
                UiIcon::Smartphone,
                Action::AmooUi(AmooUiAction::GuiMode(true)),
                Btn::Ghost,
            ),
        ];
        if let Some(task) = self.workspace_task() {
            if !task.outputs.is_empty() {
                rows.push(self.divider());
                rows.push(self.heading("继续使用已有成果"));
                for (i, o) in task.outputs.iter().enumerate() {
                    rows.push(self.ws_button(
                        &format!("attach-output-{i}"),
                        &o.name,
                        UiIcon::Plus,
                        W::AddOutput(i),
                        Btn::Ghost,
                    ));
                }
            }
        }
        if !self.studio_lab.artifacts.is_empty() {
            rows.push(self.divider());
            rows.push(self.heading("本机应用作品"));
            for a in self.studio_lab.artifacts.iter().take(12) {
                rows.push(self.ws_button(
                    &format!("artifact-{}", a.id),
                    &a.name,
                    UiIcon::Apps,
                    W::SelectLab(a.id.clone()),
                    Btn::Ghost,
                ));
            }
        }
        rows.push(self.text(
            "资料、手机技能与官网项目按需打开，当前任务和草稿会保留。",
            CAPTION,
            p.text3,
        ));
        self.vstack(S3, rows)
    }
    fn workspace_capabilities(&self) -> El {
        let tabs = self.segmented(
            "workspace-capabilities",
            [("模型", 1), ("技能", 2), ("智能体", 3), ("AI 应用", 4)]
                .iter()
                .map(|(name, tab)| {
                    (
                        (*name).into(),
                        self.work.tab == *tab,
                        Action::Work(WorkAction::Tab(*tab)),
                    )
                })
                .collect(),
        );
        self.vstack(S3, vec![tabs, self.work_catalog_pane()])
    }
    fn workspace_projects(&self) -> El {
        let p = self.p();
        let mut rows = vec![
            self.text(
                "连接手机上的 Studio 官网账号，自动读取项目、技能和智能体。",
                CAPTION,
                p.text2,
            ),
            self.wrap(vec![
                self.button_icon(
                    "workspace-studio-refresh",
                    "同步官网",
                    UiIcon::Refresh,
                    Action::Studio(StudioAction::CloudRefresh),
                    Btn::Secondary,
                ),
                self.button(
                    "workspace-studio-login",
                    "连接官网账号",
                    Action::Studio(StudioAction::WebsiteLogin),
                    Btn::Ghost,
                ),
            ]),
        ];
        if let Some(web) = self.studio.web.as_ref().filter(|w| w.authorized) {
            rows.push(self.line(
                format!("已连接 · {} 个项目", web.items.len()),
                CAPTION,
                p.success,
            ));
            for (i, item) in web.items.iter().enumerate().take(60) {
                rows.push(self.panel(vec![
                    self.line_w(&item.name, BODY, p.text, Weight::Medium),
                    self.text(
                        format!("{} · {}", item.category, item.description),
                        CAPTION,
                        p.text2,
                    ),
                    self.ws_button(
                        &format!("open-project-{i}"),
                        "继续编辑",
                        UiIcon::ExternalLink,
                        W::OpenProject(i),
                        Btn::Secondary,
                    ),
                ]));
            }
        } else {
            rows.push(
                self.text(
                    self.studio
                        .web_error
                        .as_deref()
                        .unwrap_or("首次连接需要在手机官网完成登录，之后使用已保存的登录会话。"),
                    CAPTION,
                    p.text2,
                ),
            );
        }
        self.vstack(S3, rows)
    }
    fn workspace_preview(&self) -> El {
        let p = self.p();
        let Some(output) = self
            .workspace_task()
            .and_then(|t| t.outputs.get(self.workspace.selected_output?))
        else {
            return self.empty("选择一个成果", "文件与应用会在这里预览。", None);
        };
        let mut rows = vec![self.heading(&output.name)];
        if let Some(id) = &output.lab_id {
            if let Some(a) = self.studio_lab.artifacts.iter().find(|a| &a.id == id) {
                rows.push(self.pill_badge(
                    if a.passed {
                        "本机检查通过"
                    } else {
                        "需要修复"
                    },
                    if a.passed { p.success } else { p.warning },
                    p.surface,
                ));
                if let Some(path) = &a.screenshot_path {
                    rows.push(
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
                        .with_height(260.)
                        .finish(),
                    );
                }
                rows.push(self.button_icon(
                    "workspace-app-interact",
                    "交互预览",
                    UiIcon::Monitor,
                    Action::StudioLab(StudioLabAction::Preview),
                    Btn::Secondary,
                ));
                rows.push(self.text(&a.summary, CAPTION, p.text2));
                if let Some((report_id, report)) = &self.studio_lab.report {
                    if report_id == id {
                        if let Some(checks) = report["checks"].as_array() {
                            for check in checks.iter().take(12) {
                                let pass = check["passed"].as_bool().unwrap_or(false);
                                rows.push(self.hstack(
                                    S2,
                                    vec![
                                            self.ui_icon(
                                                if pass { UiIcon::Check } else { UiIcon::Info },
                                                14.,
                                                if pass { p.success } else { p.warning },
                                            ),
                                            Expanded::new(
                                                1.,
                                                self.text(
                                                    check["label"].as_str().unwrap_or("检查项"),
                                                    CAPTION,
                                                    p.text2,
                                                ),
                                            )
                                            .finish(),
                                        ],
                                ));
                            }
                        }
                    }
                }
                rows.push(self.button(
                    "workspace-app-folder",
                    "打开作品文件夹",
                    Action::StudioLab(StudioLabAction::OpenFolder),
                    Btn::Ghost,
                ));
                rows.push(self.text(
                    "电脑已保存；发到手机后可在伴侣作品库离线使用。",
                    CAPTION,
                    p.text3,
                ));
                if a.passed {
                    rows.push(self.ws_button(
                        "phone-delivery",
                        "发送到手机",
                        UiIcon::Smartphone,
                        W::Deliver(false),
                        Btn::Primary,
                    ));
                    rows.push(self.ws_button(
                        "cloud-delivery",
                        "同步到 Studio",
                        UiIcon::Upload,
                        W::Deliver(true),
                        Btn::Secondary,
                    ));
                } else {
                    rows.push(self.ws_button(
                        "fix-app",
                        "修复这一项",
                        UiIcon::Retry,
                        W::Retry,
                        Btn::Secondary,
                    ));
                }
                if let Some(note) = self.studio_lab_phone_note() {
                    rows.push(match note {
                        Ok(n) => self.text(n, CAPTION, p.success),
                        Err(n) => self.text(n, CAPTION, p.danger),
                    });
                }
                if let Some(feedback) = self.studio_lab_cloud_feedback() {
                    if let Some(note) = &feedback.note {
                        rows.push(self.text(note, CAPTION, p.success));
                    }
                    if let Some(error) = &feedback.error {
                        rows.push(self.text(error, CAPTION, p.danger));
                    }
                    if feedback.receipt.is_some() || feedback.recovery_url.is_some() {
                        rows.push(self.button(
                            "workspace-open-cloud",
                            "继续编辑官网项目",
                            Action::StudioLab(StudioLabAction::OpenCloud),
                            Btn::Secondary,
                        ));
                    }
                }
            } else {
                rows.push(self.text("正在读取作品记录…", CAPTION, p.text3));
            }
        } else {
            if let Some(preview) = self
                .work
                .local_preview
                .as_ref()
                .filter(|v| v.path == output.path)
            {
                if preview.image {
                    rows.push(
                        ConstrainedBox::new(
                            Image::new(
                                AssetSource::LocalFile {
                                    path: preview.path.to_string_lossy().into_owned(),
                                },
                                CacheOption::BySize,
                            )
                            .contain()
                            .finish(),
                        )
                        .with_height(260.)
                        .finish(),
                    );
                }
                if let Some(text) = &preview.text {
                    rows.push(self.work_rich_text(
                        &text.chars().take(6000).collect::<String>(),
                        "workspace-preview-text",
                    ));
                }
                rows.push(self.text(&preview.note, CAPTION, p.text3));
                rows.push(self.wrap(vec![
                    self.button_icon(
                        "workspace-open-file",
                        "打开原文件",
                        UiIcon::ExternalLink,
                        Action::Work(WorkAction::OpenLocalPreview),
                        Btn::Secondary,
                    ),
                    self.button_icon(
                        "workspace-export-file",
                        "另存为",
                        UiIcon::Download,
                        Action::Work(WorkAction::ExportLocalPreview),
                        Btn::Secondary,
                    ),
                ]));
                rows.push(self.ws_button(
                    "file-phone",
                    "发送到手机",
                    UiIcon::Smartphone,
                    W::Deliver(false),
                    Btn::Primary,
                ));
            } else {
                rows.push(
                    self.text(
                        self.work
                            .local_preview_error
                            .as_deref()
                            .unwrap_or("正在读取文件预览…"),
                        CAPTION,
                        p.text2,
                    ),
                );
            }
        }
        if let Some(task) = self.workspace_task() {
            for receipt in task
                .receipts
                .iter()
                .rev()
                .filter(|r| r.path == output.path)
                .take(3)
            {
                rows.push(self.text(
                    &receipt.note,
                    CAPTION,
                    if receipt.success {
                        p.success
                    } else {
                        p.warning
                    },
                ));
            }
        }
        if let Some(error) = &self.studio_lab.error {
            if output.lab_id.is_some() {
                rows.push(self.text(error, CAPTION, p.danger));
            }
        }
        if let Some(cloud) = self.workspace.delivery {
            rows.push(self.divider());
            rows.push(self.heading(if cloud {
                "确认创建官网草稿"
            } else {
                "确认发送到手机"
            }));
            rows.push(self.text(
                if cloud {
                    "只创建 Studio 草稿，不公开发布。使用官网所选模型和官网账号额度。".into()
                } else {
                    format!(
                        "目标：{}\n保存到：{}",
                        self.selected
                            .as_ref()
                            .map(|d| d.model.as_str())
                            .unwrap_or("未连接手机"),
                        if output.lab_id.is_some() {
                            "StepX伴侣 → 作品"
                        } else {
                            "StepX伴侣 → 文件 → 接收文件"
                        }
                    )
                },
                BODY,
                p.text2,
            ));
            if cloud {
                rows.push(self.button(
                    "workspace-get-cloud-models",
                    "读取官网模型",
                    Action::StudioLab(StudioLabAction::CloudModels),
                    Btn::Secondary,
                ));
                for m in &self.studio_lab.cloud_models {
                    rows.push(self.button(
                        format!("workspace-cloud-model-{}", m.id),
                        format!(
                            "{} {}",
                            if self.studio_lab.cloud_model.as_ref() == Some(&m.id) {
                                "✓"
                            } else {
                                ""
                            },
                            m.name
                        ),
                        Action::StudioLab(StudioLabAction::CloudModel(m.id.clone())),
                        Btn::Ghost,
                    ));
                }
            } else if output.lab_id.is_some() {
                rows.push(self.ws_button(
                    "open-after",
                    if self.workspace.open_phone {
                        "已勾选：发送后在手机打开"
                    } else {
                        "发送后在手机打开"
                    },
                    if self.workspace.open_phone {
                        UiIcon::Check
                    } else {
                        UiIcon::Smartphone
                    },
                    W::ToggleOpenPhone,
                    Btn::Ghost,
                ));
            }
            rows.push(self.ws_button(
                "cancel-delivery",
                "取消",
                UiIcon::Close,
                W::CancelDelivery,
                Btn::Ghost,
            ));
            rows.push(if self.busy || self.selected.is_none() {
                self.disabled_button(if self.busy {
                    "正在处理"
                } else {
                    "请先连接手机"
                })
            } else {
                self.ws_button(
                    "confirm-delivery",
                    if cloud {
                        "确认同步"
                    } else {
                        "确认发送"
                    },
                    UiIcon::Check,
                    W::ConfirmDelivery,
                    Btn::Primary,
                )
            });
        }
        if let Some(e) = &self.workspace.error {
            rows.push(self.text(e, CAPTION, p.danger));
        }
        self.vstack(S3, rows)
    }
}

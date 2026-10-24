use super::super::{
    icons::UiIcon,
    studio_lab_state::{StudioLabAction as Lab, LAB_TEMPLATES},
    studio_state::StudioAction,
    theme::*,
    widgets::*,
    Action, Companion, F_LAB_ENDPOINT, F_LAB_KEY, F_LAB_MODEL, F_LAB_PROMPT,
};
use facloud_ui::{assets::asset_cache::AssetSource, elements::*, fonts::Weight, platform::Cursor};

impl Companion {
    fn lab_button(&self, id: &str, title: &str, action: Lab, kind: Btn) -> El {
        let icon = match &action {
            Lab::Start => Some(UiIcon::Play),
            Lab::Stop => Some(UiIcon::Close),
            Lab::Fresh => Some(UiIcon::Plus),
            Lab::Refresh | Lab::Models | Lab::CloudModels => Some(UiIcon::Refresh),
            Lab::Preview => Some(UiIcon::Monitor),
            Lab::OpenFolder => Some(UiIcon::Folder),
            Lab::DeliverPhone => Some(UiIcon::Smartphone),
            Lab::SyncCloud => Some(UiIcon::Upload),
            Lab::OpenCloud => Some(UiIcon::ExternalLink),
            Lab::Iterate => Some(UiIcon::Retry),
            _ => None,
        };
        if !self.studio_lab_action_allowed(&action) {
            self.disabled_button(title)
        } else if let Some(icon) = icon {
            self.button_icon(
                format!("studio-lab-{id}"),
                title,
                icon,
                Action::StudioLab(action),
                kind,
            )
        } else {
            self.button(
                format!("studio-lab-{id}"),
                title,
                Action::StudioLab(action),
                kind,
            )
        }
    }
    fn lab_prompt(&self) -> El {
        let p = self.p();
        let field = &self.fields[F_LAB_PROMPT];
        let focused = self.focus == Some(F_LAB_PROMPT);
        let mut value = field.text.clone();
        let mut caret = 0;
        if focused {
            let cursor = field.cursor.min(value.len());
            caret = cursor + field.preedit.len();
            value.insert_str(cursor, &format!("{}│", field.preedit));
        }
        let empty = field.text.is_empty() && field.preedit.is_empty();
        if value.is_empty() {
            value = if self.studio_lab.source.is_some() {
                "描述要继续改进的交互、功能或视觉效果…"
            } else {
                "想做一个什么应用？写下用途、关键交互和你希望如何验证…"
            }
            .into();
        }
        Hoverable::new(self.mouse_state("studio-lab-prompt"), move |_| {
            ConstrainedBox::new(
                Container::new(
                    self.pane_scroll(
                        self.studio_lab.prompt_scroll.clone(),
                        Text::new(value, self.font, BODY)
                            .with_saved_char_position(caret, format!("input-caret:{F_LAB_PROMPT}"))
                            .with_color(if empty && !focused { p.text3 } else { p.text })
                            .with_line_height_ratio(1.4)
                            .finish(),
                    ),
                )
                .with_uniform_padding(S3)
                .with_background_color(p.surface)
                .with_border(Border::all(1.).with_border_color(if focused {
                    p.accent
                } else {
                    p.border
                }))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_CONTROL)))
                .finish(),
            )
            .with_height(124.)
            .finish()
        })
        .with_cursor(Cursor::IBeam)
        .on_mouse_down(|ctx, _, _| ctx.dispatch_typed_action(Action::Focus(F_LAB_PROMPT)))
        .finish()
    }
    pub(in super::super) fn studio_lab_page(&self) -> El {
        let p = self.p();
        let running = self.studio_lab.handle.is_some();
        let configured = self.studio_lab.config.has_key && !self.studio_lab.config.model.is_empty();
        let mut left = vec![self.panel(vec![
            self.between(
                self.heading("把想法做成手机应用"),
                self.pill_badge("本地工作台", p.text2, p.surface),
            ),
            self.text(
                "用自己的模型创作离线网页应用与小游戏，实际运行、检查与修复，再发到手机。",
                BODY,
                p.text2,
            ),
            self.wrap(
                LAB_TEMPLATES
                    .iter()
                    .enumerate()
                    .map(|(i, (title, _))| {
                        self.lab_button(
                            &format!("template-{i}"),
                            title,
                            Lab::Template(i),
                            Btn::Secondary,
                        )
                    })
                    .collect(),
            ),
        ])];
        let mut composer = vec![self.between(
            self.heading(if self.studio_lab.source.is_some() {
                "继续改进"
            } else {
                "创作需求"
            }),
            self.lab_button("fresh", "新任务", Lab::Fresh, Btn::Ghost),
        )];
        if let Some(name) = &self.studio_lab.source_name {
            composer.push(self.text(format!("基于：{name}"), CAPTION, p.text2));
        }
        composer.push(self.lab_prompt());
        composer.push(self.between(
            self.wrap(vec![
                self.ui_icon(UiIcon::Bolt, 16., p.text3),
                self.line(
                    if configured {
                        self.studio_lab.config.model.as_str()
                    } else {
                        "尚未配置创作模型"
                    },
                    CAPTION,
                    p.text2,
                ),
                self.button(
                    "lab-config",
                    "模型设置",
                    Action::Studio(StudioAction::Tab(3)),
                    Btn::Ghost,
                ),
            ]),
            if running {
                self.lab_button("stop", "停止", Lab::Stop, Btn::Secondary)
            } else if self.fields[F_LAB_PROMPT].text.trim().is_empty() {
                self.disabled_button("开始创作")
            } else {
                self.lab_button("start", "开始创作", Lab::Start, Btn::Primary)
            },
        ));
        composer.push(self.text(
            "Enter 开始 · Shift+Enter 换行 · 自动修复最多 3 轮",
            MICRO,
            p.text3,
        ));
        left.push(self.panel(composer));
        if let Some(error) = &self.studio_lab.error {
            left.push(self.panel(vec![self.text(error, BODY, p.danger)]));
        }
        if running || !self.studio_lab.status.is_empty() {
            let mut progress = vec![
                self.between(
                    self.heading(if running {
                        "正在创作与验证"
                    } else {
                        "本次进展"
                    }),
                    self.pill_badge(
                        if running {
                            "运行中"
                        } else if self.studio_lab.error.is_some() {
                            "待处理"
                        } else {
                            "已结束"
                        },
                        if running { p.accent } else { p.text2 },
                        p.surface,
                    ),
                ),
                self.text(&self.studio_lab.status, BODY, p.text),
            ];
            for event in self
                .studio_lab
                .events
                .iter()
                .rev()
                .take(4)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
            {
                progress.push(self.text(event, CAPTION, p.text3));
            }
            left.push(self.panel(progress));
        }
        let mut history = vec![self.between(
            self.heading("本机作品"),
            self.lab_button("refresh", "刷新", Lab::Refresh, Btn::Ghost),
        )];
        if self.studio_lab.artifacts.is_empty() {
            history.push(self.text(
                "生成的版本与真实检查结果保留在这里，关闭程序后仍能继续。",
                CAPTION,
                p.text3,
            ));
        }
        let page = self
            .studio_lab
            .history_page
            .min(self.studio_lab.artifacts.len().saturating_sub(1) / 6);
        for (i, a) in self
            .studio_lab
            .artifacts
            .iter()
            .enumerate()
            .skip(page * 6)
            .take(6)
        {
            history.push(self.list_row(
                format!("lab-artifact-{i}"),
                self.between(
                    self.vstack(
                        S1,
                        vec![
                            self.line_w(&a.name, BODY, p.text, Weight::Medium),
                            self.line(format!("第 {} 轮 · {}", a.round, a.summary), MICRO, p.text3),
                        ],
                    ),
                    self.pill_badge(
                        if a.passed {
                            "检查通过"
                        } else {
                            "需改进"
                        },
                        if a.passed { p.success } else { p.text3 },
                        p.surface,
                    ),
                ),
                self.studio_lab.selected == Some(i),
                Some(Action::StudioLab(Lab::Select(i))),
                None,
            ));
        }
        let pages = self.studio_lab.artifacts.len().max(1).div_ceil(6);
        if pages > 1 {
            history.push(self.between(
                if page > 0 {
                    self.lab_button(
                        "history-prev",
                        "上一页",
                        Lab::HistoryPage(page - 1),
                        Btn::Ghost,
                    )
                } else {
                    self.disabled_button("上一页")
                },
                self.hstack(
                    S2,
                    vec![
                        self.caption(format!("{} / {}", page + 1, pages)),
                        if page + 1 < pages {
                            self.lab_button(
                                "history-next",
                                "下一页",
                                Lab::HistoryPage(page + 1),
                                Btn::Ghost,
                            )
                        } else {
                            self.disabled_button("下一页")
                        },
                    ],
                ),
            ));
        }
        left.push(self.panel(history));
        Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Start)
            .with_spacing(S4)
            .with_child(Expanded::new(1.15, self.vstack(S4, left)).finish())
            .with_child(Expanded::new(1., self.studio_lab_preview()).finish())
            .finish()
    }
    fn studio_lab_preview(&self) -> El {
        let p = self.p();
        let Some(a) = self.studio_lab_selected() else {
            return self.panel(vec![
                self.ui_icon(UiIcon::Monitor, 36., p.text3),
                self.heading("成果与预览"),
                self.text(
                    "创作完成后，在这里查看真实浏览器截图、逐项验收结果和上机入口。",
                    BODY,
                    p.text2,
                ),
                self.caption("生成 → 运行 → 检查 → 修复 → 交付"),
            ]);
        };
        let mut rows = vec![self.between(
            self.heading(&a.name),
            self.pill_badge(
                if a.passed { "可交付" } else { "待验证" },
                if a.passed { p.success } else { p.text3 },
                p.surface,
            ),
        )];
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
                .with_height(200.)
                .finish(),
            );
        } else {
            rows.push(self.empty("预览尚未生成", "先查看验收结果并继续改进。", None));
        }
        rows.push(self.wrap(vec![
            self.lab_button("preview", "打开交互预览", Lab::Preview, Btn::Secondary),
            self.lab_button("iterate", "继续改进", Lab::Iterate, Btn::Ghost),
            self.lab_button("folder", "作品目录", Lab::OpenFolder, Btn::Ghost),
        ]));
        rows.push(self.divider());
        rows.push(self.between(
            self.heading("实际检查"),
            self.lab_button(
                "report",
                if self.studio_lab.report_open {
                    "收起"
                } else {
                    "全部结果"
                },
                Lab::ToggleReport,
                Btn::Ghost,
            ),
        ));
        if let Some((_, report)) = self
            .studio_lab
            .report
            .as_ref()
            .filter(|(id, _)| id == &a.id)
        {
            if let Some(checks) = report["checks"].as_array() {
                let passed = checks.iter().filter(|c| c["passed"] == true).count();
                rows.push(self.text(
                    format!("{passed} / {} 项通过 · 浏览器实际执行", checks.len()),
                    CAPTION,
                    if a.passed { p.success } else { p.text2 },
                ));
                for check in checks
                    .iter()
                    .filter(|c| self.studio_lab.report_open || c["passed"] != true)
                    .take(if self.studio_lab.report_open { 32 } else { 2 })
                {
                    let pass = check["passed"] == true;
                    let label = check["label"].as_str().unwrap_or("检查项");
                    rows.push(self.lead(
                        S2,
                        self.ui_icon(
                            if pass { UiIcon::Check } else { UiIcon::Close },
                            14.,
                            if pass { p.success } else { p.danger },
                        ),
                        Expanded::new(1., self.text(label, CAPTION, p.text2)).finish(),
                    ));
                    if !pass {
                        if let Some(detail) = check["detail"].as_str().filter(|s| !s.is_empty()) {
                            rows.push(self.text(detail, CAPTION, p.danger));
                        }
                    }
                }
            }
        } else if let Some(error) = &self.studio_lab.report_error {
            rows.push(self.text(error, CAPTION, p.danger));
        } else {
            rows.push(self.caption("正在读取检查报告…"));
        }
        rows.push(self.text(
            "自动检查覆盖页面、手机布局与所列交互；上机后还可继续体验。",
            MICRO,
            p.text3,
        ));
        let mut all = vec![self.panel(rows)];
        let mut delivery = vec![
            self.heading("发到手机"),
            self.text(
                "保存到 StepX伴侣作品库，脱离电脑也可离线打开。",
                CAPTION,
                p.text2,
            ),
            if a.passed && self.selected.is_some() {
                self.lab_button(
                    "phone",
                    "发送并打开手机作品",
                    Lab::DeliverPhone,
                    Btn::Secondary,
                )
            } else {
                self.disabled_button(if self.selected.is_none() {
                    "请先连接手机"
                } else {
                    "通过检查后可发送"
                })
            },
        ];
        if let Some(note) = self.studio_lab_phone_note() {
            delivery.push(match note {
                Ok(note) => self.text(note, CAPTION, p.success),
                Err(error) => self.text(error, CAPTION, p.danger),
            });
        }
        all.push(self.panel(delivery));
        let web = self.studio.web.as_ref().filter(|w| w.authorized);
        let current = web.is_some_and(|w| self.studio_lab.cloud_scope.as_ref() == Some(&w.scope));
        let mut cloud = vec![
            self.between(
                self.heading("接着在 Studio 创作"),
                self.lab_button("cloud-models", "读取官网模型", Lab::CloudModels, Btn::Ghost),
            ),
            self.text(
                "把通过检查的源码新建为官网项目。下方模型仅供官网继续创作，使用官网账号额度。",
                CAPTION,
                p.text2,
            ),
        ];
        if current && !self.studio_lab.cloud_models.is_empty() {
            cloud.push(
                self.wrap(
                    self.studio_lab
                        .cloud_models
                        .iter()
                        .map(|m| {
                            self.lab_button(
                                &format!("cloud-model-{}", m.id),
                                &m.name,
                                Lab::CloudModel(m.id.clone()),
                                if self.studio_lab.cloud_model.as_ref() == Some(&m.id) {
                                    Btn::Primary
                                } else {
                                    Btn::Secondary
                                },
                            )
                        })
                        .collect(),
                ),
            );
        } else {
            cloud.push(self.caption(if web.is_some() {
                "先读取官网可用模型，再创建项目。"
            } else {
                "请在连接设置中完成官网连接。"
            }));
        }
        cloud.push(
            if self
                .studio_lab_cloud_feedback()
                .is_some_and(|f| f.receipt.is_some())
            {
                self.lab_button(
                    "cloud-open",
                    "继续编辑官网项目",
                    Lab::OpenCloud,
                    Btn::Primary,
                )
            } else if self
                .studio_lab_cloud_feedback()
                .is_some_and(|f| f.recovery_url.is_some())
            {
                self.lab_button(
                    "cloud-recover",
                    "打开已创建的项目",
                    Lab::OpenCloud,
                    Btn::Secondary,
                )
            } else if a.passed && current && self.studio_lab.cloud_model.is_some() {
                self.lab_button(
                    "cloud-sync",
                    "新建官网项目并同步",
                    Lab::SyncCloud,
                    Btn::Secondary,
                )
            } else {
                self.disabled_button("新建官网项目并同步")
            },
        );
        if let Some(feedback) = self.studio_lab_cloud_feedback() {
            if let Some(note) = &feedback.note {
                cloud.push(self.text(note, CAPTION, p.success));
            }
            if let Some(error) = &feedback.error {
                cloud.push(self.text(error, CAPTION, p.danger));
            }
        }
        all.push(self.panel(cloud));
        self.vstack(S4, all)
    }
    pub(in super::super) fn studio_lab_settings(&self) -> El {
        let p = self.p();
        let mut rows = vec![
            self.between(
                self.heading("本地创作模型"),
                self.pill_badge(
                    if self.studio_lab.config.has_key {
                        "已保存密钥"
                    } else {
                        "未配置"
                    },
                    p.text2,
                    p.surface,
                ),
            ),
            self.text(
                "本地生成与自动修复使用你自己的模型 API 和独立额度。官网模型仅用于官网项目，不会作为本机生成接口。",
                BODY,
                p.text2,
            ),
            self.caption("API Base URL"),
            self.input(F_LAB_ENDPOINT, "https://你的服务地址/v1", None),
            self.segmented(
                "lab-protocol",
                [("OpenAI 兼容", 0), ("Anthropic", 1)]
                    .iter()
                    .map(|(name, i)| {
                        (
                            (*name).into(),
                            self.studio_lab.protocol == *i,
                            Action::StudioLab(Lab::Protocol(*i)),
                        )
                    })
                    .collect(),
            ),
            self.caption("API Key"),
            self.input(
                F_LAB_KEY,
                if self.studio_lab.config.has_key {
                    "已安全保存；留空保留，粘贴新密钥可替换"
                } else {
                    "粘贴密钥，仅保存到本机"
                },
                None,
            ),
            self.caption("模型名称"),
            self.input(F_LAB_MODEL, "填写模型 ID，或读取下方模型目录", None),
            self.wrap(vec![
                self.lab_button("config-save", "保存设置", Lab::Save, Btn::Primary),
                self.lab_button("config-test", "测试实际调用", Lab::Test, Btn::Secondary),
                self.lab_button("config-models", "读取模型", Lab::Models, Btn::Secondary),
                self.lab_button("config-clear", "清除密钥", Lab::ClearKey, Btn::Ghost),
            ]),
        ];
        if self.studio_lab_config_dirty() {
            rows.push(self.text("有尚未保存的模型设置", CAPTION, p.text3));
        }
        if let Some(note) = &self.studio_lab.model_note {
            rows.push(self.text(note, CAPTION, p.success));
        }
        if let Some(error) = &self.studio_lab.error {
            rows.push(self.text(error, CAPTION, p.danger));
        }
        let query = self.fields[F_LAB_MODEL].text.trim().to_lowercase();
        let query = if self
            .studio_lab
            .models
            .iter()
            .any(|m| m.to_lowercase() == query)
        {
            String::new()
        } else {
            query
        };
        let filtered: Vec<_> = self
            .studio_lab
            .models
            .iter()
            .filter(|m| query.is_empty() || m.to_lowercase().contains(&query))
            .take(12)
            .collect();
        if !filtered.is_empty() {
            rows.push(self.divider());
            rows.push(self.caption("模型目录 · 输入模型名筛选，选择后保存"));
            for m in filtered {
                rows.push(self.list_row(
                    format!("lab-model-{m}"),
                    self.line(m, CAPTION, p.text),
                    self.fields[F_LAB_MODEL].text == *m,
                    Some(Action::StudioLab(Lab::Model(m.clone()))),
                    None,
                ));
            }
        }
        self.panel(rows)
    }
}

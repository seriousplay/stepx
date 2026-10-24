use super::super::{
    expansion_state::ExpansionAction as EA, icons::UiIcon, theme::*, widgets::*, *,
};
use facloud_ui::{elements::*, fonts::Weight};
use serde_json::Value;
fn label(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or("").into()
}
fn state(v: &Value) -> &'static str {
    match v["state"].as_str().unwrap_or("") {
        "archived" => "已迁移",
        "restored" => "已恢复",
        "failed" => "未完成",
        "verified" => "等待手机确认",
        _ => "待迁移",
    }
}
impl Companion {
    fn ex_button(&self, id: impl Into<String>, text: &str, a: EA, style: Btn) -> El {
        self.button(id, text, Action::Expansion(a), style)
    }
    pub(in super::super) fn expansion_card(&self) -> El {
        let p = self.p();
        let s = &self.expansion.snapshot;
        let bytes = s.status["releasedBytes"].as_u64().unwrap_or_else(|| {
            s.files
                .iter()
                .filter(|j| j["state"] == "archived")
                .map(|j| j["size"].as_u64().unwrap_or(0))
                .sum()
        });
        let count = s.status["count"]
            .as_u64()
            .unwrap_or_else(|| s.files.iter().filter(|j| j["state"] == "archived").count() as u64);
        let headline = self.between(
            self.hstack(
                S2,
                vec![
                    self.ui_icon(UiIcon::Storage, 20., p.text),
                    self.heading("存储扩容"),
                ],
            ),
            if self.page == Page::Expansion {
                self.line("照片 · 视频 · 文档", CAPTION, p.text3)
            } else {
                self.button(
                    "expansion-manage",
                    "管理扩容",
                    Action::Page(Page::Expansion),
                    Btn::Secondary,
                )
            },
        );
        let metric = self.between(
            self.hstack(
                S4,
                vec![
                    self.line_w(
                        storage::format_bytes(bytes),
                        METRIC,
                        p.text,
                        Weight::Semibold,
                    ),
                    self.line(
                        format!(
                            "{} · {count} 个文件保存在电脑",
                            if s.status["releasedBytes"].is_u64() {
                                "已释放"
                            } else {
                                "原件已归档"
                            }
                        ),
                        CAPTION,
                        p.text2,
                    ),
                ],
            ),
            self.pill_badge(
                if s.status["policy"]["enabled"] == true {
                    "自动迁移已开启"
                } else {
                    if s.status["policy"].is_null() {
                        "尚未读取规则"
                    } else if s.status["policy"]["desktopId"]
                        .as_str()
                        .unwrap_or("")
                        .is_empty()
                    {
                        "尚未设置"
                    } else {
                        "自动迁移已暂停"
                    }
                },
                p.text2,
                p.hover,
            ),
        );
        let phase = if s.active.is_some() {
            s.phase.clone()
        } else if !s.online {
            "连接手机后自动迁移，手机保留文件入口".into()
        } else if s.error {
            s.phase.clone()
        } else {
            if s.phase.is_empty() {
                "正在检查迁移条件…".into()
            } else {
                s.phase.clone()
            }
        };
        let mut children = vec![
            headline,
            metric,
            self.text(phase, CAPTION, if s.error { p.danger } else { p.text2 }),
        ];
        if s.active.is_some() {
            let step = if s.phase.contains("传输") {
                0
            } else if s.phase.contains("校验电脑") {
                1
            } else if s.phase.contains("入口") {
                2
            } else if s.phase.contains("完成") {
                3
            } else {
                0
            };
            children.push(
                self.hstack(
                    S2,
                    ["1 传输", "2 校验", "3 保留入口", "4 释放空间"]
                        .into_iter()
                        .enumerate()
                        .map(|(i, t)| {
                            self.pill_badge(
                                t,
                                if i == step { p.success } else { p.text3 },
                                if i == step { p.success_soft } else { p.hover },
                            )
                        })
                        .collect(),
                ),
            );
        }
        if let Some(j) = s.active.as_ref() {
            children.push(self.text(label(j, "name"), CAPTION, p.text));
        } else {
            let mut recent = s.files.iter().collect::<Vec<_>>();
            recent.sort_by_key(|j| std::cmp::Reverse(j["archivedAt"].as_u64().unwrap_or(0)));
            if let Some(j) = recent.first() {
                children.push(self.between(
                    self.line(format!("最近迁移 · {}", label(j, "name")), CAPTION, p.text2),
                    self.line(state(j), CAPTION, p.text3),
                ));
            }
        }
        self.panel(children)
    }
    pub(in super::super) fn expansion_actions(&self) -> Vec<El> {
        if self.expansion.settings {
            return vec![];
        }
        vec![
            self.ex_button("exp-refresh", "检查迁移", EA::Refresh, Btn::Secondary),
            self.ex_button("exp-settings", "设置自动迁移", EA::Settings, Btn::Primary),
        ]
    }
    fn expansion_report_card(&self, report: &Value, preview: bool) -> El {
        let p = self.p();
        let e = &self.expansion;
        let title = if preview {
            "按这些规则，会迁移什么？"
        } else {
            "文件检查"
        };
        if !report.is_object() {
            let error = if preview {
                e.preview_error.as_str()
            } else {
                e.snapshot.report_error.as_str()
            };
            return self.panel(vec![
                self.heading(title),
                self.text(
                    if error.is_empty() {
                        "正在检查手机文件，只读取清单，不会移动或清理文件。"
                    } else {
                        error
                    },
                    BODY,
                    p.text2,
                ),
                self.ex_button(
                    if preview {
                        "exp-preview-retry"
                    } else {
                        "exp-check-again"
                    },
                    "重新检查",
                    if preview { EA::Inspect } else { EA::Refresh },
                    Btn::Secondary,
                ),
            ]);
        }
        let metrics = [
            ("可以迁移", report["candidateCount"].as_u64().unwrap_or(0)),
            (
                "保留在手机",
                report["retainedCount"].as_u64().unwrap_or(0)
                    + report["writingCount"].as_u64().unwrap_or(0),
            ),
            (
                "已处理 / 跳过",
                report["excludedCount"].as_u64().unwrap_or(0)
                    + report["invalidCount"].as_u64().unwrap_or(0),
            ),
        ];
        let mut rows = vec![
            self.between(
                self.heading(title),
                self.ex_button(
                    if preview {
                        "exp-preview-refresh"
                    } else {
                        "exp-report-refresh"
                    },
                    "重新检查",
                    if preview { EA::Inspect } else { EA::Refresh },
                    Btn::Ghost,
                ),
            ),
            self.hstack(
                S4,
                metrics
                    .into_iter()
                    .map(|(name, n)| {
                        Expanded::new(
                            1.,
                            self.vstack(
                                S1,
                                vec![
                                    self.line_w(
                                        format!("{n} 个"),
                                        METRIC,
                                        p.text,
                                        Weight::Semibold,
                                    ),
                                    self.line(name, CAPTION, p.text2),
                                ],
                            ),
                        )
                        .finish()
                    })
                    .collect(),
            ),
            self.text(
                format!(
                    "识别到：{} 张照片 · {} 个视频 · {} 份文档",
                    report["counts"]["photos"],
                    report["counts"]["videos"],
                    report["counts"]["documents"]
                ),
                CAPTION,
                p.text2,
            ),
        ];
        if report["candidateCount"].as_u64().unwrap_or(0) > 0 {
            rows.push(self.text(
                format!(
                    "预计迁移 {}。{}",
                    storage::format_bytes(report["candidateBytes"].as_u64().unwrap_or(0)),
                    if preview {
                        "确认后开始；执行前逐个复核。"
                    } else {
                        "按当前已确认的规则处理。"
                    }
                ),
                BODY,
                p.text,
            ));
        } else if let Some(at) = report["nextEligibleAt"]
            .as_i64()
            .filter(|n| *n > 0)
            .and_then(chrono::DateTime::from_timestamp_millis)
        {
            rows.push(self.text(
                format!(
                    "最早一批将在 {} 达到迁移时间，届时保持手机和本客户端连接即可。",
                    at.with_timezone(&chrono::Local).format("%m 月 %d 日 %H:%M")
                ),
                BODY,
                p.text,
            ));
        } else {
            rows.push(self.text(
                "目前没有符合这些规则的文件。可以调整文件类型或保留时间后重新检查。",
                BODY,
                p.text2,
            ));
        }
        if preview && report["retainedCount"].as_u64().unwrap_or(0) > 0 {
            rows.push(self.text(
                "想迁移最近拍摄的文件？返回第一步，将时间改为“写入完成后迁移”，再检查并确认。",
                CAPTION,
                p.text2,
            ));
        }
        if report["partial"] == true {
            rows.push(self.text("文件较多，本次清单统计不完整。", CAPTION, p.warning));
        }
        if let Some(at) = report["checkedAt"]
            .as_i64()
            .and_then(chrono::DateTime::from_timestamp_millis)
        {
            rows.push(self.text(
                format!(
                    "检查于 {} · 照片 / 视频：DCIM 根目录与 Camera；文档：Documents / Download",
                    at.with_timezone(&chrono::Local).format("%H:%M:%S")
                ),
                CAPTION,
                p.text3,
            ));
        }
        self.panel(rows)
    }
    pub(in super::super) fn expansion_page(&self) -> El {
        let p = self.p();
        let e = &self.expansion;
        let q = e.draft.as_ref().unwrap_or(&e.snapshot.status["policy"]);
        let mut sections = vec![];
        if e.settings {
            sections.push(self.between(
                self.heading("设置自动迁移"),
                self.text(format!("第 {} 步 / 共 3 步", e.step + 1), CAPTION, p.text2),
            ));
            sections.push(
                self.hstack(
                    S3,
                    ["1  选择内容", "2  电脑位置", "3  检查并开启"]
                        .into_iter()
                        .enumerate()
                        .map(|(i, t)| {
                            self.pill_badge(
                                t,
                                if i == e.step as usize {
                                    p.on_accent
                                } else {
                                    p.text2
                                },
                                if i == e.step as usize {
                                    p.accent
                                } else {
                                    p.hover
                                },
                            )
                        })
                        .collect(),
                ),
            );
            sections.push(self.text(
                "修改尚未生效，手机继续使用原规则。完成第三步确认后保存并应用。",
                CAPTION,
                p.text2,
            ));
            match e.step {
                0 => {
                    let mut cards = vec![];
                    for (key, name, note, icon) in [
                        ("photos", "拍摄照片", "DCIM 根目录 / Camera", UiIcon::Image),
                        ("videos", "拍摄视频", "DCIM 根目录 / Camera", UiIcon::Video),
                        ("documents", "文档", "Documents / Download", UiIcon::File),
                    ] {
                        let on = q[key] == true;
                        cards.push(
                            Expanded::new(
                                1.,
                                self.panel(vec![
                                    self.hstack(
                                        S2,
                                        vec![self.ui_icon(icon, 20., p.text), self.heading(name)],
                                    ),
                                    self.text(note, CAPTION, p.text2),
                                    self.ex_button(
                                        format!("exp-type-{key}"),
                                        if on { "✓ 已选择" } else { "选择" },
                                        EA::Type(key.into()),
                                        if on { Btn::Primary } else { Btn::Secondary },
                                    ),
                                ]),
                            )
                            .finish(),
                        );
                    }
                    sections.push(self.heading("选择要自动迁移的类型"));
                    sections.push(self.hstack(S3, cards));
                    sections.push(self.panel(vec![self.heading("什么时候迁移？"),self.wrap([(0,"写入完成后迁移"),(7,"保留最近 7 天"),(30,"保留最近 30 天")].into_iter().map(|(d,t)|self.ex_button(format!("exp-days-{d}"),t,EA::Days(d),if q["retentionDays"].as_u64().unwrap_or(7)==d{Btn::Primary}else{Btn::Secondary})).collect()),
                        self.text(if q["retentionDays"]==0{"文件停止写入至少 2 分钟后迁移；连接时新文件也会按此规则处理。"}else{"只迁移早于所选天数的文件。最近拍摄的照片和视频会继续留在手机，满期后再迁移。"},BODY,p.text2),
                        self.text("截图、应用缓存与隐藏文件不在自动迁移范围；恢复到手机的文件不会被重复迁移。",CAPTION,p.text3)]));
                }
                1 => {
                    sections.push(self.panel(vec![self.heading("选择电脑上的原件保存位置"),self.text(e.draft_folder.as_ref().map(|f|f.join("StepX Storage").display().to_string()).unwrap_or_else(||"尚未选择保存文件夹".into()),BODY,p.text),
                        self.wrap(vec![self.ex_button("exp-folder","选择文件夹…",EA::Folder,Btn::Primary),if e.draft_folder.is_some(){self.ex_button("exp-open-folder","打开位置",EA::OpenFolder,Btn::Ghost)}else{self.disabled_button("打开位置")}]),
                        self.text("迁移后的原件保存在这里。请使用长期连接、空间充足的磁盘，并保留这个目录。",BODY,p.text2)]));
                    sections.push(self.panel(vec![
                        self.heading("连接与权限"),
                        self.text(
                            if e.snapshot.online {
                                "✓ 手机已连接，USB 与无线 ADB 均可使用"
                            } else {
                                "请先连接手机；连接成功后继续"
                            },
                            BODY,
                            p.text2,
                        ),
                        self.text(
                            if e.snapshot.status["permission"] == true {
                                "✓ 手机已授予文件管理权限"
                            } else {
                                "请在手机伴侣 → 文件 → 存储扩容中开启文件管理权限"
                            },
                            BODY,
                            p.text2,
                        ),
                    ]));
                }
                _ => {
                    sections.push(
                        self.expansion_report_card(
                            e.preview.as_ref().unwrap_or(&Value::Null),
                            true,
                        ),
                    );
                    sections.push(self.panel(vec![self.heading("确认后的规则"),self.text(expansion_state::ExpansionUi::policy_summary(q),BODY,p.text),self.text(format!("电脑位置：{}",e.draft_folder.as_ref().map(|f|f.join("StepX Storage").display().to_string()).unwrap_or_default()),CAPTION,p.text2),
                        self.text("电脑收到原件并校验成功后，才清理手机原件。手机保留文件入口和相册预览。",BODY,p.text2),self.text("取回原图：在相册分享预览图至“取回电脑原图”，或从伴侣文件库恢复。电脑离线时不能取回未缓存的原件。",CAPTION,p.text3)]));
                }
            }
        } else {
            sections.push(self.expansion_card());
            sections.push(
                self.panel(vec![
                    self.between(
                        self.heading("当前迁移规则"),
                        self.ex_button("exp-edit-rule", "修改规则", EA::Settings, Btn::Secondary),
                    ),
                    self.text(
                        expansion_state::ExpansionUi::policy_summary(&e.snapshot.status["policy"]),
                        BODY,
                        p.text,
                    ),
                    self.text(
                        format!(
                            "电脑位置：{}",
                            e.config
                                .folder
                                .as_ref()
                                .map(|f| f.join("StepX Storage").display().to_string())
                                .unwrap_or_else(|| "未设置".into())
                        ),
                        CAPTION,
                        p.text2,
                    ),
                    self.wrap(vec![
                        if e.snapshot.status["policy"]["enabled"] == true {
                            self.ex_button("exp-pause", "暂停自动迁移", EA::Pause, Btn::Secondary)
                        } else {
                            self.ex_button("exp-setup", "设置并开启", EA::Settings, Btn::Primary)
                        },
                        self.ex_button(
                            "exp-open-current",
                            "打开电脑文件夹",
                            EA::OpenFolder,
                            Btn::Ghost,
                        ),
                    ]),
                ]),
            );
            if e.snapshot.online {
                sections.push(self.expansion_report_card(&e.snapshot.report, false));
            }
        }
        if !e.feedback.is_empty() {
            sections.push(self.text(
                &e.feedback,
                BODY,
                if e.feedback.contains("失败") || e.feedback.contains("未完成") {
                    p.danger
                } else {
                    p.text2
                },
            ));
        }
        if e.snapshot.error {
            sections.push(self.text(&e.snapshot.phase, BODY, p.danger));
        }

        if !e.settings && !e.snapshot.files.is_empty() {
            sections.push(self.heading("迁移记录"));
            let mut controls = vec![];
            for (k, n) in [
                ("all", "全部"),
                ("photos", "照片"),
                ("videos", "视频"),
                ("documents", "文档"),
            ] {
                controls.push(self.ex_button(
                    format!("exp-category-{k}"),
                    n,
                    EA::Filter(k.into()),
                    if e.config.kind == k {
                        Btn::Primary
                    } else {
                        Btn::Secondary
                    },
                ));
            }
            let mut sorts = vec![];
            for (k, n) in [("date", "日期"), ("name", "名称"), ("size", "大小")] {
                sorts.push(self.ex_button(
                    format!("exp-sort-{k}"),
                    n,
                    EA::Sort(k.into()),
                    if e.config.sort == k {
                        Btn::Secondary
                    } else {
                        Btn::Ghost
                    },
                ));
            }
            sorts.push(self.ex_button(
                "exp-direction",
                if e.config.descending {
                    "降序 ↓"
                } else {
                    "升序 ↑"
                },
                EA::Descending,
                Btn::Ghost,
            ));
            let status_filters = [
                ("all", "全部状态"),
                ("archived", "已迁移"),
                ("restored", "已恢复"),
                ("prepared", "待迁移"),
                ("verified", "待确认"),
                ("failed", "失败"),
            ]
            .into_iter()
            .map(|(k, n)| {
                self.ex_button(
                    format!("exp-state-{k}"),
                    n,
                    EA::State(k.into()),
                    if e.config.filter == k {
                        Btn::Secondary
                    } else {
                        Btn::Ghost
                    },
                )
            })
            .collect();
            sections.push(self.panel(vec![
                self.text(
                    "筛选文件库 · 只改变下面显示的记录，不改变迁移规则",
                    CAPTION,
                    p.text2,
                ),
                self.between(self.wrap(controls), self.wrap(sorts)),
                self.wrap(status_filters),
            ]));
            let mut files = e
                .snapshot
                .files
                .iter()
                .filter(|j| {
                    (e.config.kind == "all" || j["kind"] == e.config.kind)
                        && (e.config.filter == "all" || j["state"] == e.config.filter)
                })
                .collect::<Vec<_>>();
            files.sort_by(|a, b| {
                let order = match e.config.sort.as_str() {
                    "name" => label(a, "name")
                        .to_lowercase()
                        .cmp(&label(b, "name").to_lowercase()),
                    "size" => a["size"].as_u64().cmp(&b["size"].as_u64()),
                    _ => a["modified"].as_u64().cmp(&b["modified"].as_u64()),
                };
                if e.config.descending {
                    order.reverse()
                } else {
                    order
                }
            });
            if files.is_empty() {
                sections.push(self.panel(if e.snapshot.files.is_empty(){vec![self.heading("尚未迁移文件"),self.text("先选择文件类型和电脑保存位置，再确认开启。不会因为点击上面的分类按钮就迁移文件。",BODY,p.text2),self.ex_button("exp-empty-settings","设置迁移规则",EA::Settings,Btn::Primary)]}else{vec![self.heading("没有符合筛选条件的记录"),self.text("切换为“全部”和“全部状态”可查看其他文件。迁移规则未改变。",BODY,p.text2)]}));
            }
            for j in files.iter().take(e.limit) {
                let id = label(j, "id");
                let date =
                    chrono::DateTime::from_timestamp_millis(j["modified"].as_i64().unwrap_or(0))
                        .map(|d| {
                            d.with_timezone(&chrono::Local)
                                .format("%Y-%m-%d %H:%M")
                                .to_string()
                        })
                        .unwrap_or_default();
                let icon = match j["kind"].as_str() {
                    Some("photos") => UiIcon::Image,
                    Some("videos") => UiIcon::Video,
                    _ => UiIcon::File,
                };
                let mut actions = vec![self.ex_button(
                    format!("exp-open-{id}"),
                    "打开原件",
                    EA::Open(id.clone()),
                    Btn::Secondary,
                )];
                if j["state"] == "archived" {
                    actions.push(self.ex_button(
                        format!("exp-restore-{id}"),
                        "恢复到手机",
                        EA::Restore(id),
                        Btn::Ghost,
                    ));
                }
                sections.push(self.panel(vec![
                    self.between(
                        self.hstack(
                            S2,
                            vec![
                                self.ui_icon(icon, 20., p.text2),
                                self.heading(label(j, "name")),
                            ],
                        ),
                        self.pill_badge(state(j), p.text2, p.hover),
                    ),
                    self.text(
                        format!(
                            "{date} · {} · {}",
                            storage::format_bytes(j["size"].as_u64().unwrap_or(0)),
                            label(j, "path")
                        ),
                        CAPTION,
                        p.text3,
                    ),
                    self.wrap(actions),
                ]));
            }
            if files.len() > e.limit {
                sections.push(self.ex_button("exp-more", "加载更多", EA::More, Btn::Secondary));
            }
        }
        let body = self.pane_scroll(self.scroll.clone(), self.vstack(S4, sections));
        if !e.settings {
            return body;
        }
        let mut buttons = vec![self.ex_button("exp-cancel", "取消", EA::Back, Btn::Ghost)];
        if e.step > 0 {
            buttons.push(self.ex_button("exp-previous", "上一步", EA::Previous, Btn::Secondary));
        }
        let ready = e.pending.is_none()
            && (e.step < 2 || (e.preview.is_some() && e.preview_pending.is_none()));
        buttons.push(if ready {
            self.ex_button(
                "exp-next",
                if e.step == 2 {
                    "确认并开启自动迁移"
                } else {
                    "下一步"
                },
                if e.step == 2 { EA::Confirm } else { EA::Next },
                Btn::Primary,
            )
        } else {
            self.disabled_button(if e.pending.is_some() {
                "正在保存…"
            } else {
                "正在检查…"
            })
        });
        let footer = Container::new(self.between(
            self.vstack(
                S1,
                vec![
                    self.line_w(
                        format!("第 {} / 3 步", e.step + 1),
                        BODY,
                        p.text,
                        Weight::Semibold,
                    ),
                    self.line(
                        if e.step == 2 {
                            "确认后生效，取消保留当前手机规则"
                        } else {
                            "完成选择后，点击右侧下一步"
                        },
                        CAPTION,
                        p.text2,
                    ),
                ],
            ),
            self.wrap(buttons),
        ))
        .with_uniform_padding(S3)
        .with_background_color(p.bg)
        .with_border(Border::top(1.).with_border_color(p.border))
        .finish();
        Flex::column()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_child(Expanded::new(1., body).finish())
            .with_child(footer)
            .finish()
    }
}

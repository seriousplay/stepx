use super::super::{
    theme::*, update_state::UpdateAction, widgets::*, Action, Companion, Page, ToolOperation,
    ToolPage,
};
use facloud_ui::{elements::*, fonts::Weight};
use stepx_companion::{storage::format_bytes, toolbox::FileInfo};

impl Companion {
    fn storage_actions(&self) -> Vec<El> {
        if self.busy {
            return vec![self.disabled_button("正在处理…")];
        }
        vec![
            self.button(
                "tools-settings",
                "手机存储",
                Action::OpenStorageSettings,
                Btn::Secondary,
            ),
            self.button(
                "tools-scan",
                "重新扫描",
                Action::ScanStorage,
                if self.tools_pending.is_some() {
                    Btn::Secondary
                } else {
                    Btn::Primary
                },
            ),
        ]
    }

    fn tool_file_row(
        &self,
        index: usize,
        file: &FileInfo,
        operation: Option<ToolOperation>,
        keep: Option<&str>,
    ) -> El {
        let p = self.p();
        let name = file.path.rsplit('/').next().unwrap_or(&file.path);
        let mut details = vec![
            self.line_w(name, BODY, p.text, Weight::Medium),
            self.line(&file.path, CAPTION, p.text2),
        ];
        if let Some(keep) = keep {
            details.push(self.line(format!("保留：{keep}"), CAPTION, p.text3));
        }
        let action = match operation {
            Some(operation)
                if !self.busy
                    && (file.bytes <= 256 * 1024 * 1024
                        || matches!(operation, ToolOperation::DeleteFile(_))) =>
            {
                self.button(
                    format!("tool-op-{index}"),
                    operation.label(),
                    Action::PrepareToolOperation(operation),
                    Btn::Secondary,
                )
            }
            Some(_) => self.disabled_button(if self.busy {
                "处理中"
            } else {
                "超过 256 MiB"
            }),
            None => self.button(
                format!("tool-locate-{index}"),
                "查看文件",
                Action::LocateToolFile(file.path.clone()),
                Btn::Secondary,
            ),
        };
        Container::new(self.between(
            self.vstack(S1, details),
            self.hstack(
                S4,
                vec![
                    self.mono(format_bytes(file.bytes), CAPTION, p.text2),
                    action,
                ],
            ),
        ))
        .with_uniform_padding(S3)
        .finish()
    }

    fn storage_tool_page(&self) -> El {
        let p = self.p();
        let mut sections = Vec::new();
        if self.error {
            sections.push(self.panel(vec![self.text(&self.status, BODY, p.danger)]));
        }
        if let Some(report) = &self.storage_report {
            let metrics = [
                ("手机可用", format_bytes(report.available_bytes)),
                ("数据分区总量", format_bytes(report.total_bytes)),
                ("重复副本大小", format_bytes(report.duplicate_bytes)),
            ]
            .into_iter()
            .map(|(label, value)| {
                Expanded::new(
                    1.,
                    self.vstack(
                        S2,
                        vec![
                            self.line(label, CAPTION, p.text2),
                            self.line_w(value, METRIC, p.text, Weight::Semibold),
                        ],
                    ),
                )
                .finish()
            })
            .collect();
            sections.push(self.panel(vec![
                self.hstack(S5, metrics),
                self.text(
                    "重复副本大小是文件内容大小；实际可用空间以处理后的手机读数为准。",
                    CAPTION,
                    p.text3,
                ),
            ]));
        }
        if let Some(result) = &self.tools_last_result {
            sections.push(self.panel(vec![self.text(result, CAPTION, p.text)]));
        }

        let (system_title, system_note) = match &self.system_compression {
            None => (
                "应用透明压缩 · 待检测",
                "检查当前固件是否支持保持原路径、直接使用的系统压缩。",
            ),
            Some(s) if !s.kernel_support => {
                ("应用透明压缩 · 内核不支持", "当前固件缺少系统压缩能力。")
            }
            Some(s) if !s.volume_support => (
                "应用透明压缩 · 当前不可用",
                "当前数据分区未开启压缩特性，仅提升应用权限无法启用。",
            ),
            Some(_) => (
                "应用透明压缩 · 需要适配",
                "检测到存储支持；应用更新、写入和还原兼容性尚未验证。",
            ),
        };
        sections.push(self.panel(vec![self.between(
            self.vstack(
                S1,
                vec![
                    self.heading(system_title),
                    self.text(system_note, CAPTION, p.text2),
                ],
            ),
            if self.busy {
                self.disabled_button("检测能力")
            } else {
                self.button(
                    "tools-probe",
                    "检测能力",
                    Action::ProbeSystemCompression,
                    Btn::Secondary,
                )
            },
        )]));
        sections.push(self.panel(vec![self.between(
            self.vstack(
                S1,
                vec![
                    self.heading("应用缓存"),
                    self.text(
                        "由手机系统清理可再生成的缓存，应用后续使用时会重新生成。",
                        CAPTION,
                        p.text2,
                    ),
                ],
            ),
            if self.busy {
                self.disabled_button("清理缓存")
            } else {
                self.button(
                    "tools-cache",
                    "清理缓存",
                    Action::PrepareToolOperation(ToolOperation::ClearCache),
                    Btn::Secondary,
                )
            },
        )]));

        if let Some(operation) = &self.tools_pending {
            let explanation = match operation {
                ToolOperation::ClearCache => "请求手机系统清理所有应用可回收的缓存。缓存的图片等内容可能需要重新加载；不执行清除应用数据或卸载。".to_string(),
                ToolOperation::Compress(_) => "压缩并校验后，以 .gz 文件替换原文件，释放手机空间。使用前需要还原；不会备份到电脑。".to_string(),
                ToolOperation::Restore(_) => "还原后移除 .gz 文件，会占用更多手机空间；已有同名文件时不覆盖。".to_string(),
                ToolOperation::DeleteFile(_) => "永久删除此文件以释放手机空间。此操作不经过回收站，无法撤销。".to_string(),
                ToolOperation::DeleteDuplicate { keep, .. } => format!("重新核对两份内容一致后，永久删除此副本。保留：{}。此操作不经过回收站。", keep.path),
            };
            sections.push(self.panel(vec![
                    self.heading(format!("确认{}", operation.label())),
                    self.text(
                        operation
                            .file()
                            .map(|f| f.path.as_str())
                            .unwrap_or("手机内部存储 · 应用缓存"),
                        BODY,
                        p.text,
                    ),
                    self.text(explanation, CAPTION, p.text2),
                    self.hstack(
                        S2,
                        vec![
                            self.button(
                                "tool-cancel",
                                "取消",
                                Action::CancelToolOperation,
                                Btn::Secondary,
                            ),
                            self.button(
                                "tool-confirm",
                                "确认处理",
                                Action::ConfirmToolOperation,
                                if matches!(
                                    operation,
                                    ToolOperation::DeleteDuplicate { .. }
                                        | ToolOperation::DeleteFile(_)
                                ) {
                                    Btn::DangerSolid
                                } else {
                                    Btn::Primary
                                },
                            ),
                        ],
                    ),
                ]));
        }
        sections.push(
            self.hstack(
                S2,
                ["文档压缩", "重复文件", "大文件"]
                    .into_iter()
                    .enumerate()
                    .map(|(i, label)| {
                        self.button(
                            format!("tools-tab-{i}"),
                            if self.tools_tab as usize == i {
                                format!("{label} · 当前")
                            } else {
                                label.to_string()
                            },
                            Action::ToolTab(i as u8),
                            Btn::Secondary,
                        )
                    })
                    .collect(),
            ),
        );
        let Some(report) = &self.storage_report else {
            sections.push(self.panel(vec![
                self.heading(if self.busy {
                    "正在读取手机存储"
                } else {
                    "等待扫描"
                }),
                self.text(
                    "扫描下载、文档、相册、图片、视频和音乐目录。应用私有数据不在扫描范围。",
                    CAPTION,
                    p.text2,
                ),
            ]));
            return self.vstack(S4, sections);
        };
        let scope = format!(
            "{} · 已扫描 {} 个文件 · {} · {} 个目录项无法读取 · {} 个文件未完成重复校验",
            super::format_time(report.scanned_at_ms),
            report.scanned_files,
            if report.limited {
                "达到扫描上限，结果不完整"
            } else {
                "公开目录扫描完成"
            },
            report.unreadable,
            report.hash_skipped
        );
        sections.push(self.text(scope, CAPTION, p.text2));
        sections.push(self.text(
            match self.tools_tab {
                0 => "仅压缩文本文档，每个文件最多 256 MiB。压缩无收益时保留原文件。",
                1 => "按内容校验重复项，每组保留第一份。删除前会再次校验，并显示具体路径。",
                _ => "按大小列出前 20 个文件。确认后永久删除，不经过回收站。",
            },
            CAPTION,
            p.text3,
        ));

        let mut entries: Vec<(&FileInfo, Option<ToolOperation>, Option<&str>)> = vec![];
        match self.tools_tab {
            0 => {
                for f in &report.documents {
                    let op = if f.path.ends_with(".gz") {
                        ToolOperation::Restore(f.clone())
                    } else {
                        ToolOperation::Compress(f.clone())
                    };
                    entries.push((f, Some(op), None));
                }
            }
            1 => {
                for group in &report.duplicates {
                    if let Some(keep) = group.first() {
                        for f in group.iter().skip(1) {
                            entries.push((
                                f,
                                Some(ToolOperation::DeleteDuplicate {
                                    file: f.clone(),
                                    keep: keep.clone(),
                                }),
                                Some(&keep.path),
                            ));
                        }
                    }
                }
            }
            _ => {
                for f in &report.largest {
                    entries.push((f, Some(ToolOperation::DeleteFile(f.clone())), None));
                }
            }
        }
        if entries.is_empty() {
            sections.push(self.panel(vec![self.text(
                match self.tools_tab {
                    0 => "扫描范围内没有可压缩或还原的文本文档。",
                    1 => "已校验的文件中没有发现重复副本。",
                    _ => "扫描范围内没有文件。",
                },
                BODY,
                p.text2,
            )]));
        } else {
            let page_count = entries.len().div_ceil(10);
            let page = self.tools_result_page.min(page_count - 1);
            sections.push(
                self.list(
                    entries
                        .iter()
                        .enumerate()
                        .skip(page * 10)
                        .take(10)
                        .map(|(i, (f, op, keep))| self.tool_file_row(i, f, op.clone(), *keep))
                        .collect(),
                ),
            );
            let mut pagination = vec![self.line(
                format!(
                    "共 {} 项 · 第 {} / {} 页",
                    entries.len(),
                    page + 1,
                    page_count
                ),
                CAPTION,
                p.text2,
            )];
            if page > 0 {
                pagination.push(self.button(
                    "tools-prev",
                    "上一页",
                    Action::ToolResultPage(page - 1),
                    Btn::Ghost,
                ));
            }
            if page + 1 < page_count {
                pagination.push(self.button(
                    "tools-next",
                    "下一页",
                    Action::ToolResultPage(page + 1),
                    Btn::Ghost,
                ));
            }
            sections.push(self.hstack(S3, pagination));
        }
        self.vstack(S4, sections)
    }
}

impl Companion {
    pub(in super::super) fn tools_actions(&self) -> Vec<El> {
        if self.tool_page.is_none() || self.selected.is_none() {
            return vec![];
        }
        if self.busy {
            return vec![self.disabled_button("正在处理…")];
        }
        match self.tool_page {
            Some(ToolPage::Storage) => self.storage_actions(),
            Some(ToolPage::Google) => vec![self.button(
                "google-check",
                "检测状态",
                Action::GoogleSuiteInspect,
                Btn::Secondary,
            )],
            Some(ToolPage::Companion) => vec![self.button(
                "companion-check",
                "检测状态",
                Action::CompanionInspect,
                Btn::Secondary,
            )],
            Some(ToolPage::Services) => vec![self.button(
                "services-check",
                "刷新状态",
                Action::InspectServices,
                Btn::Secondary,
            )],
            Some(ToolPage::Device) => {
                vec![self.button("device-check", "重新检测", Action::Summary, Btn::Secondary)]
            }
            _ => vec![],
        }
    }

    fn utility_button(&self, id: &str, label: &str, action: Action, primary: bool) -> El {
        if self.busy || self.selected.is_none() {
            self.disabled_button(label)
        } else {
            self.button(
                id,
                label,
                action,
                if primary {
                    Btn::Primary
                } else {
                    Btn::Secondary
                },
            )
        }
    }

    fn tool_card(&self, tool: ToolPage) -> El {
        let p = self.p();
        let (symbol, state) = match tool {
            ToolPage::Storage => (
                "▤",
                self.storage_report
                    .as_ref()
                    .map(|s| format!("可用 {}", format_bytes(s.available_bytes)))
                    .unwrap_or("进入后扫描".into()),
            ),
            ToolPage::Google => (
                "G",
                if !self.google_suite.checked {
                    "待检测"
                } else if self.google_suite.compatibility_active {
                    "已安装 · 兼容补丁已应用"
                } else if self.google_suite.installed {
                    "已安装 · 查看状态"
                } else {
                    "待安装 / 完善"
                }
                .into(),
            ),
            ToolPage::Companion => (
                "📱",
                if self.companion_status.installed {
                    format!("已安装 · {}", self.companion_status.version)
                } else if self.companion_status.checked {
                    "等待安装".into()
                } else {
                    "进入后检测".into()
                },
            ),
            ToolPage::Apk => ("⊞", "选择本地文件".into()),
            ToolPage::Services => (
                "≡",
                if self.system_services.is_empty() {
                    "进入后检测".into()
                } else {
                    format!("{} 项服务", self.system_services.len())
                },
            ),
            ToolPage::Screen => (
                "□",
                if self.mirror.is_some() {
                    "正在投屏"
                } else {
                    "按需开启"
                }
                .into(),
            ),
            ToolPage::Device => ("◈", "读取手机状态".into()),
            ToolPage::Updates => ("↻", self.updates.title().to_owned()),
        };
        self.panel(vec![
            self.between(
                self.hstack(
                    S3,
                    vec![
                        self.icon_box(symbol, 32., p.surface, p.text2, None),
                        self.heading(tool.title()),
                    ],
                ),
                self.line(
                    if self.selected.is_none() && tool != ToolPage::Updates {
                        "需要连接手机".into()
                    } else {
                        state
                    },
                    MICRO,
                    p.text3,
                ),
            ),
            self.text(tool.description(), BODY, p.text2),
            self.between(
                self.line(
                    ["", "空间整理", "应用与服务", "设备协同"][tool.category() as usize],
                    CAPTION,
                    p.text3,
                ),
                self.button(
                    format!("tool-open-{tool:?}"),
                    "› 打开工具",
                    Action::OpenTool(Some(tool)),
                    Btn::Secondary,
                ),
            ),
        ])
    }

    pub(in super::super) fn tools_page(&self) -> El {
        let p = self.p();
        let mut parts = vec![];
        if let Some(tool) = self.tool_page {
            parts.push(self.hstack(
                S2,
                vec![
                    self.button(
                        "tools-back",
                        "← 全部工具",
                        Action::OpenTool(None),
                        Btn::Ghost,
                    ),
                    self.line(format!("/ {}", tool.title()), CAPTION, p.text3),
                ],
            ));
        }
        if self.selected.is_none() && self.tool_page != Some(ToolPage::Updates) {
            parts.push(self.panel(vec![self.between(
                self.text("连接手机后即可使用；可以先浏览工具。", BODY, p.text2),
                self.button(
                    "tools-connect",
                    "连接设备",
                    Action::Page(Page::Connect),
                    Btn::Primary,
                ),
            )]));
        }
        if self.busy {
            let progress = self
                .google_progress
                .lock()
                .ok()
                .filter(|s| {
                    !s.is_empty()
                        && matches!(self.job_tool, Some(ToolPage::Google | ToolPage::Companion))
                })
                .map(|s| s.clone())
                .unwrap_or_else(|| self.status.clone());
            parts.push(self.panel(vec![self.text(progress, CAPTION, p.accent)]));
        } else if let Some((tool, message, error)) = &self.tools_feedback {
            if self.tool_page.is_none() || self.tool_page == *tool {
                let label = format!(
                    "{} · {}",
                    tool.map(ToolPage::title).unwrap_or("小工具"),
                    message
                );
                parts.push(self.panel(vec![self.text(
                    label,
                    CAPTION,
                    if *error { p.danger } else { p.text2 },
                )]));
            }
        }
        if let Some(action) = self.tool_confirmation.as_deref() {
            let message = match action {
                Action::GoogleSuiteInstall => "将安装或修复 StepX伴侣，并保留已有 Google 组件。完整安装且版本已适配时，会备份并应用 Google Play 兼容补丁；首次安装后需按提示重启解锁。",
                Action::GoogleSuiteMaintain => "将核对手机固件与 Google 组件版本，保存可恢复的备份，再应用兼容补丁并重新启动 Google 服务。用于恢复商店浏览和下载安装，不改变 Google 官方认证状态。请保持手机已解锁并连接电脑。",
                Action::GoogleSuiteRollback => "将核对备份并恢复补丁修改的设置，重新启动 Google 服务。保留 Google 账号和已安装应用；设备未认证的拦截页可能恢复。",
                Action::ToggleSystemService(_, true) => "将重新启用此服务，它可能恢复后台运行。",
                _ => "将停用此服务，相关功能会停止；可在此处重新启用。不会卸载应用或删除数据。",
            };
            let target = match action {
                Action::ToggleSystemService(package, _) => self
                    .system_services
                    .iter()
                    .find(|s| &s.package == package)
                    .map(|s| format!("{} · {}", s.name, s.package))
                    .unwrap_or_else(|| package.clone()),
                Action::GoogleSuiteMaintain | Action::GoogleSuiteRollback => "Google Play 兼容补丁 · 当前连接的手机".into(),
                _ => "Google Play + StepX伴侣 · 当前连接的手机".into(),
            };
            parts.push(self.panel(vec![
                self.heading("确认操作"),
                self.text(target, BODY, p.text),
                self.text(message, BODY, p.text2),
                self.hstack(
                    S2,
                    vec![
                        self.button(
                            "utility-cancel",
                            "取消",
                            Action::CancelUtility,
                            Btn::Secondary,
                        ),
                        self.utility_button(
                            "utility-confirm",
                            "确认继续",
                            Action::ConfirmUtility,
                            true,
                        ),
                    ],
                ),
            ]));
        }
        match self.tool_page {
            None => {
                parts.push(self.vstack(S2, vec![self.heading("工具集合"), self.text("按需要选择工具，检测和操作结果会留在这里。", BODY, p.text2)]));
                parts.push(self.segmented("tool-categories", ["全部 · 8", "空间整理 · 1", "应用与服务 · 3", "设备协同 · 4"].into_iter().enumerate().map(|(i, title)|
                    (title.into(), i as u8 == self.tool_category, Action::ToolCategory(i as u8))
                ).collect()));
                let tools: Vec<_> = ToolPage::ALL.into_iter().filter(|t| self.tool_category == 0 || t.category() == self.tool_category).collect();
                for row in tools.chunks(2) {
                    let mut cards: Vec<El> = row.iter().map(|t| Expanded::new(1., self.tool_card(*t)).finish()).collect();
                    if row.len() == 1 { cards.push(Expanded::new(1., Empty::new().finish()).finish()); }
                    parts.push(self.hstack(S4, cards));
                }
            }
            Some(ToolPage::Updates) => parts.push(self.updates_tool_page()),
            Some(ToolPage::Companion) => parts.push(self.companion_tool_page()),
            Some(_) if self.selected.is_none() => {}
            Some(ToolPage::Storage) => parts.push(self.storage_tool_page()),
            Some(ToolPage::Google) => parts.push(self.google_tool_page()),
            Some(ToolPage::Apk) => parts.push(self.panel(vec![
                self.heading("安装本地 APK"), self.text("选择电脑上的单个 APK 文件。已有应用会尝试更新，版本或签名不兼容时会显示失败原因。", BODY, p.text2),
                self.text("拆分安装包（APKS / XAPK）不支持在此安装。Google 服务请使用 Google Play 工具。", CAPTION, p.text3),
                self.hstack(S2, vec![self.utility_button("tool-apk", "选择 APK…", Action::InstallApk, true), self.button("tool-app-list", "已安装应用", Action::Page(Page::Apps), Btn::Secondary)]),
            ])),
            Some(ToolPage::Screen) => {
                parts.push(self.panel(vec![self.heading("截图"), self.text("截取当前手机画面，预览后选择保存位置。", BODY, p.text2), self.hstack(S2, vec![
                    self.utility_button("tool-shot", "截取画面", Action::Screenshot, true),
                    if self.screenshot.is_some() { self.utility_button("tool-save-shot", "保存截图", Action::SaveScreenshot, false) } else { self.disabled_button("保存截图") },
                ])]));
                if let Some((_, path)) = &self.screenshot {
                    parts.push(ConstrainedBox::new(Image::new(facloud_ui::assets::asset_cache::AssetSource::LocalFile { path: path.to_string_lossy().into_owned() }, CacheOption::BySize).contain().finish()).with_height(300.).finish());
                }
                parts.push(self.panel(vec![self.heading("实时投屏"), self.text("在独立窗口查看并操作手机，投屏期间保持手机屏幕活跃。", BODY, p.text2),
                    self.text("关闭投屏窗口或点击结束投屏后，手机恢复正常的自动息屏。", CAPTION, p.text3),
                    self.utility_button("tool-mirror", if self.mirror.is_some() { "结束投屏" } else { "开启投屏" }, if self.mirror.is_some() { Action::StopMirror } else { Action::Mirror }, false),
                ]));
            }
            Some(ToolPage::Services) => {
                parts.push(self.panel(vec![self.text("仅列出已适配的服务。停用会影响对应功能，请逐项选择；当前输入法不允许停用。", BODY, p.text2),
                    self.utility_button("tool-ime-settings", "手机输入法设置", Action::OpenInputSettings, false)]));
                if self.system_services.is_empty() { parts.push(self.panel(vec![self.text("尚无检测结果，请点击刷新状态。", BODY, p.text2)])); }
                for (i, service) in self.system_services.iter().enumerate() {
                    let can_toggle = service.safe_to_remove && !service.status_note.contains("当前默认") && service.status_note != "未安装";
                    parts.push(self.panel(vec![self.between(self.vstack(S1, vec![self.heading(&service.name), self.line(&service.status_note, CAPTION, p.text2)]),
                        if can_toggle { self.utility_button(&format!("tool-service-{i}"), if service.enabled { "停用" } else { "启用" }, Action::PrepareUtility(Box::new(Action::ToggleSystemService(service.package.clone(), !service.enabled))), false) } else { self.disabled_button("不可停用") }),
                        self.text(&service.package, CAPTION, p.text3),
                    ]));
                }
            }
            Some(ToolPage::Device) => {
                let report = self.selected.as_ref().map(|d| format!("型号：{}\nAndroid：{}\n连接：{}\n\n{}", d.model, d.android, d.transport, self.summary)).unwrap_or_default();
                parts.push(self.panel(vec![self.heading("设备状态"), self.text(if self.summary.is_empty() { "尚无检测结果，请点击重新检测。" } else { &report }, BODY, p.text2)]));
                if !self.summary.is_empty() { parts.push(self.button("tool-copy-report", "复制检测结果", Action::CopyText(report, "已复制设备检测结果".into()), Btn::Secondary)); }
            }
        }
        self.vstack(S4, parts)
    }

    fn updates_tool_page(&self) -> El {
        use stepx_companion::ota::{OtaStatus, VersionRelation};
        let p = self.p();
        let u = &self.updates;
        let mut summary = vec![self.between(
            self.vstack(
                S1,
                vec![
                    self.heading("手机系统更新"),
                    self.text(
                        "官方检查会打开手机更新界面，由系统确认可用版本。",
                        CAPTION,
                        p.text2,
                    ),
                ],
            ),
            if u.busy || u.transfer.is_some() {
                self.disabled_button("检查中…")
            } else if self.selected.is_none() {
                self.button(
                    "ota-connect",
                    "连接手机",
                    Action::Page(Page::Connect),
                    Btn::Primary,
                )
            } else {
                self.hstack(
                    S2,
                    vec![
                        self.button(
                            "ota-channels",
                            "测试渠道",
                            Action::Updates(UpdateAction::Check),
                            Btn::Secondary,
                        ),
                        self.button(
                            "ota-check",
                            "官方检查",
                            Action::Updates(UpdateAction::CheckOfficial),
                            Btn::Primary,
                        ),
                    ],
                )
            },
        )];
        if let Some(phone) = &u.phone {
            summary.push(self.divider());
            summary.push(self.text(format!("当前系统  {}", phone.firmware), BODY, p.text));
            summary.push(self.line(
                format!("安全补丁  {}", phone.security_patch),
                CAPTION,
                p.text2,
            ));
        } else if let Some(device) = &self.selected {
            summary.push(self.text(
                format!("已连接 {}，点击检查以读取系统版本。", device.model),
                BODY,
                p.text2,
            ));
        } else {
            summary.push(self.text(
                "连接 StepX 手机后即可检测，无需配置桌面软件更新源。",
                BODY,
                p.text2,
            ));
        }
        if let Some(progress) = &u.progress {
            summary.push(self.text(progress, BODY, p.accent));
        }
        for error in [&u.error, &u.phone_error].into_iter().flatten() {
            summary.push(self.text(error, BODY, p.danger));
        }
        if let Some(notice) = &u.notice {
            summary.push(self.text(notice, CAPTION, p.text2));
        }
        if let Some((phase, done, total)) = &u.transfer_progress {
            summary.push(self.divider());
            summary.push(self.between(
                self.text(phase, BODY, p.text),
                self.button(
                    "ota-cancel-transfer",
                    "取消",
                    Action::Updates(UpdateAction::CancelDownload),
                    Btn::Ghost,
                ),
            ));
            summary.push(self.progress(
                if *total > 0 {
                    Some((done.saturating_mul(100) / total).min(100) as u8)
                } else {
                    None
                },
                p.accent,
            ));
            if *total > 0 {
                summary.push(self.line(
                    format!("{} / {}", format_bytes(*done), format_bytes(*total)),
                    CAPTION,
                    p.text2,
                ));
            }
        }
        let mut sections = vec![self.panel(summary)];
        if let Some(staged) = &u.staged {
            let can_install = u.report.as_ref().is_some_and(|r| {
                r.eligibility_verified
                    && r.channels
                        .iter()
                        .filter(|c| c.channel == "current")
                        .flat_map(|c| &c.releases)
                        .any(|release| {
                            release.slot == "new"
                                && release.relation == VersionRelation::Newer
                                && release.need_update
                                && release.exists_update
                                && release.version == staged.package.version
                                && release.sha256 == staged.package.firmware().sha256
                                && release.md5 == staged.package.firmware().md5
                                && !release.mandatory_clear
                                && release.size_bytes == staged.package.size_bytes
                        })
            });
            let mut ready = vec![
                self.heading("手机上的更新包已就绪"),
                self.text(&staged.package.version, BODY, p.text),
                self.line(format_bytes(staged.package.size_bytes), CAPTION, p.text2),
                self.text(
                    format!("手机位置：{}", staged.remote_path),
                    CAPTION,
                    p.text2,
                ),
                self.button(
                    "ota-copy-path",
                    "复制手机路径",
                    Action::CopyText(staged.remote_path.clone(), "已复制手机固件路径".into()),
                    Btn::Ghost,
                ),
            ];
            if can_install {
                ready.push(if u.busy {
                    self.disabled_button("正在处理…")
                } else {
                    self.button(
                        "ota-prepare-install",
                        "准备安装",
                        Action::Updates(UpdateAction::PrepareInstall),
                        Btn::Primary,
                    )
                });
                if u.confirm_install {
                    ready.push(self.divider());
                    ready.push(self.text(format!("即将升级当前连接的手机至 {}。安装会交给手机官方校验器，请求保留数据且不自动重启。", staged.package.version), BODY, p.text));
                    ready.push(self.text("请保持连接，完成正在手机上进行的工作。若官方策略要求清除数据，请改在手机更新器中核对处理。", CAPTION, p.text2));
                    ready.push(self.hstack(
                        S2,
                        vec![
                            self.button(
                                "ota-install-confirm",
                                "确认提交系统安装",
                                Action::Updates(UpdateAction::ConfirmInstall),
                                Btn::Primary,
                            ),
                            self.button(
                                "ota-install-cancel",
                                "暂不安装",
                                Action::Updates(UpdateAction::CancelInstall),
                                Btn::Secondary,
                            ),
                        ],
                    ));
                }
            } else {
                ready.push(self.text(
                    "此包已完成下载验证；本次尚未确认可用于这台手机的升级，不会重刷同版或降级。",
                    BODY,
                    p.text2,
                ));
            }
            sections.push(self.panel(ready));
        }
        if let Some(report) = &u.report {
            let mut results = vec![self.between(
                self.heading(u.title()),
                self.line(format!("检测时间  {}", report.checked_at), CAPTION, p.text3),
            )];
            for channel in &report.channels {
                let color = match channel.status {
                    OtaStatus::Available => p.success,
                    OtaStatus::AccessDenied
                    | OtaStatus::Failed
                    | OtaStatus::DeviceContextMissing => p.warning,
                    _ => p.text2,
                };
                results.push(self.between(
                    self.line(channel.label(), BODY, p.text),
                    self.line(channel.status_label(), BODY, color),
                ));
                if u.details && !channel.message.is_empty() {
                    results.push(self.text(&channel.message, CAPTION, p.text2));
                }
                if let Some(plan) = &channel.plan {
                    results.push(self.text(format!("发布计划：{}", plan.note), CAPTION, p.text2));
                }
            }
            results.push(self.divider());
            if report.install_status.internal_permission == Some(false) {
                results.push(self.text("此手机未开放桌面后台安装权限。下载包可校验并传到手机，安装须由手机官方更新器确认。", CAPTION, p.text2));
            }
            results.push(self.text(
                "未返回新版本不代表不存在内部构建；测试渠道以官方开放资格为准。",
                CAPTION,
                p.text3,
            ));
            results.push(self.hstack(
                S2,
                vec![self.button(
                    "ota-details",
                    if u.details {
                        "收起检测详情"
                    } else {
                        "检测详情"
                    },
                    Action::Updates(UpdateAction::ToggleDetails),
                    Btn::Ghost,
                )],
            ));
            if u.details {
                results.push(self.text(&report.limitation, CAPTION, p.text3));
                results.push(self.text(
                    format!(
                        "服务环境：{} · 本机信息：{}",
                        report.environment,
                        if report.identity_complete {
                            "已读取"
                        } else {
                            "部分信息不可读"
                        }
                    ),
                    CAPTION,
                    p.text2,
                ));
                for channel in &report.channels {
                    results.push(self.text(
                        format!(
                                "{} · HTTP {} · 服务返回码 {}",
                                channel.label(),
                                channel
                                    .http_status
                                    .map(|n| n.to_string())
                                    .unwrap_or("—".into()),
                                channel
                                    .server_code
                                    .map(|n| n.to_string())
                                    .unwrap_or("—".into())
                            ),
                        CAPTION,
                        p.text3,
                    ));
                }
            }
            sections.push(self.panel(results));
            let mut shown = std::collections::HashSet::new();
            for (ci, channel) in report.channels.iter().enumerate() {
                for (ri, release) in channel.releases.iter().enumerate() {
                    if !shown.insert(release.version.clone()) {
                        continue;
                    }
                    let state = match release.relation {
                        VersionRelation::Current => "与本机同版",
                        VersionRelation::Newer => "高于本机版本",
                        VersionRelation::Older => "低于本机版本",
                        VersionRelation::Unknown => "版本关系待确认",
                    };
                    let mut card = vec![
                        self.between(
                            self.heading(format!("{} · {}", channel.label(), state)),
                            self.line(&release.channel_label, CAPTION, p.text3),
                        ),
                        self.text(&release.version, BODY, p.text),
                        self.line(
                            format!(
                                "{} · {}",
                                release.release_date,
                                format_bytes(release.size_bytes)
                            ),
                            CAPTION,
                            p.text2,
                        ),
                    ];
                    if release.mandatory_clear {
                        card.push(self.text(
                            "官方标记：此升级需要清除数据，桌面不会自动提交安装。",
                            BODY,
                            p.warning,
                        ));
                    }
                    if release.mandatory_upgrade {
                        card.push(self.line("官方标记：必需更新", CAPTION, p.text2));
                    }
                    if !release.release_notes.is_empty() {
                        card.push(self.work_rich_text(
                            &release.release_notes,
                            &format!("ota-notes-{ci}-{ri}"),
                        ));
                    }
                    if release.has_download()
                        && (release.sha256.is_some() || release.md5.is_some())
                        && release.size_bytes > 0
                    {
                        let already_staged = u
                            .staged
                            .as_ref()
                            .is_some_and(|pkg| pkg.package.version == release.version);
                        card.push(if u.busy || u.transfer.is_some() || already_staged {
                            self.disabled_button(if already_staged {
                                "已传到手机"
                            } else {
                                "正在处理…"
                            })
                        } else {
                            self.button(
                                format!("ota-download-{ci}-{ri}"),
                                "下载到手机",
                                Action::Updates(UpdateAction::Download(ci, ri)),
                                Btn::Secondary,
                            )
                        });
                    }
                    sections.push(self.panel(card));
                }
            }
        }
        if self.selected.is_some() {
            let mut cleanup = vec![self.between(
                self.vstack(
                    S1,
                    vec![
                        self.heading("OTA 暂存空间"),
                        self.text(
                            "查看和清理此工具传到手机的固件包，可在重启桌面后继续清理。",
                            CAPTION,
                            p.text2,
                        ),
                    ],
                ),
                if u.busy || u.transfer.is_some() {
                    self.disabled_button("管理暂存包")
                } else {
                    self.button(
                        "ota-cleanup-inspect",
                        "管理暂存包",
                        Action::Updates(UpdateAction::PrepareCleanup),
                        Btn::Secondary,
                    )
                },
            )];
            if let Some(usage) = &u.cleanup {
                cleanup.push(self.text(
                    format!(
                        "手机上有 {} 个暂存包，占用 {}。",
                        usage.count,
                        format_bytes(usage.bytes)
                    ),
                    BODY,
                    p.text,
                ));
                if usage.count > 0 {
                    cleanup.push(self.text(
                        "清理前请确认手机官方更新器没有正在安装此包。",
                        CAPTION,
                        p.warning,
                    ));
                    cleanup.push(self.hstack(
                        S2,
                        vec![
                            self.button(
                                "ota-cleanup-confirm",
                                "清理这些暂存包",
                                Action::Updates(UpdateAction::ConfirmCleanup),
                                Btn::Secondary,
                            ),
                            self.button(
                                "ota-cleanup-cancel",
                                "保留",
                                Action::Updates(UpdateAction::CancelCleanup),
                                Btn::Ghost,
                            ),
                        ],
                    ));
                }
            }
            sections.push(self.panel(cleanup));
            sections.push(self.panel(vec![self.between(
                self.vstack(
                    S1,
                    vec![
                        self.heading("手机官方更新器"),
                        self.text("查看系统升级策略、报名资格与安装结果。", CAPTION, p.text2),
                    ],
                ),
                if u.busy || u.transfer.is_some() {
                    self.disabled_button("在手机查看")
                } else {
                    self.button(
                        "ota-phone-open",
                        "在手机查看",
                        Action::Updates(UpdateAction::PhoneOpen),
                        Btn::Secondary,
                    )
                },
            )]));
        }
        self.vstack(S4, sections)
    }

    fn companion_tool_page(&self) -> El {
        let p = self.p();
        let companion = &self.companion_status;
        let mut sections = vec![self.panel(vec![
            self.between(
                self.vstack(
                    S1,
                    vec![
                        self.heading("StepX伴侣"),
                        self.text(
                            if companion.installed {
                                format!("手机版本 {}", companion.version)
                            } else if companion.checked {
                                "尚未安装".into()
                            } else {
                                "等待检测手机状态".into()
                            },
                            CAPTION,
                            p.text2,
                        ),
                    ],
                ),
                self.utility_button(
                    "companion-install",
                    if companion.installed {
                        "更新／修复伴侣"
                    } else {
                        "安装 StepX伴侣"
                    },
                    Action::CompanionInstall,
                    true,
                ),
            ),
            self.text(
                "管理手机伴侣、文件收发与背屏，并分别核验扫码和熄屏服务能力。",
                BODY,
                p.text2,
            ),
            self.text(
                if companion.checked {
                    &companion.message
                } else {
                    "USB 或已配对无线连接均可检测；安装、更新与修复伴侣需要已授权的 USB 连接。"
                },
                CAPTION,
                p.text2,
            ),
            self.wrap(vec![
                self.button(
                    "companion-guide",
                    "首次连接引导",
                    Action::ConnectGuide(Some(if self.selected.is_some() { 2 } else { 0 })),
                    Btn::Secondary,
                ),
                if companion.installed {
                    self.utility_button(
                        "companion-open",
                        "在手机打开伴侣",
                        Action::OpenApp(stepx_companion::companion_app::PACKAGE.into()),
                        false,
                    )
                } else {
                    self.disabled_button("在手机打开伴侣")
                },
            ]),
        ])];
        if let Some((message, error)) = &self.phone_link_notice {
            sections.push(self.panel(vec![
                self.heading("手机文件连接"),
                self.text(message, CAPTION, if *error { p.warning } else { p.success }),
            ]));
        }
        if !companion.installed {
            return self.vstack(S4, sections);
        }
        sections.push(self.panel(vec![
            self.between(self.heading("伴侣系统服务 · Amoo 后台连接"), self.pill_badge(if companion.background_supported == Some(false) { "熄屏待支持" } else { match companion.background_ready { Some(true) => "通道已确认", Some(false) => "待连接确认", None => "待检测" } }, if companion.background_ready == Some(true) { p.success_text } else { p.text2 }, p.surface)),
            self.text(&companion.background_message, BODY, p.text2),
            self.text("手机界面、任务租约和真实连接分别检测。发送任务后按需请求原生短时连接；停止会结束续期，已请求的原生窗口最多约 3 分钟后到期。", CAPTION, p.text3),
            self.wrap(vec![if companion.background_supported == Some(true) { self.utility_button("companion-keep-start", "开启后台连接", Action::CompanionBackground(true), companion.background_enabled != Some(true)) } else { self.disabled_button("熄屏服务待支持") }, self.utility_button("companion-keep-stop", "停止后台连接", Action::CompanionBackground(false), false)]),
        ]));
        sections.push(self.panel(vec![
            self.between(self.heading("文件互传"), self.line("USB / 已配对的无线连接", CAPTION, p.text3)),
            self.text("电脑每次可发送 1–10 个文件，单个文件最多 256 MB。手机在伴侣「文件」页选择文件发送到电脑；传输失败会显示具体原因。", BODY, p.text2),
            self.wrap(vec![self.utility_button("companion-send", "发送文件到手机", Action::CompanionSendFiles, false), self.button("companion-receive", "打开电脑接收文件夹", Action::CompanionReceiveFolder, Btn::Secondary)]),
        ]));
        sections.push(self.panel(vec![
            self.between(self.heading("扫码连接电脑"), self.line(if companion.pairing_ready { "系统配对服务已核验" } else if companion.pairing_permission_granted { "权限已授权 · 服务未核验" } else { "权限尚未就绪" }, CAPTION, if companion.pairing_ready { p.success } else { p.warning })),
            self.text(&companion.pairing_message, BODY, p.text2),
            self.text("扫码服务随伴侣安装，并核验手机实际接收结果；当前只在本次手机开机期间生效。重启手机后，通过 USB 连接后点击「更新／修复伴侣」恢复。手机和电脑需处于同一局域网，真实无线通道验证后才显示连接成功。", CAPTION, p.text3),
            self.button("companion-pair", if companion.pairing_ready { "打开配对二维码" } else { "查看系统无线配对方式" }, Action::ConnectGuide(Some(if companion.pairing_ready { 4 } else { 3 })), Btn::Secondary),
        ]));
        let power_state = |value: Option<bool>| match value {
            Some(true) => "已豁免",
            Some(false) => "未豁免",
            None => "检测失败／未知",
        };
        let mut power = vec![
            self.heading("手机界面 APK · 电源权限"),
            self.between(self.line("Android 电池优化（Doze）", BODY, p.text), self.line(power_state(companion.doze_exempt), CAPTION, if companion.doze_exempt == Some(true) { p.success } else { p.warning })),
            self.between(self.line("厂商后台冻结策略", BODY, p.text), self.line(power_state(companion.oem_freezer_exempt), CAPTION, if companion.oem_freezer_exempt == Some(true) { p.success } else { p.warning })),
            self.text("安装时仅配置 StepX伴侣自身的豁免，保存原状态。电源豁免与系统服务连接能力分别核验；不会修改全局省电策略。", CAPTION, p.text2),
        ];
        if !companion.power_message.is_empty() {
            power.push(self.text(&companion.power_message, CAPTION, p.warning));
        }
        sections.push(self.panel(power));
        let mut permissions = vec![
            self.heading("权限与用途"),
            self.text(
                "安装时授予这版伴侣实际使用的权限，并逐项读取系统状态核验。文件收发使用系统文件选择器，无需全盘存储权限。",
                CAPTION,
                p.text2,
            ),
        ];
        for permission in &companion.permissions {
            permissions.push(self.divider());
            permissions.push(self.between(
                self.vstack(
                    S1,
                    vec![
                        self.line(permission.label, BODY, p.text),
                        self.text(permission.purpose, CAPTION, p.text2),
                    ],
                ),
                self.line(
                    if permission.granted {
                        "已授权"
                    } else {
                        "未授权"
                    },
                    CAPTION,
                    if permission.granted {
                        p.success
                    } else {
                        p.warning
                    },
                ),
            ));
        }
        sections.push(self.panel(permissions));
        self.vstack(S4, sections)
    }

    fn google_tool_page(&self) -> El {
        let p = self.p();
        let g = &self.google_suite;
        let mut rows = vec![
            self.heading("Google Play 安装与检测"),
            self.text(
                if !g.checked {
                    "请先检测手机上已有的 Google 服务。"
                } else {
                    &g.verification_note
                },
                BODY,
                p.text2,
            ),
        ];
        for (title, version) in [
            ("Google 服务框架", &g.gsf_version),
            ("Google Play 服务", &g.gms_version),
            ("Google Play 商店", &g.play_version),
        ] {
            rows.push(self.between(
                self.line(title, BODY, p.text),
                self.line(
                    if !g.checked {
                        "待检测"
                    } else if version.is_empty() {
                        "未检测到"
                    } else {
                        version
                    },
                    CAPTION,
                    p.text2,
                ),
            ));
        }
        rows.push(self.divider());
        rows.push(self.text("安装 Google Play 时会同时安装 StepX伴侣，用于扫码、文件互传、背屏与 Amoo 后台连接。仅适配已验证的 StepX 固件；保留已有 Google 包。", CAPTION, p.text2));
        let mut buttons = vec![];
        if g.checked
            && (g.installed
                || (g.gsf_version.is_empty()
                    && g.gms_version.is_empty()
                    && g.play_version.is_empty()))
            && self.tool_confirmation.is_none()
        {
            buttons.push(self.utility_button(
                "tool-google-install",
                if g.installed {
                    "完善 Google Play 和伴侣"
                } else {
                    "安装 Google Play 和伴侣"
                },
                Action::PrepareUtility(Box::new(Action::GoogleSuiteInstall)),
                true,
            ));
        }
        rows.push(self.between(
            self.text(
                if self.companion_status.installed {
                    format!("StepX伴侣 · {}", self.companion_status.version)
                } else {
                    "StepX伴侣 · 尚未安装或检测".into()
                },
                BODY,
                p.text,
            ),
            self.button(
                "google-companion",
                "管理伴侣",
                Action::OpenTool(Some(ToolPage::Companion)),
                Btn::Secondary,
            ),
        ));
        if g.checked && !g.play_version.is_empty() {
            buttons.push(self.utility_button(
                "tool-google-open",
                "在手机打开 Play",
                Action::OpenApp("com.android.vending".into()),
                self.tool_confirmation.is_none(),
            ));
        }
        if !buttons.is_empty() {
            rows.push(self.hstack(S2, buttons));
        }
        let mut compatibility = vec![
            self.between(
                self.heading("Google Play 兼容补丁"),
                self.line(
                    if g.compatibility_active {
                        "已应用"
                    } else if g.compatibility_checked && g.compatibility_supported {
                        "可应用"
                    } else {
                        "待适配 / 检测"
                    },
                    CAPTION,
                    if g.compatibility_active {
                        p.success
                    } else {
                        p.text3
                    },
                ),
            ),
            self.text(&g.compatibility_note, BODY, p.text2),
            self.text(
                "用于已适配设备的商店浏览和手动下载安装；应用前核对组件版本并备份，可在这里撤销。检测只读取补丁配置。",
                CAPTION,
                p.text2,
            ),
        ];
        if g.checked && g.installed && self.tool_confirmation.is_none() {
            let mut actions = vec![];
            if g.compatibility_checked && g.compatibility_supported && !g.compatibility_active {
                actions.push(self.utility_button(
                    "tool-google-compat-apply",
                    "应用兼容补丁",
                    Action::PrepareUtility(Box::new(Action::GoogleSuiteMaintain)),
                    true,
                ));
            }
            if g.compatibility_can_rollback {
                actions.push(self.utility_button(
                    "tool-google-compat-rollback",
                    "撤销兼容补丁",
                    Action::PrepareUtility(Box::new(Action::GoogleSuiteRollback)),
                    false,
                ));
            }
            if !actions.is_empty() {
                compatibility.push(self.hstack(S2, actions));
            }
        }
        let mut certification = vec![
            self.heading("Google 官方认证"),
            self.text(&g.certification_note, BODY, p.text2),
            self.text(
                "兼容补丁与 Google 官方认证分别检测。商店下载可用不代表支付、DRM 或 Play Integrity 已通过。",
                CAPTION,
                p.text3,
            ),
        ];
        if !g.boot_security_note.is_empty() {
            certification.push(self.text(&g.boot_security_note, CAPTION, p.text2));
        }
        if !g.certification_catalog.is_empty() {
            certification.push(self.text(&g.certification_catalog, CAPTION, p.text2));
        }
        certification.push(self.hstack(S2, vec![
            self.button("google-cert-help", "官方排查说明", Action::StudioOpenUrl("https://support.google.com/googleplay/answer/7165974?hl=zh-Hans".into()), Btn::Secondary),
            self.button("google-cert-register", "官方设备登记", Action::StudioOpenUrl("https://www.google.com/android/uncertified/".into()), Btn::Secondary),
        ]));
        self.vstack(
            S4,
            vec![self.panel(rows), self.panel(compatibility), self.panel(certification)],
        )
    }
}

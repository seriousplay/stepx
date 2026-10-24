use super::*;
use std::sync::atomic::Ordering;
use stepx_companion::{studio_delivery, studio_lab};

#[derive(Clone, Debug)]
pub(super) enum StudioLabAction {
    Protocol(u8),
    Save,
    Test,
    Models,
    Model(String),
    ClearKey,
    Start,
    Stop,
    Fresh,
    Template(usize),
    Select(usize),
    HistoryPage(usize),
    Iterate,
    Preview,
    OpenFolder,
    Refresh,
    ToggleReport,
    CloudModels,
    CloudModel(String),
    DeliverPhone,
    SyncCloud,
    OpenCloud,
}
impl StudioLabAction {
    fn available(&self, busy: bool, running: bool) -> bool {
        use StudioLabAction::*;
        if running
            && matches!(
                self,
                Start | Fresh | Template(_) | Iterate | Save | Test | Models | ClearKey
            )
        {
            return false;
        }
        !busy
            || matches!(
                self,
                Stop | Select(_)
                    | HistoryPage(_)
                    | ToggleReport
                    | Protocol(_)
                    | Model(_)
                    | CloudModel(_)
            )
    }
}
pub(super) enum StudioLabOutcome {
    Loaded(Result<(studio_lab::PublicConfig, Vec<studio_lab::LabArtifact>), String>),
    Config(Result<studio_lab::PublicConfig, String>),
    Test(
        Result<
            (
                studio_lab::PublicConfig,
                Result<studio_lab::ModelTest, String>,
            ),
            String,
        >,
    ),
    Models(Result<(studio_lab::PublicConfig, Result<Vec<String>, String>), String>),
    Artifacts(Result<Vec<studio_lab::LabArtifact>, String>),
    Report(String, Result<serde_json::Value, String>),
    Note(Result<String, String>),
    CloudModels(
        String,
        String,
        Result<Vec<studio_delivery::StudioModel>, String>,
    ),
    Phone(String, Option<String>, Result<String, String>),
    Cloud(
        String,
        String,
        String,
        Result<studio_delivery::CloudReceipt, String>,
    ),
}
#[derive(Default)]
pub(super) struct CloudFeedback {
    pub note: Option<String>,
    pub receipt: Option<studio_delivery::CloudReceipt>,
    pub recovery_url: Option<String>,
    pub error: Option<String>,
}
#[derive(Default)]
pub(super) struct StudioLabUiState {
    pub loaded: bool,
    pub config: studio_lab::PublicConfig,
    pub protocol: u8,
    pub models: Vec<String>,
    pub model_note: Option<String>,
    pub pending_config: Option<(String, u8, String)>,
    pub artifacts: Vec<studio_lab::LabArtifact>,
    pub history_page: usize,
    pub selected: Option<usize>,
    pub handle: Option<studio_lab::LabHandle>,
    pub events: Vec<String>,
    pub status: String,
    pub error: Option<String>,
    pub source: Option<PathBuf>,
    pub source_name: Option<String>,
    pub prompt_scroll: ClippedScrollStateHandle,
    pub report: Option<(String, serde_json::Value)>,
    pub report_loading: Option<String>,
    pub report_error: Option<String>,
    pub report_open: bool,
    pub cloud_models: Vec<studio_delivery::StudioModel>,
    pub cloud_model: Option<String>,
    pub cloud_scope: Option<String>,
    pub phone_feedback: std::collections::HashMap<(String, String), Result<String, String>>,
    pub cloud_feedback: std::collections::HashMap<(String, String, String), CloudFeedback>,
}
pub(super) const LAB_TEMPLATES: [(&str,&str);3] = [
    ("做个小游戏","制作一款适合手机竖屏的贪吃蛇小游戏。清晰的开始、暂停、重开和触控方向按钮；实时积分，碰撞结束；键盘方向键也可操作。离线可用，不依赖网络。请真实测试开始、暂停、方向控制与重新开始，验证分数和游戏状态会变化。"),
    ("随手记账","制作适合手机竖屏的离线记账应用。金额、分类和备注输入，新增、修改、删除，实时总计，重开后保留本地数据。首次没有伪造账单。请真实测试新增两笔、统计总额和删除后的总额变化。"),
    ("专注计时","制作手机和电脑都能用的离线专注计时器。可选时长，开始、暂停、继续、重置，清楚显示剩余时间；支持短时测试模式。请验证倒计时实际变化、暂停时保持、重置恢复，以及触屏布局。"),
];

impl Companion {
    pub(super) fn studio_lab_phone_note(&self) -> Option<&Result<String, String>> {
        self.studio_lab.phone_feedback.get(&(
            self.selected.as_ref()?.serial.clone(),
            self.studio_lab_selected()?.id.clone(),
        ))
    }
    pub(super) fn studio_lab_cloud_feedback(&self) -> Option<&CloudFeedback> {
        let web = self.studio.web.as_ref().filter(|w| w.authorized)?;
        self.studio_lab.cloud_feedback.get(&(
            self.selected.as_ref()?.serial.clone(),
            web.scope.clone(),
            self.studio_lab_selected()?.id.clone(),
        ))
    }
    pub(super) fn studio_lab_action_allowed(&self, action: &StudioLabAction) -> bool {
        action.available(self.busy, self.studio_lab.handle.is_some())
    }
    pub(super) fn studio_lab_config_dirty(&self) -> bool {
        self.fields[F_LAB_ENDPOINT]
            .text
            .trim()
            .trim_end_matches('/')
            != self.studio_lab.config.endpoint.trim_end_matches('/')
            || self.fields[F_LAB_MODEL].text.trim() != self.studio_lab.config.model
            || !self.fields[F_LAB_KEY].text.is_empty()
            || (if self.studio_lab.protocol == 1 {
                "anthropic"
            } else {
                "openai"
            }) != self.studio_lab.config.protocol
    }
    pub(super) fn studio_lab_selected(&self) -> Option<&studio_lab::LabArtifact> {
        self.studio_lab
            .selected
            .and_then(|i| self.studio_lab.artifacts.get(i))
    }
    pub(super) fn tick_studio_lab(&mut self) -> bool {
        let mut changed = false;
        let mut events = Vec::new();
        let mut disconnected = false;
        if let Some(handle) = &self.studio_lab.handle {
            loop {
                match handle.events.try_recv() {
                    Ok(event) => events.push(event),
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        disconnected = true;
                        break;
                    }
                }
            }
        }
        for event in events {
            changed = true;
            match event {
                studio_lab::LabEvent::Phone(event) => match event {
                    stepx_companion::work::WorkEvent::SessionReady(key) => {
                        self.workspace_bind_session(&key);
                        if !self.amoo_sessions.iter().any(|s| s.session_key == key) { self.apply(Outcome::AmooCreated(key)); }
                    }
                    stepx_companion::work::WorkEvent::ModelResponse { data, key, run_id, elapsed_ms } => {
                        if self.amoo_ui.account_scope == data.account_scope { self.record_work_response(&data, &key, &run_id, elapsed_ms); }
                    }
                    _ => {}
                },
                studio_lab::LabEvent::Status(note) => {
                    self.studio_lab.status = note.clone();
                    self.studio_lab.events.push(note);
                    if self.studio_lab.events.len() > 40 {
                        self.studio_lab.events.remove(0);
                    }
                }
                studio_lab::LabEvent::Artifact(artifact) => {
                    self.studio_lab_receive_artifact(artifact);
                }
                studio_lab::LabEvent::Complete(artifact) => {
                    self.studio_lab_receive_artifact(artifact);
                    // Only the terminal event releases the handle: dropping a live
                    // handle cancels its worker, even when an earlier round passed.
                    self.studio_lab.handle = None;
                    self.studio_lab.status = "作品已完成，本机自动检查通过".into();
                    self.workspace_finish(stepx_companion::amoo_workspace::Phase::Done,self.studio_lab.status.clone());
                    self.studio_lab.error = None;
                }
                studio_lab::LabEvent::Failed(note) => {
                    self.workspace_finish(stepx_companion::amoo_workspace::Phase::Failed,note.clone());
                    self.studio_lab.handle = None;
                    self.studio_lab.error = Some(note.clone());
                    self.studio_lab.status = note;
                }
                studio_lab::LabEvent::Cancelled => {
                    self.workspace_finish(stepx_companion::amoo_workspace::Phase::Stopped,"已停止，已生成版本保留在电脑".into());
                    self.studio_lab.handle = None;
                    self.studio_lab.status = "已停止，已生成的版本保留在本机".into();
                }
            }
        }
        if disconnected && self.studio_lab.handle.is_some() {
            self.studio_lab.handle = None;
            self.studio_lab.error =
                Some("创作进程已结束，未收到完成结果；已保存的版本仍可检查与继续改进".into());
            self.studio_lab.status = "任务未完成".into();
            self.workspace_finish(stepx_companion::amoo_workspace::Phase::Failed,self.studio_lab.error.clone().unwrap_or_default());
            changed = true;
        }
        if !matches!(self.page,Page::Studio|Page::Amoo) || self.busy || self.demo_mode {
            return changed;
        }
        if !self.studio_lab.loaded {
            self.studio_lab.loaded = true;
            self.submit("正在读取 Studio 工作台…", |_| {
                Ok(Outcome::StudioLab(StudioLabOutcome::Loaded(
                    (|| Ok((studio_lab::load_config()?, studio_lab::list_artifacts()?)))()
                        .map_err(|e: anyhow::Error| e.to_string()),
                )))
            });
            return true;
        }
        if let Some(artifact) = self.studio_lab_selected().cloned() {
            if self
                .studio_lab
                .report
                .as_ref()
                .is_none_or(|(id, _)| id != &artifact.id)
                && self.studio_lab.report_loading.as_ref() != Some(&artifact.id)
            {
                self.studio_lab.report_loading = Some(artifact.id.clone());
                self.submit("正在读取作品验证结果…", move |_| {
                    Ok(Outcome::StudioLab(StudioLabOutcome::Report(
                        artifact.id.clone(),
                        studio_lab::read_report(&artifact).map_err(|e| e.to_string()),
                    )))
                });
                return true;
            }
        }
        changed
    }
    fn studio_lab_receive_artifact(&mut self, artifact: studio_lab::LabArtifact) {
        self.workspace_output(stepx_companion::amoo_workspace::Output{path:artifact.html_path.clone(),name:artifact.name.clone(),kind:if artifact.passed{"应用 · 已检查"}else{"应用 · 待修复"}.into(),lab_id:Some(artifact.id.clone())});
        self.studio_lab.status = artifact.summary.clone();
        self.studio_lab.artifacts.retain(|a| a.id != artifact.id);
        self.studio_lab.artifacts.insert(0, artifact);
        self.studio_lab.selected = Some(0);
        self.studio_lab.report = None;
        self.studio_lab.report_loading = None;
        self.studio_lab.report_error = None;
    }
    fn studio_lab_store_action(&mut self, kind: u8) {
        let endpoint = self.fields[F_LAB_ENDPOINT].text.clone();
        let model = self.fields[F_LAB_MODEL].text.clone();
        let key = self.fields[F_LAB_KEY].text.clone();
        let protocol = if self.studio_lab.protocol == 1 {
            "anthropic"
        } else {
            "openai"
        };
        self.studio_lab.model_note = None;
        self.studio_lab.pending_config =
            Some((endpoint.clone(), self.studio_lab.protocol, model.clone()));
        self.submit(
            match kind {
                1 => "正在验证模型实际响应…",
                2 => "正在读取模型目录…",
                _ => "正在保存模型设置…",
            },
            move |_| {
                let saved = studio_lab::save_config(&endpoint, protocol, &model, &key, kind == 3);
                let result = match kind {
                    1 => StudioLabOutcome::Test(
                        saved
                            .map(|c| (c, studio_lab::test_connection().map_err(|e| e.to_string())))
                            .map_err(|e| e.to_string()),
                    ),
                    2 => StudioLabOutcome::Models(
                        saved
                            .map(|c| (c, studio_lab::list_models().map_err(|e| e.to_string())))
                            .map_err(|e| e.to_string()),
                    ),
                    _ => StudioLabOutcome::Config(saved.map_err(|e| e.to_string())),
                };
                Ok(Outcome::StudioLab(result))
            },
        );
    }
    pub(super) fn handle_studio_lab_action(
        &mut self,
        action: &StudioLabAction,
        ctx: &mut ViewContext<Self>,
    ) {
        use StudioLabAction as A;
        if !self.studio_lab_action_allowed(action) {
            return;
        }
        self.studio_lab.error = None;
        self.error = false;
        match action {
            A::Protocol(v) => self.studio_lab.protocol = (*v).min(1),
            A::Save => self.studio_lab_store_action(0),
            A::Test => self.studio_lab_store_action(1),
            A::Models => self.studio_lab_store_action(2),
            A::ClearKey => self.studio_lab_store_action(3),
            A::Model(name) => self.fields[F_LAB_MODEL].set(name),
            A::Start => {
                if self.studio_lab.handle.is_some() {
                    return;
                }
                let use_phone = !self.studio_lab.config.has_key;
                if !use_phone && (self.studio_lab.config.model.is_empty() || self.studio_lab_config_dirty()) {
                    self.studio_lab.error =
                        Some("请先在连接设置中保存自己的模型 API 与模型名称".into());
                    self.studio.tab = 3;
                    return;
                }
                let prompt = self.fields[F_LAB_PROMPT].text.trim().to_string();
                if prompt.is_empty() {
                    self.focus_field(F_LAB_PROMPT, ctx);
                    return;
                }
                self.studio_lab.events.clear();
                self.studio_lab.status = "正在连接模型，开始创作…".into();
                let phone = if use_phone {
                    match self.selected.clone().zip(Adb::discover().ok()) {
                        Some((device, adb)) => Some((adb, device)),
                        None => { self.studio_lab.error = Some("请连接手机使用 Amoo，或在模型设置保存自己的 API".into()); return; }
                    }
                } else { None };
                self.studio_lab.handle = Some(studio_lab::start_with_phone(studio_lab::LabRequest {
                    prompt,
                    source: self.studio_lab.source.clone(),
                }, phone));
            }
            A::Stop => {
                if let Some(handle) = &self.studio_lab.handle {
                    handle.cancel.store(true, Ordering::SeqCst);
                    self.studio_lab.status = "正在停止任务…".into();
                }
            }
            A::HistoryPage(page) => {
                self.studio_lab.history_page =
                    (*page).min(self.studio_lab.artifacts.len().saturating_sub(1) / 6);
            }
            A::Fresh => {
                self.studio.tab = 2;
                self.studio_lab.events.clear();
                self.studio_lab.selected = None;
                self.studio_lab.report = None;
                self.studio_lab.report_loading = None;
                self.studio_lab.report_error = None;
                if self.studio_lab.handle.is_some() {
                    return;
                }
                self.studio_lab.source = None;
                self.studio_lab.source_name = None;
                self.fields[F_LAB_PROMPT].clear();
                self.studio_lab.status.clear();
                self.focus_field(F_LAB_PROMPT, ctx);
            }
            A::Template(i) => {
                if self.studio_lab.handle.is_some() {
                    return;
                }
                if let Some((_, prompt)) = LAB_TEMPLATES.get(*i) {
                    self.studio_lab.source = None;
                    self.studio_lab.source_name = None;
                    self.fields[F_LAB_PROMPT].set(prompt);
                    self.focus_field(F_LAB_PROMPT, ctx);
                }
            }
            A::Select(i) => {
                if *i < self.studio_lab.artifacts.len() {
                    self.studio_lab.selected = Some(*i);
                    self.studio_lab.report = None;
                    self.studio_lab.report_loading = None;
                    self.studio_lab.report_error = None;
                }
            }
            A::Iterate => {
                if let Some(a) = self.studio_lab_selected().cloned() {
                    self.studio_lab.source = Some(a.html_path);
                    self.studio_lab.source_name = Some(a.name);
                    self.fields[F_LAB_PROMPT].clear();
                    self.focus_field(F_LAB_PROMPT, ctx);
                }
            }
            A::ToggleReport => self.studio_lab.report_open = !self.studio_lab.report_open,
            A::Refresh => self.submit("正在读取本机作品…", |_| {
                Ok(Outcome::StudioLab(StudioLabOutcome::Artifacts(
                    studio_lab::list_artifacts().map_err(|e| e.to_string()),
                )))
            }),
            A::Preview => {
                if let Some(a) = self.studio_lab_selected().cloned() {
                    self.submit("正在打开隔离预览…", move |_| {
                        Ok(Outcome::StudioLab(StudioLabOutcome::Note(
                            studio_lab::open_preview(&a)
                                .map(|_| "已打开独立离线预览，可实际操作检查".into())
                                .map_err(|e| e.to_string()),
                        )))
                    });
                }
            }
            A::OpenFolder => {
                if let Some(a) = self.studio_lab_selected().cloned() {
                    self.submit("正在打开作品目录…", move |_| {
                        let cmd = if cfg!(target_os = "macos") {
                            "open"
                        } else if cfg!(windows) {
                            "explorer"
                        } else {
                            "xdg-open"
                        };
                        let result = std::process::Command::new(cmd)
                            .arg(a.root)
                            .spawn()
                            .map(|mut child| {
                                std::thread::spawn(move || {
                                    let _ = child.wait();
                                });
                                "已打开作品目录".into()
                            })
                            .map_err(|_| "无法打开作品目录".into());
                        Ok(Outcome::StudioLab(StudioLabOutcome::Note(result)))
                    });
                }
            }
            A::CloudModels => {
                let Some(web) = self.studio.web.as_ref().filter(|w| w.authorized) else {
                    self.studio_lab.error = Some("请先连接 Studio 官网并同步项目".into());
                    self.studio.tab = 3;
                    return;
                };
                let scope = web.scope.clone();
                self.with_device("正在读取官网模型…", move |adb, d| {
                    Ok(Outcome::StudioLab(StudioLabOutcome::CloudModels(
                        d.serial.clone(),
                        scope.clone(),
                        studio_delivery::cloud_models(adb, &d, &scope).map_err(|e| e.to_string()),
                    )))
                });
            }
            A::CloudModel(model) => {
                if self.studio_lab.cloud_models.iter().any(|m| &m.id == model) {
                    self.studio_lab.cloud_model = Some(model.clone());
                }
            }
            A::DeliverPhone => {
                let Some(a) = self.studio_lab_selected().filter(|a| a.passed).cloned() else {
                    self.studio_lab.error = Some("请先选择自动检查通过的作品".into());
                    return;
                };
                self.with_device(
                    "正在发送并验证手机离线作品…",
                    move |adb, d| {
                        let result = (|| {
                            let html = studio_lab::verified_html(&a)?;
                            let r = studio_delivery::install_local(adb, &d, &a.name, &html)?;
                            studio_delivery::open_local(adb, &d, &r.id)?;
                            Ok(format!(
                                "{} 已发送到手机并完成页面载入；可在伴侣作品库离线打开",
                                a.name
                            ))
                        })();
                        Ok(Outcome::StudioLab(StudioLabOutcome::Phone(
                            d.serial,
                            Some(a.id),
                            result.map_err(|e: anyhow::Error| e.to_string()),
                        )))
                    },
                );
            }
            A::SyncCloud => {
                if self
                    .studio_lab_cloud_feedback()
                    .is_some_and(|f| f.receipt.is_some() || f.recovery_url.is_some())
                {
                    self.handle_studio_lab_action(&A::OpenCloud, ctx);
                    return;
                }
                let Some(a) = self.studio_lab_selected().filter(|a| a.passed).cloned() else {
                    self.studio_lab.error = Some("请先选择自动检查通过的作品".into());
                    return;
                };
                let Some(web) = self.studio.web.as_ref().filter(|w| w.authorized) else {
                    self.studio_lab.error = Some("请先连接官网".into());
                    return;
                };
                let scope = web.scope.clone();
                let Some(model) = self
                    .studio_lab
                    .cloud_model
                    .clone()
                    .filter(|_| self.studio_lab.cloud_scope.as_ref() == Some(&scope))
                else {
                    self.studio_lab.error = Some("请先读取并选择官网模型".into());
                    return;
                };
                self.with_device(
                    "正在新建官网项目并回读验证源码…",
                    move |adb, d| {
                        let result = (|| {
                            let html = studio_lab::verified_html(&a)?;
                            studio_delivery::cloud_sync_new(adb, &d, &scope, &a.name, &html, &model)
                        })();
                        Ok(Outcome::StudioLab(StudioLabOutcome::Cloud(
                            d.serial,
                            scope,
                            a.id,
                            result.map_err(|e: anyhow::Error| format!("{e:#}")),
                        )))
                    },
                );
            }
            A::OpenCloud => {
                if let Some(url) = self.studio_lab_cloud_feedback().and_then(|f| {
                    f.receipt
                        .as_ref()
                        .map(|r| r.editor_url.clone())
                        .or_else(|| f.recovery_url.clone())
                }) {
                    self.with_device("正在连接官网编辑器…", move |adb, d| {
                        Ok(Outcome::StudioLab(StudioLabOutcome::Phone(
                            d.serial.clone(),
                            None,
                            stepx_companion::studio::open_editor(adb, &d, &url)
                                .map_err(|e| e.to_string()),
                        )))
                    });
                }
            }
        }
    }
    fn studio_lab_apply_saved_config(&mut self, config: studio_lab::PublicConfig) {
        self.fields[F_LAB_KEY].clear();
        self.record_notice("Studio 模型设置已保存","后续任务将使用已确认的模型服务；密钥仅保存在本机。");
        if self
            .studio_lab
            .pending_config
            .take()
            .is_some_and(|(endpoint, protocol, model)| {
                self.fields[F_LAB_ENDPOINT].text == endpoint
                    && self.studio_lab.protocol == protocol
                    && self.fields[F_LAB_MODEL].text == model
            })
        {
            self.fields[F_LAB_ENDPOINT].set(&config.endpoint);
            self.fields[F_LAB_MODEL].set(&config.model);
            self.studio_lab.protocol = u8::from(config.protocol == "anthropic");
        }
        self.studio_lab.config = config;
    }
    pub(super) fn apply_studio_lab(&mut self, outcome: StudioLabOutcome) {
        use StudioLabOutcome as O;
        let mut error = None;
        match outcome {
            O::Loaded(result) => match result {
                Ok((config, artifacts)) => {
                    self.fields[F_LAB_ENDPOINT].set(&config.endpoint);
                    self.fields[F_LAB_MODEL].set(&config.model);
                    self.studio_lab.protocol = u8::from(config.protocol == "anthropic");
                    self.studio_lab.config = config;
                    self.studio_lab.artifacts = artifacts;
                    self.studio_lab.selected = (!self.studio_lab.artifacts.is_empty()).then_some(0);
                }
                Err(e) => error = Some(e),
            },
            O::Config(result) => match result {
                Ok(config) => {
                    self.studio_lab_apply_saved_config(config);
                    self.studio_lab.model_note =
                        Some("模型设置已保存，密钥仅存于本机私有配置".into());
                }
                Err(e) => error = Some(e),
            },
            O::Test(result) => match result {
                Ok((config, result)) => {
                    self.studio_lab_apply_saved_config(config);
                    match result {
                        Ok(test) => {
                            self.studio_lab.model_note = Some(format!(
                                "实际请求通过 · {} · {:.1} 秒",
                                test.model,
                                test.elapsed_ms as f64 / 1000.
                            ))
                        }
                        Err(e) => error = Some(e),
                    }
                }
                Err(e) => error = Some(e),
            },
            O::Models(result) => match result {
                Ok((config, result)) => {
                    self.studio_lab_apply_saved_config(config);
                    match result {
                        Ok(models) => {
                            self.studio_lab.model_note = Some(format!(
                                "已读取 {} 个模型；请按模型名筛选并选择，使用前可测试连接",
                                models.len()
                            ));
                            self.studio_lab.models = models;
                        }
                        Err(e) => error = Some(e),
                    }
                }
                Err(e) => error = Some(e),
            },
            O::Artifacts(result) => match result {
                Ok(artifacts) => {
                    let selected = self.studio_lab_selected().map(|a| a.id.clone());
                    self.studio_lab.artifacts = artifacts;
                    self.studio_lab.selected = selected
                        .and_then(|id| self.studio_lab.artifacts.iter().position(|a| a.id == id))
                        .or_else(|| (!self.studio_lab.artifacts.is_empty()).then_some(0));
                    self.studio_lab.report = None;
                    self.studio_lab.report_loading = None;
                    self.studio_lab.report_error = None;
                }
                Err(e) => error = Some(e),
            },
            O::Report(id, result) => {
                if self.studio_lab_selected().is_some_and(|a| a.id == id) {
                    match result {
                        Ok(report) => {
                            self.studio_lab.report = Some((id, report));
                            self.studio_lab.report_error = None;
                        }
                        Err(e) => {
                            self.studio_lab.report_error = Some(e.clone());
                            error = Some(e);
                        }
                    }
                }
            }
            O::Note(result) => match result {
                Ok(note) => {
                    self.status = note.clone();
                }
                Err(e) => error = Some(e),
            },
            O::Phone(serial, artifact_id, result) => {
                let current_device = self.selected.as_ref().is_some_and(|d| d.serial == serial);
                let visible = current_device
                    && artifact_id
                        .as_ref()
                        .is_none_or(|id| self.studio_lab_selected().is_some_and(|a| &a.id == id));
                if visible {
                    match &result {
                        Ok(note) => self.status = note.clone(),
                        Err(e) => error = Some(e.clone()),
                    }
                }
                if let Some(id) = artifact_id {
                    self.workspace_app_receipt(&id,&serial,None,result.clone().unwrap_or_else(|e|format!("手机交付未完成：{e}")),result.is_ok(),None);
                    self.studio_lab.phone_feedback.insert((serial, id), result);
                }
            }
            O::CloudModels(serial, scope, result) => {
                if self.selected.as_ref().is_none_or(|d| d.serial != serial)
                    || self.studio.web.as_ref().is_none_or(|w| w.scope != scope)
                {
                    return;
                }
                match result {
                    Ok(models) => {
                        self.studio_lab.cloud_model = models
                            .iter()
                            .find(|m| m.is_default)
                            .or_else(|| models.first())
                            .map(|m| m.id.clone());
                        self.studio_lab.cloud_models = models;
                        self.studio_lab.cloud_scope = Some(scope);
                    }
                    Err(e) => error = Some(e),
                }
            }
            O::Cloud(serial, scope, artifact_id, result) => {
                let visible = self.selected.as_ref().is_some_and(|d| d.serial == serial)
                    && self
                        .studio
                        .web
                        .as_ref()
                        .is_some_and(|w| w.authorized && w.scope == scope)
                    && self
                        .studio_lab_selected()
                        .is_some_and(|a| a.id == artifact_id);
                // Retain an issued write's receipt even if selection changes while it runs.
                // Every result is bound to the device, account and immutable artifact ID.
                let feedback = match result {
                    Ok(receipt) => CloudFeedback {
                        note: Some(format!(
                            "源码已同步并保存版本，已核对 {} 个文件；官网测试与发布另行完成",
                            receipt.files.len()
                        )),
                        receipt: Some(receipt),
                        ..Default::default()
                    },
                    Err(e) => {
                        if visible {
                            error = Some(e.clone());
                        }
                        CloudFeedback {
                            recovery_url: created_editor_url(&e),
                            error: Some(e),
                            ..Default::default()
                        }
                    }
                };
                self.workspace_app_receipt(&artifact_id,&serial,Some(scope.clone()),feedback.note.clone().or(feedback.error.clone()).unwrap_or_default(),feedback.receipt.is_some(),feedback.receipt.as_ref().map(|r|r.editor_url.clone()).or(feedback.recovery_url.clone()));
                self.studio_lab.cloud_feedback.insert((serial, scope, artifact_id), feedback);
                self.studio.cloud_checked = None;
            }
        }
        if let Some(error) = error {
            self.status = error.clone();
            self.error = true;
            self.studio_lab.error = Some(error);
        }
    }
}

// Only a validated official editor route may become a recovery action. Never
// offer retry-as-new after creation succeeded but source upload failed.
fn created_editor_url(error: &str) -> Option<String> {
    let (_, tail) = error.split_once("项目已创建，可继续编辑：")?;
    let url = tail.split_whitespace().next()?.trim_end_matches(':');
    stepx_companion::studio::parse_link(url)
        .ok()
        .filter(|(_, kind)| *kind == stepx_companion::studio::LinkKind::Project)
        .map(|(url, _)| url)
}
#[cfg(test)]
mod tests {
    use super::created_editor_url;
    #[test]
    fn recovery_link_is_only_an_official_project_route() {
        let url = "https://studio.stepx.com/builder/app/p_1234567890abcdef";
        assert_eq!(
            created_editor_url(&format!("项目已创建，可继续编辑：{url}: 上传失败")),
            Some(url.into())
        );
        assert!(created_editor_url(
            "项目已创建，可继续编辑：https://evil.example/builder/app/p_1234567890abcdef: error"
        )
        .is_none());
        assert!(created_editor_url("HTTP 403").is_none());
    }
}

#[cfg(test)]
mod interaction_tests {
    use super::*;
    #[test]
    fn running_creation_cannot_be_reset_or_reconfigured() {
        for action in [
            StudioLabAction::Start,
            StudioLabAction::Fresh,
            StudioLabAction::Template(0),
            StudioLabAction::Iterate,
            StudioLabAction::Save,
            StudioLabAction::ClearKey,
        ] {
            assert!(!action.available(false, true), "{action:?}");
            assert!(action.available(false, false), "{action:?}");
        }
        assert!(StudioLabAction::Stop.available(true, true));
        assert!(StudioLabAction::Select(1).available(true, true));
        assert!(!StudioLabAction::DeliverPhone.available(true, false));
        assert!(!StudioLabAction::SyncCloud.available(true, false));
    }
}

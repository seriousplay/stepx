//! One visible task, composer and result collection; existing executors remain internal.
use super::*;
use anyhow::Context;
use facloud_ui::TypedActionView;
use stepx_companion::amoo_workspace::{
    self as model, Executor, Intent, Journal, Output, Phase, Step, Task, Turn,
};

#[derive(Clone, Debug)]
pub(super) enum WorkspaceAction {
    New,
    History,
    Select(String),
    PhoneSession(usize),
    Suggest(u8),
    Prepare,
    Confirm,
    EditPlan,
    Intent(Intent),
    Menu(u8),
    ClosePanel,
    ChooseFolder,
    Preview(usize),
    Retry,
    Stop,
    Deliver(bool),
    ConfirmDelivery,
    CancelDelivery,
    ToggleOpenPhone,
    Settings,
    AddOutput(usize),
    OpenProject(usize),
    SelectLab(String),
}
#[derive(Clone)]
pub(super) struct Proposal {
    pub text: String,
    pub files: Vec<PathBuf>,
    pub steps: Vec<Executor>,
}
#[derive(Clone)]
pub(super) struct ActiveRun {
    pub task: String,
    pub step: usize,
    pub executor: Executor,
    pub accepted: bool,
}
struct ImageDelivery {
    task: String,
    scope: Option<String>,
    note: String,
    receiver: mpsc::Receiver<Result<Vec<(String, PathBuf)>, String>>,
}
pub(super) struct WorkspaceUi {
    pub journal: Journal,
    pub active: Option<String>,
    pub history: bool,
    pub panel: u8,
    pub viewport: Vector2F,
    pub intent: Intent,
    pub proposal: Option<Proposal>,
    pub running: Option<ActiveRun>,
    pub next: bool,
    image_delivery: Option<ImageDelivery>,
    pub selected_output: Option<usize>,
    pub delivery: Option<bool>,
    delivery_target: Option<(String, PathBuf, Option<String>, Option<String>)>,
    pub open_phone: bool,
    pub error: Option<String>,
    pub preview_scroll: ClippedScrollStateHandle,
    pub panel_entered: Instant,
    writer: Option<mpsc::Sender<Journal>>,
    writer_thread: Option<std::thread::JoinHandle<()>>,
    last_saved: Instant,
    last_draft: (String, Vec<PathBuf>),
}
impl Default for WorkspaceUi {
    fn default() -> Self {
        let path = stepx_companion::resources::user_state().join("amoo-workspace/tasks.json");
        let demo = std::env::args().any(|a| a == "--demo");
        let (journal, error) = match if demo {
            Ok(Journal::default())
        } else {
            Journal::load(&path)
        } {
            Ok(v) => (v, None),
            Err(e) => (Journal::default(), Some(e.to_string())),
        };
        let mut writer_thread = None;
        let writer = if error.is_none() && !demo {
            let (tx, rx) = mpsc::channel::<Journal>();
            writer_thread = Some(std::thread::spawn(move || {
                while let Ok(mut journal) = rx.recv() {
                    while let Ok(newer) = rx.recv_timeout(Duration::from_millis(300)) {
                        journal = newer;
                    }
                    let _ = journal.save(&path);
                }
            }));
            Some(tx)
        } else {
            None
        };
        Self {
            journal,
            active: None,
            history: false,
            panel: 0,
            viewport: Vector2F::new(1320., 860.),
            intent: Intent::Chat,
            proposal: None,
            running: None,
            next: false,
            image_delivery: None,
            selected_output: None,
            delivery: None,
            delivery_target: None,
            open_phone: false,
            error,
            preview_scroll: Default::default(),
            panel_entered: Instant::now(),
            writer,
            writer_thread,
            last_saved: Instant::now(),
            last_draft: Default::default(),
        }
    }
}
impl Companion {
    pub(super) fn workspace_task(&self) -> Option<&Task> {
        let id = self.workspace.active.as_ref()?;
        self.workspace.journal.tasks.iter().find(|t| &t.id == id)
    }
    fn workspace_task_mut(&mut self) -> Option<&mut Task> {
        let id = self.workspace.active.as_ref()?;
        self.workspace
            .journal
            .tasks
            .iter_mut()
            .find(|t| &t.id == id)
    }
    pub(super) fn workspace_visible(&self, t: &Task) -> bool {
        if self.selected.is_none() {
            return true;
        }
        t.visible(
            self.selected.as_ref().map(|d| d.serial.as_str()),
            self.amoo_ui.account_scope.as_deref(),
        )
    }
    pub(super) fn workspace_busy(&self) -> bool {
        self.workspace.running.is_some()
            || self.work.running
            || self.studio_lab.handle.is_some()
            || self.amoo_ui.gui_running
    }
    pub(super) fn workspace_can_send(&self) -> bool {
        !self.busy
            && !self.workspace_busy()
            && self.workspace.proposal.is_none()
            && self.fields[F_AMOO].preedit.is_empty()
            && (!self.fields[F_AMOO].text.trim().is_empty() || !self.amoo_attachments.is_empty())
            && !self
                .amoo_sessions
                .get(self.amoo_session_idx)
                .is_some_and(|s| self.amoo_pending_reply.contains_key(&s.session_key))
    }
    pub(super) fn workspace_flush(&mut self) {
        self.workspace_capture_draft();
        self.workspace_persist();
        self.workspace.writer.take();
        if let Some(thread) = self.workspace.writer_thread.take() {
            let _ = thread.join();
        }
    }
    pub(super) fn workspace_persist(&self) {
        if !self.demo_mode {
            if let Some(tx) = &self.workspace.writer {
                let _ = tx.send(self.workspace.journal.clone());
            }
        }
    }
    fn workspace_ensure_task(&mut self) {
        if self.workspace.active.is_none() {
            let task = Task::new(None, None);
            self.workspace.active = Some(task.id.clone());
            self.workspace.journal.tasks.push(task);
        }
    }
    fn workspace_capture_draft(&mut self) {
        let draft = self.fields[F_AMOO].text.clone();
        let paths = self
            .amoo_attachments
            .iter()
            .filter_map(|a| a.local_path.clone())
            .collect::<Vec<_>>();
        if self.workspace.active.is_none() && draft.is_empty() && paths.is_empty() {
            return;
        }
        self.workspace_ensure_task();
        let intent = self.workspace.intent;
        if let Some(task) = self.workspace_task_mut() {
            if task.turns.is_empty() && task.session.is_none() {
                task.title = if draft.trim().is_empty() {
                    "新任务".into()
                } else {
                    draft.trim().chars().take(26).collect()
                };
            }
            task.draft = draft.clone();
            task.attachments = paths.clone();
            task.intent = intent;
        }
        self.workspace.last_draft = (draft, paths);
    }
    pub(super) fn workspace_skill_imported(&mut self, root: &PathBuf) {
        self.workspace_ensure_task();
        if let Some(t) = self.workspace_task_mut() {
            t.workspace = Some(root.clone());
        }
        self.workspace.intent = Intent::Files;
        self.workspace.panel = 0;
        self.workspace_capture_draft();
        self.workspace_persist();
    }
    pub(super) fn workspace_bind_session(&mut self, key: &str) {
        if let Some(run) = &self.workspace.running {
            let id = run.task.clone();
            if let Some(task) = self.workspace.journal.tasks.iter_mut().find(|t| t.id == id) {
                task.session = Some(key.into());
            }
            if let Some(run) = &mut self.workspace.running {
                run.accepted = true;
            }
        }
    }
    fn workspace_restore(&mut self) {
        if let Some(task) = self.workspace_task().cloned() {
            self.fields[F_AMOO].set(&task.draft);
            self.workspace.intent = task.intent;
            self.amoo_attachments = task
                .attachments
                .iter()
                .map(|p| AmooAttachment {
                    name: p
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                    size_label: std::fs::metadata(p)
                        .map(|m| storage::format_bytes(m.len()))
                        .unwrap_or("文件已移动，请重新添加".into()),
                    is_image: p.extension().and_then(|e| e.to_str()).is_some_and(|e| {
                        ["png", "jpg", "jpeg", "webp", "gif"].contains(&e.to_lowercase().as_str())
                    }),
                    local_path: Some(p.clone()),
                })
                .collect();
            self.amoo_session_idx = task
                .session
                .as_ref()
                .and_then(|key| {
                    self.amoo_sessions
                        .iter()
                        .position(|s| &s.session_key == key)
                })
                .unwrap_or(usize::MAX);
            self.work.workspace = task.workspace.clone();
            self.work.local_preview = None;
            self.work.local_preview_loading = None;
            self.work.local_preview_error = None;
            self.work.remote_preview_url = None;
        } else {
            self.fields[F_AMOO].clear();
            self.amoo_attachments.clear();
            self.workspace.intent = Intent::Chat;
            self.amoo_session_idx = usize::MAX;
        }
        self.workspace.proposal = None;
        self.workspace.selected_output = None;
        self.workspace.delivery = None;
        self.workspace.panel = 0;
        self.workspace.history = false;
        self.clear_text_interaction();
        self.reveal_latest_chat();
        self.load_work_previews();
    }
    pub(super) fn handle_workspace(
        &mut self,
        action: &WorkspaceAction,
        ctx: &mut ViewContext<Self>,
    ) {
        if let Err(error) = self.do_workspace(action, ctx) {
            self.workspace.error = Some(format!("{error:#}"));
            self.status = format!("{error:#}");
            self.error = true;
        }
        self.workspace_capture_draft();
        self.workspace_persist();
        ctx.notify();
    }
    fn do_workspace(
        &mut self,
        action: &WorkspaceAction,
        ctx: &mut ViewContext<Self>,
    ) -> Result<()> {
        use WorkspaceAction as A;
        self.workspace.error = None;
        if matches!(action, A::Menu(_) | A::Preview(_) | A::Settings) {
            self.workspace.panel_entered = Instant::now();
        }
        match action {
            A::History => {
                self.workspace.history = !self.workspace.history;
                self.workspace.panel = 0;
                self.scroll_aux = Default::default();
            }
            A::ClosePanel => {
                self.workspace.panel = 0;
                self.workspace.delivery = None;
                self.workspace.history = false;
            }
            A::New => {
                anyhow::ensure!(
                    !self.busy && !self.workspace_busy(),
                    "任务仍在执行，请先等待完成或停止"
                );
                self.workspace_capture_draft();
                self.workspace.active = None;
                self.workspace_restore();
                self.focus_field(F_AMOO, ctx);
            }
            A::Select(id) => {
                anyhow::ensure!(
                    !self.busy && !self.workspace_busy(),
                    "任务仍在执行，完成后可以切换记录"
                );
                anyhow::ensure!(
                    self.workspace
                        .journal
                        .tasks
                        .iter()
                        .any(|t| &t.id == id && self.workspace_visible(t)),
                    "此任务属于其他设备或账号"
                );
                self.workspace_capture_draft();
                self.workspace.active = Some(id.clone());
                self.workspace_restore();
                if let Some(i) = self
                    .amoo_sessions
                    .get(self.amoo_session_idx)
                    .map(|_| self.amoo_session_idx)
                {
                    self.handle_action(&Action::AmooSelectSession(i), ctx);
                }
            }
            A::PhoneSession(i) => {
                anyhow::ensure!(
                    !self.busy && !self.workspace_busy(),
                    "任务仍在执行，完成后可以切换记录"
                );
                let session = self
                    .amoo_sessions
                    .get(*i)
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("会话已更新，请刷新记录"))?;
                self.workspace_capture_draft();
                let existing = self
                    .workspace
                    .journal
                    .tasks
                    .iter()
                    .find(|t| {
                        self.workspace_visible(t)
                            && t.session.as_ref() == Some(&session.session_key)
                    })
                    .map(|t| t.id.clone());
                let id = existing.unwrap_or_else(|| {
                    let mut task = Task::new(
                        self.selected.as_ref().map(|d| d.serial.clone()),
                        self.amoo_ui.account_scope.clone(),
                    );
                    task.title = amoo_display::session_title(&session.title);
                    task.session = Some(session.session_key);
                    let id = task.id.clone();
                    self.workspace.journal.tasks.push(task);
                    id
                });
                self.workspace.active = Some(id);
                self.workspace_restore();
                self.handle_action(&Action::AmooSelectSession(*i), ctx);
            }
            A::Suggest(i) => {
                let (intent,prompt)=match i {0=>(Intent::Files,"分析附件中的表格，核对数据与异常，生成可核对的结果表和分析报告。"),1=>(Intent::Files,"根据提供的资料制作一份汇报 PPT，先梳理结构，再生成可编辑文件。"),2=>(Intent::Image,"生成一张"),_=>(Intent::App,"做一个适合手机使用的专注计时器，支持开始、暂停、继续、重置，实际检查交互。")};
                self.workspace.intent = intent;
                self.fields[F_AMOO].set(prompt);
                self.workspace.proposal = None;
                self.focus_field(F_AMOO, ctx);
            }
            A::Intent(intent) => {
                self.workspace.intent = if *intent == Intent::Auto {
                    Intent::Chat
                } else {
                    *intent
                };
                self.workspace.proposal = None;
            }
            A::Menu(7) => self.open_workspace_intent_menu(ctx),
            A::Menu(panel) => {
                if *panel == 3 && !(1..=4).contains(&self.work.tab) {
                    self.work.tab = 2;
                }
                self.workspace.panel = if self.workspace.panel == *panel {
                    0
                } else {
                    *panel
                };
                self.workspace.history = false;
                self.workspace.preview_scroll = Default::default();
                if *panel == 3
                    && self.work.catalog.is_none()
                    && !self.busy
                    && self.selected.is_some()
                {
                    self.refresh_work();
                }
                if *panel == 4 && !self.studio.loaded && !self.busy {
                    self.refresh_studio();
                }
            }
            A::Settings => {
                self.workspace.panel = 5;
                self.page = Page::Amoo;
            }
            A::ChooseFolder => {
                anyhow::ensure!(!self.workspace_busy(), "执行期间不能修改资料目录");
                if let Some(path) = native_dialog::FileDialog::new()
                    .set_title("选择此任务可读取资料、保存成果的文件夹")
                    .show_open_single_dir()?
                {
                    self.workspace_ensure_task();
                    let path = path.canonicalize()?;
                    self.work.workspace = Some(path.clone());
                    self.workspace_task_mut().unwrap().workspace = Some(path);
                }
            }
            A::Prepare => {
                anyhow::ensure!(
                    self.workspace_can_send(),
                    "请等待当前操作完成；草稿与附件已保留"
                );
                self.workspace_capture_draft();
                let text = if self.fields[F_AMOO].text.trim().is_empty() {
                    "请查看并分析这些附件。".into()
                } else {
                    self.fields[F_AMOO].text.trim().to_string()
                };
                let files = self
                    .amoo_attachments
                    .iter()
                    .filter_map(|a| a.local_path.clone())
                    .collect::<Vec<_>>();
                anyhow::ensure!(
                    files.len() == self.amoo_attachments.len() && files.iter().all(|p| p.is_file()),
                    "有附件已移动或不可读取，请重新添加"
                );
                let steps = model::propose_followup(
                    &text,
                    if self.workspace.intent == Intent::Auto {
                        Intent::Chat
                    } else {
                        self.workspace.intent
                    },
                    !files.is_empty(),
                    self.workspace_task()
                        .and_then(|t| t.steps.last())
                        .map(|s| s.executor),
                );
                let simple = steps == [Executor::Phone];
                self.workspace.proposal = Some(Proposal { text, files, steps });
                self.workspace.panel = 0;
                if simple {
                    if let Err(error) = self.workspace_confirm(ctx) {
                        // Connection and send errors remain chat errors, never a work confirmation.
                        self.workspace.proposal = None;
                        return Err(error);
                    }
                } else {
                    self.reveal_latest_chat();
                }
            }
            A::EditPlan => {
                // Editing a plan keeps the explicitly selected capability.
                // Only accepting/submitting a turn resets the next turn to chat.
                self.workspace.proposal = None;
                self.focus_field(F_AMOO, ctx);
            }
            A::Confirm => self.workspace_confirm(ctx)?,
            A::Stop => {
                if let Some(run) = self.workspace.running.clone() {
                    self.workspace.next = false;
                    match run.executor {
                        Executor::Files => self.handle_work_action(&WorkAction::Cancel, ctx),
                        Executor::App => self.handle_studio_lab_action(&StudioLabAction::Stop, ctx),
                        _ => {
                            self.workspace_finish(
                                Phase::Unconfirmed,
                                "已停止等待；手机任务可能仍继续，可同步查看结果".into(),
                            );
                        }
                    }
                }
            }
            A::Retry => {
                anyhow::ensure!(!self.busy && !self.workspace_busy(), "请等待当前操作完成");
                let task = self
                    .workspace_task()
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("请选择任务"))?;
                let prompt = task
                    .turns
                    .iter()
                    .rev()
                    .find(|t| t.user)
                    .map(|t| t.text.clone())
                    .unwrap_or_default();
                self.fields[F_AMOO].set(&prompt);
                let start = task
                    .steps
                    .iter()
                    .position(|s| {
                        matches!(s.phase, Phase::Failed | Phase::Stopped | Phase::Unconfirmed)
                    })
                    .unwrap_or(task.steps.len().saturating_sub(1));
                let steps = task
                    .steps
                    .iter()
                    .skip(start)
                    .map(|s| s.executor)
                    .collect::<Vec<_>>();
                let files = if steps.first() == Some(&Executor::App) {
                    vec![]
                } else {
                    task.submitted_files
                };
                self.amoo_attachments.clear();
                self.add_amoo_paths(&files)?;
                self.workspace.proposal = Some(Proposal {
                    text: prompt,
                    files,
                    steps,
                });
            }
            A::Preview(i) => {
                let output = self
                    .workspace_task()
                    .and_then(|t| t.outputs.get(*i))
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("此成果不可用"))?;
                self.workspace.selected_output = Some(*i);
                self.workspace.panel = 6;
                self.workspace.preview_scroll = Default::default();
                if let Some(id) = output.lab_id {
                    self.workspace_select_lab(&id)?;
                } else {
                    self.work.workspace = self.workspace_task().and_then(|t| t.workspace.clone());
                    self.preview_work_file(output.path)?;
                }
            }
            A::SelectLab(id) => {
                anyhow::ensure!(!self.workspace_busy(), "请先等待当前任务完成");
                self.workspace_capture_draft();
                self.workspace_select_lab(id)?;
                let artifact = self.studio_lab_selected().cloned().unwrap();
                let existing = self
                    .workspace
                    .journal
                    .tasks
                    .iter()
                    .find(|t| {
                        self.workspace_visible(t)
                            && t.outputs.iter().any(|o| o.lab_id.as_ref() == Some(id))
                    })
                    .map(|t| t.id.clone());
                if let Some(id) = existing {
                    self.workspace.active = Some(id);
                } else {
                    let mut task = Task::new(None, None);
                    task.title = artifact.name.clone();
                    task.add_output(Output {
                        path: artifact.html_path,
                        name: artifact.name,
                        kind: "应用".into(),
                        lab_id: Some(id.clone()),
                    });
                    self.workspace.active = Some(task.id.clone());
                    self.workspace.journal.tasks.push(task);
                }
                self.workspace_restore();
                let i = self
                    .workspace_task()
                    .unwrap()
                    .outputs
                    .iter()
                    .position(|o| o.lab_id.as_ref() == Some(id))
                    .unwrap();
                return self.do_workspace(&A::Preview(i), ctx);
            }
            A::AddOutput(i) => {
                let path = self
                    .workspace_task()
                    .and_then(|t| t.outputs.get(*i))
                    .map(|o| o.path.clone())
                    .ok_or_else(|| anyhow::anyhow!("文件不存在"))?;
                self.add_amoo_paths(&[path])?;
                self.workspace.panel = 0;
            }
            A::Deliver(cloud) => {
                anyhow::ensure!(!self.busy, "请等待当前操作完成");
                let output = self
                    .workspace_task()
                    .and_then(|t| t.outputs.get(self.workspace.selected_output?))
                    .context("请先选择成果")?;
                self.workspace.delivery_target = Some((
                    self.workspace.active.clone().unwrap_or_default(),
                    output.path.clone(),
                    self.selected.as_ref().map(|d| d.serial.clone()),
                    self.studio.web.as_ref().map(|w| w.scope.clone()),
                ));
                self.workspace.delivery = Some(*cloud);
                self.workspace.open_phone = false;
                self.workspace.panel = 6;
                if *cloud && self.studio.web.is_none() {
                    self.sync_studio_cloud();
                }
            }
            A::CancelDelivery => {
                self.workspace.delivery = None;
                self.workspace.delivery_target = None;
            }
            A::ToggleOpenPhone => self.workspace.open_phone = !self.workspace.open_phone,
            A::ConfirmDelivery => {
                anyhow::ensure!(!self.demo_mode, "演示模式不会发送文件或同步官网");
                anyhow::ensure!(!self.busy, "正在交付，请勿重复点击");
                let cloud = self
                    .workspace
                    .delivery
                    .ok_or_else(|| anyhow::anyhow!("请先确认交付目标"))?;
                let output = self
                    .workspace_task()
                    .and_then(|t| t.outputs.get(self.workspace.selected_output?))
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("请选择成果"))?;
                let target = (
                    self.workspace.active.clone().unwrap_or_default(),
                    output.path.clone(),
                    self.selected.as_ref().map(|d| d.serial.clone()),
                    self.studio.web.as_ref().map(|w| w.scope.clone()),
                );
                let expected = self
                    .workspace
                    .delivery_target
                    .as_ref()
                    .context("请重新核对交付目标")?;
                anyhow::ensure!(
                    target.0 == expected.0
                        && target.1 == expected.1
                        && target.2 == expected.2
                        && (!cloud || target.3 == expected.3),
                    "作品或目标账号已变化，请重新点击发送或同步并确认"
                );
                if let Some(id) = &output.lab_id {
                    self.workspace_select_lab(id)?;
                    if cloud {
                        if let Some(url) = self
                            .workspace_task()
                            .and_then(|t| {
                                t.receipts.iter().rev().find(|r| {
                                    r.path == output.path
                                        && r.device == target.2.clone().unwrap_or_default()
                                        && r.account == target.3
                                        && r.cloud_url.is_some()
                                })
                            })
                            .and_then(|r| r.cloud_url.clone())
                        {
                            self.with_device(
                                "正在打开已同步的官网草稿…",
                                move |adb, d| {
                                    Ok(Outcome::Notice(stepx_companion::studio::open_editor(
                                        adb, &d, &url,
                                    )?))
                                },
                            );
                        } else {
                            self.handle_studio_lab_action(&StudioLabAction::SyncCloud, ctx);
                        }
                    } else {
                        let a = self.studio_lab_selected().cloned().unwrap();
                        let open = self.workspace.open_phone;
                        self.with_device("正在保存到手机作品库…", move |adb, d| {
                            let result = (|| {
                                let html = stepx_companion::studio_lab::verified_html(&a)?;
                                let r = stepx_companion::studio_delivery::install_local(
                                    adb, &d, &a.name, &html,
                                )?;
                                if open {
                                    stepx_companion::studio_delivery::open_local(adb, &d, &r.id)?;
                                }
                                Ok(format!(
                                    "{} 已保存到手机作品库，可离线打开{}",
                                    a.name,
                                    if open { "，已打开" } else { "" }
                                ))
                            })();
                            Ok(Outcome::StudioLab(StudioLabOutcome::Phone(
                                d.serial,
                                Some(a.id),
                                result.map_err(|e: anyhow::Error| e.to_string()),
                            )))
                        });
                    }
                } else {
                    anyhow::ensure!(!cloud, "文件请先导出；官网同步支持已验证的应用源码");
                    let path = output.path;
                    let task = self.workspace.active.clone().unwrap_or_default();
                    self.with_device("正在发送文件到手机…", move |adb, d| {
                        let result =
                            stepx_companion::companion_link::send_files(adb, &d, &[path.clone()])
                                .map(|count| {
                                    format!(
                                        "已校验并发送 {count} 个文件到 StepX伴侣 → 文件 → 接收文件"
                                    )
                                })
                                .map_err(|e| e.to_string());
                        Ok(Outcome::WorkspaceDelivered(task, path, d.serial, result))
                    });
                }
                self.workspace.delivery = None;
            }
            A::OpenProject(i) => {
                anyhow::ensure!(!self.demo_mode, "演示模式不会打开真实官网项目");
                self.studio.web_selected = Some(*i);
                let url = self
                    .studio
                    .web
                    .as_ref()
                    .and_then(|w| w.items.get(*i))
                    .and_then(|w| w.editor())
                    .context("这个项目没有可编辑的官网地址")?;
                self.with_device(
                    "正在登录官网并打开对应项目…",
                    move |adb, d| {
                        Ok(Outcome::Notice(stepx_companion::studio::open_editor(
                            adb, &d, &url,
                        )?))
                    },
                );
            }
        }
        Ok(())
    }
    fn workspace_select_lab(&mut self, id: &str) -> Result<()> {
        let i = self
            .studio_lab
            .artifacts
            .iter()
            .position(|a| a.id == id)
            .ok_or_else(|| anyhow::anyhow!("作品记录未载入，请刷新本机作品"))?;
        self.studio_lab.selected = Some(i);
        self.studio_lab.source = Some(self.studio_lab.artifacts[i].html_path.clone());
        self.studio_lab.report = None;
        self.studio_lab.report_loading = None;
        Ok(())
    }
    fn workspace_confirm(&mut self, ctx: &mut ViewContext<Self>) -> Result<()> {
        anyhow::ensure!(
            !self.demo_mode,
            "演示模式仅用于检查界面，不会执行手机任务或模型请求"
        );
        anyhow::ensure!(!self.busy && !self.workspace_busy(), "请等待当前操作完成");
        let proposal = self
            .workspace
            .proposal
            .clone()
            .ok_or_else(|| anyhow::anyhow!("请先核对执行计划"))?;
        let requires_phone = proposal
            .steps
            .iter()
            .any(|s| *s != Executor::App || !self.studio_lab.config.has_key);
        anyhow::ensure!(
            !requires_phone || self.selected.is_some(),
            "请连接手机后确认开始；计划和附件已保留"
        );
        if proposal.steps.contains(&Executor::App) {
            anyhow::ensure!(self.studio_lab.loaded, "正在读取模型配置，请稍后确认");
            anyhow::ensure!(
                !self.studio_lab.config.has_key
                    || (!self.studio_lab.config.model.is_empty()
                        && !self.studio_lab_config_dirty()),
                "制作应用需先在“模型与账号”中保存并测试自己的 API"
            );
        }
        let current = self.fields[F_AMOO].text.trim();
        anyhow::ensure!(
            current == proposal.text
                || (current.is_empty() && proposal.text == "请查看并分析这些附件。"),
            "草稿已更改，请返回重新核对计划"
        );
        let paths = self
            .amoo_attachments
            .iter()
            .filter_map(|a| a.local_path.clone())
            .collect::<Vec<_>>();
        anyhow::ensure!(paths == proposal.files, "附件已更改，请返回重新核对计划");
        self.workspace_ensure_task();
        let default_root = self
            .settings
            .value
            .work_directory
            .clone()
            .unwrap_or_else(|| {
                stepx_companion::resources::user_state().join("amoo-workspace/files")
            });
        let device = self.selected.as_ref().map(|d| d.serial.clone());
        let account = self.amoo_ui.account_scope.clone();
        let task = self.workspace_task_mut().unwrap();
        if requires_phone {
            task.device = device;
            task.account = account;
        }
        if task.turns.is_empty() {
            task.title = proposal.text.chars().take(26).collect();
        }
        task.turns.push(Turn {
            user: true,
            text: proposal.text.clone(),
        });
        task.steps = proposal
            .steps
            .iter()
            .map(|e| Step {
                executor: *e,
                phase: Phase::Ready,
                note: String::new(),
            })
            .collect();
        task.updated = chrono::Utc::now().timestamp();
        if task.workspace.is_none()
            && proposal
                .steps
                .iter()
                .any(|e| matches!(e, Executor::Files | Executor::App | Executor::Image))
        {
            task.workspace = Some(default_root.join(&task.id));
        }
        task.submitted_files = proposal.files;
        task.draft.clear();
        task.note.clear();
        self.workspace.proposal = None;
        self.workspace.panel = 0;
        // The selected capability applies to this request, never to the next chat turn.
        self.workspace.intent = Intent::Chat;
        self.workspace.next = true;
        if let Err(e) = self.workspace_start_next(ctx) {
            self.workspace_finish(Phase::Failed, e.to_string());
            return Err(e);
        }
        Ok(())
    }
    fn workspace_start_next(&mut self, ctx: &mut ViewContext<Self>) -> Result<()> {
        if self.busy || self.workspace_busy() {
            return Ok(());
        }
        let Some(task) = self.workspace_task().cloned() else {
            return Ok(());
        };
        let Some(index) = task.steps.iter().position(|s| s.phase == Phase::Ready) else {
            self.workspace.next = false;
            return Ok(());
        };
        let executor = task.steps[index].executor;
        let original = task
            .turns
            .iter()
            .rev()
            .find(|t| t.user)
            .map(|t| t.text.clone())
            .unwrap_or_default();
        self.workspace.next = false;
        self.workspace.running = Some(ActiveRun {
            task: task.id.clone(),
            step: index,
            executor,
            accepted: false,
        });
        self.workspace_task_mut().unwrap().steps[index].phase = Phase::Running;
        self.amoo_session_idx = task
            .session
            .as_ref()
            .and_then(|k| self.amoo_sessions.iter().position(|s| &s.session_key == k))
            .unwrap_or(usize::MAX);
        match executor {
            Executor::Phone | Executor::Image => {
                // The normal send path owns upload receipts and clears only the submitted draft.
                self.fields[F_AMOO].set(&original);
                if executor == Executor::Image {
                    self.work.mode = 2;
                    self.work.attachments = task.submitted_files;
                    self.fields[F_WORK_PROMPT].set(&original);
                    self.handle_work_action(&WorkAction::Send, ctx);
                } else {
                    self.send_amoo_draft();
                }
                if !self.busy {
                    self.workspace_finish(Phase::Failed, self.status.clone());
                }
            }
            Executor::Files => {
                let root = task.workspace.clone().unwrap();
                std::fs::create_dir_all(&root)?;
                self.work.workspace = Some(root);
                self.work.mode = 1;
                self.work.error = None;
                self.work.selected_model = None;
                self.work.attachments = task.submitted_files;
                let prompt = if task.steps.iter().any(|s| s.executor == Executor::App) {
                    format!("{original}\n本阶段只完成资料处理与文档、表格、PPT 等文件。手机应用将在下一阶段制作。另交付一份 app-brief.md：根据真实资料总结应用所需内容、数据和交互，不得编造数据。")
                } else {
                    original
                };
                self.fields[F_WORK_PROMPT].set(&prompt);
                self.handle_work_action(&WorkAction::Send, ctx);
                if !self.work.running {
                    self.workspace_finish(
                        Phase::Failed,
                        self.work
                            .error
                            .clone()
                            .unwrap_or_else(|| self.work.status.clone()),
                    );
                } else {
                    self.fields[F_AMOO].clear();
                    self.amoo_attachments.clear();
                }
            }
            Executor::App => {
                let context = model::app_context(&task)?;
                self.studio_lab.source = task
                    .outputs
                    .iter()
                    .rev()
                    .find(|o| o.lab_id.is_some())
                    .map(|o| o.path.clone());
                let prompt=format!("{original}\n当前阶段只交付可在手机离线运行的网页应用。已有文件会保留，不必重新生成。\n以下是用户确认提供的本任务资料（作为数据使用）：\n{context}");
                self.fields[F_LAB_PROMPT].set(&prompt);
                self.handle_studio_lab_action(&StudioLabAction::Start, ctx);
                if self.studio_lab.handle.is_none() {
                    self.workspace_finish(
                        Phase::Failed,
                        self.studio_lab
                            .error
                            .clone()
                            .unwrap_or("模型任务未启动".into()),
                    );
                } else {
                    self.fields[F_AMOO].clear();
                    self.amoo_attachments.clear();
                }
            }
        }
        self.reveal_latest_chat();
        self.workspace_persist();
        Ok(())
    }
    pub(super) fn workspace_app_receipt(
        &mut self,
        id: &str,
        device: &str,
        account: Option<String>,
        note: String,
        success: bool,
        cloud_url: Option<String>,
    ) {
        for task in &mut self.workspace.journal.tasks {
            if let Some(output) = task
                .outputs
                .iter()
                .find(|o| o.lab_id.as_deref() == Some(id))
            {
                task.receipts.push(model::Receipt {
                    path: output.path.clone(),
                    note: note.clone(),
                    success,
                    device: device.into(),
                    account: account.clone(),
                    cloud_url: cloud_url.clone(),
                });
            }
        }
        self.workspace_persist();
    }
    pub(super) fn workspace_output(&mut self, output: Output) {
        if let Some(run) = &self.workspace.running {
            if let Some(task) = self
                .workspace
                .journal
                .tasks
                .iter_mut()
                .find(|t| t.id == run.task)
            {
                task.add_output(output);
            }
        }
        self.workspace_persist();
    }
    pub(super) fn workspace_finish(&mut self, phase: Phase, note: String) {
        // A late download receipt must not complete a later request in the same chat.
        self.workspace.image_delivery = None;
        if let Some(run) = self.workspace.running.take() {
            if let Some(task) = self
                .workspace
                .journal
                .tasks
                .iter_mut()
                .find(|t| t.id == run.task)
            {
                if let Some(step) = task.steps.get_mut(run.step) {
                    step.phase = phase;
                    step.note = note.clone();
                }
                task.note = note.clone();
                task.turns.push(Turn {
                    user: false,
                    text: note,
                });
                task.updated = chrono::Utc::now().timestamp();
                self.workspace.next =
                    phase == Phase::Done && task.steps.iter().any(|s| s.phase == Phase::Ready);
            }
        }
        self.workspace_persist();
    }
    pub(super) fn workspace_deliver_images(
        &mut self,
        messages: Vec<stepx_companion::openclaw::RealChatMessage>,
        note: String,
    ) {
        if self.workspace.image_delivery.is_some() {
            return;
        }
        let Some(run) = self.workspace.running.clone() else {
            return;
        };
        let mut seen = std::collections::HashSet::new();
        let images = messages
            .iter()
            .flat_map(|m| {
                m.attachments
                    .iter()
                    .cloned()
                    .chain(pages::markdown_images(&m.text))
            })
            .filter(|a| {
                (a.kind == "image" || a.mime.starts_with("image/"))
                    && (!a.url.is_empty() || !a.blob.is_empty())
            })
            .filter(|a| seen.insert((a.url.clone(), a.blob.clone())))
            .collect::<Vec<_>>();
        if images.is_empty() {
            self.workspace_finish(Phase::Unconfirmed, format!("{note}\n\n尚未收到可下载的图片文件，不能确认图片已完成。请同步手机结果或继续要求交付原图。"));
            return;
        }
        let Some(root) = self.workspace_task().and_then(|t| t.workspace.clone()) else {
            self.workspace_finish(
                Phase::Unconfirmed,
                "图片保存目录不可用，请重新选择保存位置".into(),
            );
            return;
        };
        let Some(device) = self.selected.clone() else {
            self.workspace_finish(
                Phase::Unconfirmed,
                "手机已断开，图片尚未保存到电脑，请重连后同步".into(),
            );
            return;
        };
        let adb = match Adb::discover() {
            Ok(adb) => adb,
            Err(error) => {
                self.workspace_finish(Phase::Unconfirmed, format!("图片尚未保存：{error}"));
                return;
            }
        };
        let (tx, rx) = mpsc::channel();
        self.workspace.image_delivery = Some(ImageDelivery {
            task: run.task,
            scope: self.amoo_ui.account_scope.clone(),
            note,
            receiver: rx,
        });
        self.work.status = "已收到图片，正在保存原图到电脑…".into();
        std::thread::spawn(move || {
            let result = (|| -> Result<Vec<(String, PathBuf)>> {
                let folder = root.join(format!("Images-{}", chrono::Utc::now().timestamp_millis()));
                std::fs::create_dir_all(&folder)?;
                let mut saved = Vec::new();
                for (i, a) in images.iter().enumerate() {
                    let path = folder.join(format!("image-{}.png", i + 1));
                    let file =
                        stepx_companion::amoo_media::download_artifact(&adb, &device, a, &path)?;
                    let bytes = std::fs::read(&file.path)?;
                    image::load_from_memory(&bytes).context("收到的附件不是有效图片")?;
                    let format = image::guess_format(&bytes)?;
                    let path = file
                        .path
                        .with_extension(format.extensions_str().first().copied().unwrap_or("png"));
                    if path != file.path {
                        std::fs::rename(&file.path, &path)?;
                    }
                    saved.push((a.url.clone(), path));
                }
                Ok(saved)
            })()
            .map_err(|e| format!("{e:#}"));
            let _ = tx.send(result);
        });
    }
    pub(super) fn tick_workspace(&mut self, ctx: &mut ViewContext<Self>) -> bool {
        let mut changed = false;
        let image_result =
            self.workspace.image_delivery.as_ref().and_then(|delivery| {
                match delivery.receiver.try_recv() {
                    Ok(result) => Some(result),
                    Err(mpsc::TryRecvError::Empty) => None,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        Some(Err("图片保存任务意外结束，请重新同步".into()))
                    }
                }
            });
        if let Some(result) = image_result {
            let ImageDelivery {
                task, scope, note, ..
            } = self.workspace.image_delivery.take().unwrap();
            if self
                .workspace
                .running
                .as_ref()
                .is_some_and(|r| r.task == task && r.executor == Executor::Image)
            {
                if scope != self.amoo_ui.account_scope {
                    self.workspace_finish(
                        Phase::Unconfirmed,
                        "手机账号已变化，图片未并入当前账号，请切回原账号核对".into(),
                    );
                    return true;
                }
                match result {
                    Ok(files) => {
                        let count = files.len();
                        for (url, path) in files {
                            self.work.image_previews.insert(url, path.clone());
                            self.workspace_output(Output {
                                name: path
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .into_owned(),
                                path,
                                kind: "图片".into(),
                                lab_id: None,
                            });
                        }
                        self.workspace_finish(
                            Phase::Done,
                            format!("{note}\n\n{count} 张原图已保存在电脑，可预览或发送到手机。"),
                        );
                    }
                    Err(error) => self.workspace_finish(
                        Phase::Unconfirmed,
                        format!("已收到图片链接，但原图保存未完成：{error}"),
                    ),
                }
                changed = true;
            }
        }
        if let Some(next) = ctx.windows().platform_window(ctx.window_id()).map(|w| w.size()) {
            if self.workspace.viewport != next {
                self.workspace.viewport = next;
                changed = true;
            }
        }
        if self
            .workspace_task()
            .is_some_and(|t| !self.workspace_visible(t))
            && !self.workspace_busy()
        {
            self.workspace_capture_draft();
            self.workspace.active = None;
            self.workspace_restore();
            changed = true;
        }
        if self.workspace.next && !self.busy && !self.workspace_busy() {
            if let Err(e) = self.workspace_start_next(ctx) {
                self.workspace_finish(Phase::Failed, e.to_string());
                self.workspace.error = Some(e.to_string());
                self.workspace.next = false;
            }
            changed = true;
        }
        if let Some(run) = self.workspace.running.clone() {
            if matches!(run.executor, Executor::Phone | Executor::Image)
                && run.accepted
                && self.selected.is_none()
            {
                self.workspace_finish(
                    Phase::Unconfirmed,
                    "手机连接中断，尚未确认回复完成。重连后同步任务记录；未自动重发。".into(),
                );
                changed = true;
            } else if run.executor == Executor::Files && !self.work.running {
                self.workspace_finish(
                    Phase::Unconfirmed,
                    "手机账号已变化，原任务未继续执行；已生成文件保留。".into(),
                );
                changed = true;
            }
        }
        if !self.workspace_busy() {
            self.amoo_session_idx = self
                .workspace_task()
                .and_then(|t| t.session.as_ref())
                .and_then(|k| self.amoo_sessions.iter().position(|s| &s.session_key == k))
                .unwrap_or(usize::MAX);
        }
        if self.workspace.last_saved.elapsed() > Duration::from_millis(700) {
            let current = (
                self.fields[F_AMOO].text.clone(),
                self.amoo_attachments
                    .iter()
                    .filter_map(|a| a.local_path.clone())
                    .collect(),
            );
            if current != self.workspace.last_draft {
                self.workspace_capture_draft();
                self.workspace_persist();
            }
            self.workspace.last_saved = Instant::now();
        }
        changed
    }
}

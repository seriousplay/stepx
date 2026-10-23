//! Amoo Work's view state and orchestration. All mobile requests use the selected device.
use super::*;
use anyhow::Context;
use std::collections::HashSet;
use std::sync::atomic::Ordering;
use stepx_companion::{amoo_catalog, amoo_media, work};

#[derive(Clone, Debug)]
pub(super) enum WorkAction {
    Refresh,
    VerifyModels,
    SearchSkills,
    ImportDesktopSkill,
    UsePhoneSkill(String),
    CreateAgent,
    Tab(u8),
    Panel(u8),
    Capabilities,
    Mode(u8),
    ChooseWorkspace,
    RefreshFiles,
    ToggleAttachMenu,
    Attach,
    AttachImages,
    RemoveAttachment(usize),
    Send,
    Cancel,
    NewChat,
    Session(usize),
    OpenRemote(String),
    PreviewAttachment {
        name: String,
        kind: String,
        mime: String,
        url: String,
    },
    SaveAttachment {
        name: String,
        kind: String,
        mime: String,
        url: String,
    },
    OpenPreview(String),
    SelectModel(Option<String>),
    Template(u8),
    OpenArtifact(usize),
    OpenFile(usize),
    PreviewArtifact(usize),
    PreviewFile(usize),
    PreviewRemote(usize),
    ClosePreview,
    OpenLocalPreview,
    ExportLocalPreview,
    OpenWorkspace,
    InstallSkill {
        key: String,
        install_id: String,
    },
    ConfirmSkillInstall,
    CancelSkillInstall,
    ToggleSkill {
        key: String,
        enabled: bool,
    },
    SelectAgent(String),
    CatalogPage(usize),
}

#[derive(Clone, Debug)]
pub(super) struct WorkFileItem {
    pub path: PathBuf,
    pub name: String,
    pub bytes: u64,
    pub is_dir: bool,
}
#[derive(Clone, Debug)]
pub(super) struct WorkDesktopSkillItem {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
}

#[derive(Clone, Debug)]
pub(super) struct WorkArtifactItem {
    pub path: PathBuf,
    pub name: String,
    pub kind: String,
}

#[derive(Clone, Debug)]
pub(super) struct WorkLocalPreview {
    pub path: PathBuf,
    pub name: String,
    pub bytes: u64,
    pub image: bool,
    pub text: Option<String>,
    pub markdown: bool,
    pub note: String,
}

pub(super) struct WorkUiState {
    pub tab: u8,
    pub panel: u8,
    pub capabilities_open: bool,
    pub mode: u8,
    pub workspace: Option<PathBuf>,
    pub attachments: Vec<PathBuf>,
    pub attach_menu_open: bool,
    pub local_preview: Option<WorkLocalPreview>,
    pub local_preview_loading: Option<PathBuf>,
    pub local_preview_error: Option<String>,
    pub remote_preview_url: Option<String>,
    pub desktop_skills: Vec<WorkDesktopSkillItem>,
    pub market_skills: Vec<amoo_catalog::SkillInfo>,
    pub files: Vec<WorkFileItem>,
    pub artifacts: Vec<WorkArtifactItem>,
    pub events: Vec<String>,
    pub status: String,
    pub running: bool,
    pub selected_model: Option<String>,
    pub selected_agent: Option<String>,
    pub session_models: HashMap<String, String>,
    pub catalog: Option<amoo_catalog::WorkCatalog>,
    pub catalog_loading: bool,
    pub catalog_page: usize,
    pub error: Option<String>,
    pub pending_install: Option<(String, String)>,
    pub files_scroll: ClippedScrollStateHandle,
    pub prompt_scroll: ClippedScrollStateHandle,
    pub image_previews: HashMap<String, PathBuf>,
    pub preview_errors: HashMap<String, String>,
    preview_identities: HashMap<String, String>,
    preview_attempted: HashSet<String>,
    preview_cache: Option<Arc<tempfile::TempDir>>,
    preview_rx: Option<mpsc::Receiver<(String, std::result::Result<PathBuf, String>)>>,
    handle: Option<work::WorkHandle>,
    active_key: Option<String>,
    submitted_draft: Option<String>,
}

impl Default for WorkUiState {
    fn default() -> Self {
        let local_preview = initial_local_preview();
        let workspace = local_preview
            .as_ref()
            .and_then(|preview| preview.path.parent())
            .map(std::path::Path::to_path_buf);
        let files = workspace
            .as_ref()
            .and_then(|root| {
                work::list_workspace(root).ok().map(|files| {
                    files
                        .into_iter()
                        .map(|file| WorkFileItem {
                            path: root.join(&file.relative_path),
                            name: file.relative_path,
                            bytes: file.bytes,
                            is_dir: file.directory,
                        })
                        .collect()
                })
            })
            .unwrap_or_default();
        Self {
            tab: std::env::args()
                .position(|arg| arg == "--work-tab")
                .and_then(|index| std::env::args().nth(index + 1))
                .map(|tab| match tab.as_str() {
                    "models" => 1,
                    "skills" => 2,
                    "agents" => 3,
                    "apps" => 4,
                    _ => 0,
                })
                .unwrap_or(0),
            panel: 2,
            capabilities_open: std::env::args().any(|arg| arg == "--work-tab"),
            mode: 1,
            workspace,
            attachments: Vec::new(),
            attach_menu_open: false,
            local_preview,
            local_preview_loading: None,
            local_preview_error: None,
            remote_preview_url: None,
            desktop_skills: Vec::new(),
            market_skills: Vec::new(),
            files,
            artifacts: Vec::new(),
            events: Vec::new(),
            status: "选择手机会话继续处理，或创建工作任务".into(),
            running: false,
            selected_model: None,
            selected_agent: None,
            session_models: HashMap::new(),
            catalog: None,
            catalog_loading: false,
            catalog_page: 0,
            error: None,
            pending_install: None,
            files_scroll: Default::default(),
            prompt_scroll: Default::default(),
            image_previews: initial_image_previews(),
            preview_errors: HashMap::new(),
            preview_identities: HashMap::new(),
            preview_attempted: HashSet::new(),
            preview_cache: None,
            preview_rx: None,
            handle: None,
            active_key: None,
            submitted_draft: None,
        }
    }
}

// Explicit local preview fixture for native layout captures; absent from release behavior.
fn initial_local_preview() -> Option<WorkLocalPreview> {
    #[cfg(feature = "ui-capture")]
    {
        let args: Vec<_> = std::env::args().collect();
        if let Some(path) = args
            .iter()
            .position(|arg| arg == "--work-local-preview")
            .and_then(|index| args.get(index + 1))
            .map(PathBuf::from)
            .and_then(|path| path.canonicalize().ok())
        {
            return path
                .parent()
                .and_then(|root| read_work_preview(root, &path).ok());
        }
    }
    None
}

// Local-only image fixture for the native capture build. Production starts empty.
fn initial_image_previews() -> HashMap<String, PathBuf> {
    #[cfg(feature = "ui-capture")]
    {
        let args: Vec<_> = std::env::args().collect();
        if args.iter().any(|arg| arg == "--work-format-fixture") {
            if let Some(path) = args
                .iter()
                .position(|arg| arg == "--work-preview-file")
                .and_then(|index| args.get(index + 1))
                .map(PathBuf::from)
                .filter(|path| path.is_file())
            {
                return HashMap::from([(
                    "https://example.invalid/amoo-work-native-preview.png".into(),
                    path,
                )]);
            }
        }
    }
    HashMap::new()
}

impl WorkUiState {
    pub(super) fn reset_account_work(&mut self) {
        self.disconnect();
        self.handle = None;
        self.running = false;
        self.active_key = None;
        self.submitted_draft = None;
        self.local_preview = None;
        self.local_preview_loading = None;
        self.local_preview_error = None;
        self.events.clear();
        self.artifacts.clear();
        self.attachments.clear();
        self.error = None;
        self.status = "手机账号已变化，请同步当前账号后开始工作".into();
    }

    pub(super) fn clear_account_previews(&mut self) {
        self.preview_rx = None;
        self.preview_cache = None;
        self.preview_attempted.clear();
        self.image_previews.clear();
        self.preview_errors.clear();
        self.preview_identities.clear();
        self.remote_preview_url = None;
    }

    pub fn disconnect(&mut self) {
        if let Some(handle) = &self.handle {
            handle.cancel.store(true, Ordering::Relaxed);
        }
        self.catalog = None;
        self.market_skills.clear();
        self.catalog_loading = false;
        self.selected_model = None;
        self.selected_agent = None;
        self.session_models.clear();
        self.pending_install = None;
        self.preview_rx = None;
        self.preview_cache = None;
        self.preview_attempted.clear();
        self.image_previews = initial_image_previews();
        self.preview_errors.clear();
        self.preview_identities.clear();
        self.remote_preview_url = None;
        self.local_preview = None;
        self.local_preview_loading = None;
        self.local_preview_error = None;
        self.events.clear();
        self.artifacts.clear();
        self.attachments.clear();
        self.error = None;
        self.active_key = None;
        self.submitted_draft = None;
        self.handle = None;
        self.running = false;
        self.status = "手机已断开，请连接设备后同步工作".into();
    }
    fn log(&mut self, message: String) {
        self.events.push(message);
        if self.events.len() > 200 {
            self.events.remove(0);
        }
    }
}

pub(super) enum WorkOutcome {
    ModelsVerified {
        models: Vec<amoo_catalog::ModelInfo>,
        account_scope: Option<String>,
    },
    LocalPreview {
        workspace: PathBuf,
        path: PathBuf,
        result: std::result::Result<WorkLocalPreview, String>,
    },
    AttachmentSaved {
        path: PathBuf,
        bytes: u64,
    },
    SkillImported {
        workspace: PathBuf,
        name: String,
        omitted: usize,
    },
    MarketSkills(Vec<amoo_catalog::SkillInfo>),
    Refreshed {
        data: RealAmooData,
        catalog: std::result::Result<amoo_catalog::WorkCatalog, String>,
    },
    Sent {
        receipt: amoo::PendingReply,
        draft: String,
        attachments: Vec<PathBuf>,
    },
    CatalogChanged(amoo_catalog::WorkCatalog, String),
}

impl Companion {
    /// Only a stable native message ID can alias renewed attachment URLs.
    pub(super) fn record_work_attachment_identities(&mut self, real: &RealAmooData) {
        for session in &real.sessions {
            for message in &session.messages {
                if message.message_id.is_empty() {
                    continue;
                }
                for (index, attachment) in message.attachments.iter().enumerate() {
                    if attachment.url.is_empty() {
                        continue;
                    }
                    let identity = serde_json::json!([
                        session.key,
                        message.message_id,
                        index,
                        attachment.name,
                        attachment.mime,
                        attachment.size_bytes
                    ])
                    .to_string();
                    self.work
                        .preview_identities
                        .insert(attachment.url.clone(), identity);
                }
            }
        }
    }

    pub(super) fn refresh_work(&mut self) {
        if self.busy || self.work.catalog_loading {
            return;
        }
        if self.selected.is_none() {
            self.work.error = Some("连接手机后即可同步 Amoo 任务；本地工作文件夹仍可查看".into());
            return;
        }
        self.work.catalog_loading = true;
        self.work.error = None;
        let key = self
            .amoo_sessions
            .get(self.amoo_session_idx)
            .map(|s| s.session_key.clone());
        self.with_device(
            "正在同步 Amoo 工作会话与能力…",
            move |adb, device| {
                let data = amoo::fetch(adb, &device, key.as_deref())?;
                let catalog = amoo_catalog::fetch(adb, &device).map_err(|e| format!("{e:#}"));
                Ok(Outcome::Work(WorkOutcome::Refreshed { data, catalog }))
            },
        );
    }

    pub(super) fn apply_work(&mut self, result: WorkOutcome) {
        match result {
            WorkOutcome::ModelsVerified {
                models,
                account_scope,
            } => {
                self.record_amoo_usage(&RealAmooData {
                    account_scope,
                    ..Default::default()
                });
                let count = models.len();
                let catalog = self.work.catalog.get_or_insert_with(Default::default);
                catalog.models = models;
                catalog.can_select_model = false;
                catalog.models_status = amoo_catalog::CatalogAvailability {
                    available: true,
                    message: format!("已验证 {count} 个手机 Amoo 云端路由；手机自动选择模型，当前版本未开放指定接口。"),
                };
                self.work.catalog_page = 0;
                self.work.status = "已完成真实模型调用，只显示当前账号可用的结果".into();
                self.status = self.work.status.clone();
            }
            WorkOutcome::LocalPreview {
                workspace,
                path,
                result,
            } => {
                if self.work.workspace.as_ref() == Some(&workspace)
                    && self.work.local_preview_loading.as_ref() == Some(&path)
                {
                    self.work.local_preview_loading = None;
                    match result {
                        Ok(preview) => self.work.local_preview = Some(preview),
                        Err(error) => self.work.local_preview_error = Some(error),
                    }
                }
            }
            WorkOutcome::AttachmentSaved { path, bytes } => {
                self.work.status = format!(
                    "已保存 {}（{}）",
                    path.display(),
                    stepx_companion::storage::format_bytes(bytes)
                );
                self.status = self.work.status.clone();
            }
            WorkOutcome::SkillImported {
                workspace,
                name,
                omitted,
            } => {
                if self.work.workspace.as_ref() == Some(&workspace) {
                    let _ = self.work_files();
                }
                self.work.status = if omitted == 0 {
                    format!("已将 {name} 加入桌面工作流程，下一条桌面任务可使用")
                } else {
                    format!("已导入 {name} 的流程文档；{omitted} 个脚本或非文本文件未导入")
                };
                self.work.mode = 1;
                self.work.tab = 0;
                self.work.capabilities_open = false;
                self.status = self.work.status.clone();
            }
            WorkOutcome::MarketSkills(skills) => {
                self.work.status = format!("已查询到 {} 个技能市场条目", skills.len());
                self.work.market_skills = skills;
                self.status = self.work.status.clone();
            }
            WorkOutcome::Refreshed { data, catalog } => {
                self.apply_real_amoo_data(data);
                if !self.work.running {
                    let _ = self.work_files();
                }
                if self.work.tab == 0 {
                    Self::scroll_to_end(&self.scroll);
                }
                self.work.catalog_loading = false;
                match catalog {
                    Ok(catalog) => {
                        self.work.catalog = Some(catalog);
                        self.work.error = None;
                    }
                    Err(error) => self.work.error = Some(error),
                }
                self.work.status = format!("已同步 {} 个手机会话", self.amoo_sessions.len());
                self.status = self.work.status.clone();
            }
            WorkOutcome::Sent {
                receipt,
                draft,
                attachments,
            } => {
                self.amoo_ui
                    .session_started
                    .insert(receipt.key.clone(), Instant::now());
                self.work.active_key = Some(receipt.key.clone());
                if !self
                    .amoo_sessions
                    .iter()
                    .any(|session| session.session_key == receipt.key)
                {
                    self.apply(Outcome::AmooCreated(receipt.key.clone()));
                }
                if self.fields[F_WORK_PROMPT].text.trim() == draft.trim() {
                    self.fields[F_WORK_PROMPT].clear();
                }
                if self.work.attachments == attachments {
                    self.work.attachments.clear();
                }
                self.apply(Outcome::AmooSent(receipt));
                self.work.status = "手机已接收，正在等待回复".into();
            }
            WorkOutcome::CatalogChanged(catalog, message) => {
                self.work.catalog = Some(catalog);
                self.work.catalog_loading = false;
                self.work.pending_install = None;
                self.work.status = message.clone();
                self.status = message;
            }
        }
    }

    fn work_files(&mut self) -> Result<()> {
        let Some(root) = self.work.workspace.as_ref() else {
            return Ok(());
        };
        self.work.files = work::list_workspace(root)?
            .into_iter()
            .map(|file| WorkFileItem {
                path: root.join(&file.relative_path),
                name: file.relative_path,
                bytes: file.bytes,
                is_dir: file.directory,
            })
            .collect();
        self.work.desktop_skills = work::list_skills(root)?
            .into_iter()
            .map(|skill| WorkDesktopSkillItem {
                name: skill.name,
                description: skill.description,
                path: skill.path,
            })
            .collect();
        let key = self
            .amoo_sessions
            .get(self.amoo_session_idx)
            .map(|session| session.session_key.as_str())
            .unwrap_or("");
        self.work.artifacts = work::load_session_artifacts(root, key)?
            .into_iter()
            .map(|file| WorkArtifactItem {
                name: file.relative_path,
                path: file.path,
                kind: file.kind,
            })
            .collect();
        Ok(())
    }

    pub(super) fn handle_work_action(&mut self, action: &WorkAction, ctx: &mut ViewContext<Self>) {
        self.work.error = None;
        let result = self.do_work_action(action, ctx);
        if result.is_ok() && !self.busy {
            self.status = self.work.status.clone();
            self.error = false;
        }
        if let Err(error) = result {
            let message = format!("{error:#}");
            self.work.error = Some(message.clone());
            self.error = true;
            self.status = message;
        }
    }

    fn do_work_action(&mut self, action: &WorkAction, ctx: &mut ViewContext<Self>) -> Result<()> {
        match action {
            WorkAction::Tab(tab) => {
                self.page = Page::Work;
                self.work.tab = *tab;
                self.work.capabilities_open = *tab > 0;
                self.work.catalog_page = 0;
                self.scroll = Default::default();
                if self.work.catalog.is_none() && !self.busy {
                    self.refresh_work();
                }
            }
            WorkAction::Panel(panel) => {
                self.work.panel = if self.work.panel == *panel { 0 } else { *panel };
            }
            WorkAction::Capabilities => {
                self.work.capabilities_open = !self.work.capabilities_open;
                self.work.tab = if self.work.capabilities_open { 2 } else { 0 };
                self.scroll = Default::default();
                if self.work.capabilities_open && self.work.catalog.is_none() && !self.busy {
                    self.refresh_work();
                }
            }
            WorkAction::CatalogPage(page) => {
                self.work.catalog_page = *page;
                self.scroll = Default::default();
            }
            WorkAction::Mode(mode) => {
                anyhow::ensure!(
                    !self.busy && !self.work.running,
                    "请先等待当前工作完成，或停止桌面执行"
                );
                if *mode == 1 && !self.work.attachments.is_empty() {
                    if self.work.workspace.is_none() {
                        let Some(path) = native_dialog::FileDialog::new()
                            .set_title("选择桌面工作文件夹")
                            .show_open_single_dir()?
                        else {
                            return Ok(());
                        };
                        self.work.workspace = Some(path.canonicalize()?);
                    }
                    let root = self.work.workspace.as_ref().context("请选择工作文件夹")?;
                    let mut local_files = Vec::new();
                    for path in &self.work.attachments {
                        let path = path.canonicalize()?;
                        if path.starts_with(root) {
                            local_files.push(path);
                        } else {
                            local_files
                                .push(root.join(work::import_file(root, &path)?.relative_path));
                        }
                    }
                    self.work.attachments = local_files;
                    self.work_files()?;
                }
                self.work.mode = *mode;
            }
            WorkAction::Refresh => self.refresh_work(),
            WorkAction::VerifyModels => {
                anyhow::ensure!(
                    !self.busy && !self.work.running,
                    "请等待当前任务完成后验证模型"
                );
                if let Some(catalog) = &mut self.work.catalog {
                    catalog.models.clear();
                    catalog.models_status = amoo_catalog::CatalogAvailability {
                        available: false,
                        message: "本次验证尚未确认可用模型；验证成功后会显示结果。".into(),
                    };
                }
                self.work.selected_model = None;
                self.with_device(
                    "正在验证账号模型权限，发送一条短测试请求…",
                    |adb, device| {
                        let models = amoo_catalog::verify_models(adb, &device)?;
                        let account_scope =
                            stepx_companion::amoo_usage::account_scope(adb, &device)?;
                        anyhow::ensure!(
                            models
                                .iter()
                                .any(|model| model.is_verified_for(account_scope.as_deref())),
                            "模型验证后账号身份已变化或无法核实，请重新验证"
                        );
                        Ok(Outcome::Work(WorkOutcome::ModelsVerified {
                            models,
                            account_scope,
                        }))
                    },
                );
            }
            WorkAction::SearchSkills => {
                anyhow::ensure!(!self.busy, "请等待当前同步完成");
                let query = self.fields[F_WORK_CAP_SEARCH].text.trim().to_string();
                anyhow::ensure!(!query.is_empty(), "请输入要搜索的技能名称或工作场景");
                self.with_device("正在搜索手机技能市场…", move |adb, device| {
                    Ok(Outcome::Work(WorkOutcome::MarketSkills(
                        amoo_catalog::search_skills(adb, &device, &query)?,
                    )))
                });
            }
            WorkAction::ImportDesktopSkill => {
                anyhow::ensure!(!self.work.running, "请等待当前工作完成后导入工作流程");
                let root = self
                    .work
                    .workspace
                    .clone()
                    .ok_or_else(|| anyhow::anyhow!("请先选择工作文件夹"))?;
                if let Some(path) = native_dialog::FileDialog::new()
                    .set_title("选择包含 SKILL.md 的工作流程文件夹")
                    .show_open_single_dir()?
                {
                    let skill = work::import_skill(&root, &path)?;
                    self.work.status = format!("已导入工作流程 {}，桌面任务可读取使用", skill.name);
                    self.work_files()?;
                }
            }
            WorkAction::UsePhoneSkill(key) => {
                anyhow::ensure!(!self.busy && !self.work.running, "请等待当前操作完成");
                if self.work.workspace.is_none() {
                    let Some(path) = native_dialog::FileDialog::new()
                        .set_title("选择使用技能的工作文件夹")
                        .show_open_single_dir()?
                    else {
                        return Ok(());
                    };
                    self.work.workspace = Some(path.canonicalize()?);
                    self.work_files()?;
                }
                let workspace = self.work.workspace.clone().context("请选择工作文件夹")?;
                let key = key.clone();
                self.with_device(
                    "正在从手机导入工作流程…",
                    move |adb, device| {
                        let bundle = amoo_catalog::export_skill(adb, &device, &key)?;
                        let documents = bundle
                            .files
                            .into_iter()
                            .map(|file| (file.path, file.content))
                            .collect::<Vec<_>>();
                        let skill = work::import_skill_documents(&workspace, &documents)?;
                        Ok(Outcome::Work(WorkOutcome::SkillImported {
                            workspace,
                            name: skill.name,
                            omitted: bundle.omitted,
                        }))
                    },
                );
            }
            WorkAction::CreateAgent => {
                anyhow::ensure!(!self.busy && !self.work.running, "请等待当前操作完成");
                let name = self.fields[F_WORK_AGENT_NAME].text.trim().to_string();
                anyhow::ensure!(!name.is_empty(), "请输入新智能体名称");
                self.with_device("正在创建手机智能体…", move |adb, device| {
                    let catalog = amoo_catalog::create_agent(adb, &device, &name)?;
                    Ok(Outcome::Work(WorkOutcome::CatalogChanged(
                        catalog,
                        format!("已在手机 OpenClaw 创建智能体 {name}"),
                    )))
                });
            }
            WorkAction::ChooseWorkspace => {
                anyhow::ensure!(
                    !self.busy && !self.work.running,
                    "工作进行中，请先停止桌面执行再切换文件夹"
                );
                if let Some(path) = native_dialog::FileDialog::new()
                    .set_title("选择 Amoo Work 工作文件夹")
                    .show_open_single_dir()?
                {
                    let path = path.canonicalize()?;
                    self.work.workspace = Some(path);
                    self.work.local_preview = None;
                    self.work.local_preview_loading = None;
                    self.work.local_preview_error = None;
                    self.work_files()?;
                    self.work.status =
                        "已选择工作文件夹；桌面任务仅在该目录内读取和生成文件".into();
                }
            }
            WorkAction::RefreshFiles => self.work_files()?,
            WorkAction::ToggleAttachMenu => {
                self.work.attach_menu_open = !self.work.attach_menu_open;
            }
            WorkAction::Attach | WorkAction::AttachImages => {
                anyhow::ensure!(
                    !self.busy && !self.work.running,
                    "请等待当前操作完成后添加文件"
                );
                if self.work.mode == 1 && self.work.workspace.is_none() {
                    let Some(path) = native_dialog::FileDialog::new()
                        .set_title("选择桌面工作文件夹")
                        .show_open_single_dir()?
                    else {
                        return Ok(());
                    };
                    self.work.workspace = Some(path.canonicalize()?);
                    self.work_files()?;
                }
                self.work.attach_menu_open = false;
                let images_only = matches!(action, WorkAction::AttachImages);
                let dialog = native_dialog::FileDialog::new().set_title(if images_only {
                    "上传本地图片"
                } else {
                    "添加文档与附件"
                });
                let dialog = if images_only {
                    dialog.add_filter("图片", &["png", "jpg", "jpeg", "webp", "gif", "bmp"])
                } else {
                    dialog
                };
                let selected = dialog.show_open_multiple_file()?;
                if !selected.is_empty() {
                    anyhow::ensure!(
                        self.work.attachments.len() + selected.len() <= 8,
                        "最多添加 8 个附件，请减少选择数量"
                    );
                    if self.work.mode != 1 {
                        let mut all = self.work.attachments.clone();
                        all.extend(selected.iter().cloned());
                        amoo_media::prepare_attachments(&all)?;
                    }
                    for path in selected {
                        let local = if self.work.mode == 1 {
                            let root =
                                self.work.workspace.as_ref().context("请先选择工作文件夹")?;
                            let file = work::import_file(root, &path)?;
                            root.join(file.relative_path)
                        } else {
                            let path = path.canonicalize()?;
                            anyhow::ensure!(path.is_file(), "请选择文件作为附件");
                            path
                        };
                        if !self.work.attachments.contains(&local) {
                            self.work.attachments.push(local);
                        }
                    }
                    self.work_files()?;
                    self.work.status = "附件已加入草稿，发送后交给 Amoo 处理".into();
                }
            }
            WorkAction::RemoveAttachment(index) => {
                anyhow::ensure!(!self.busy && !self.work.running, "任务进行中，附件已经提交");
                if *index < self.work.attachments.len() {
                    self.work.attachments.remove(*index);
                }
            }
            WorkAction::SelectModel(model) => {
                anyhow::ensure!(!self.work.running, "工作进行中，模型不能中途变更");
                if let Some(id) = model {
                    let catalog = self
                        .work
                        .catalog
                        .as_ref()
                        .ok_or_else(|| anyhow::anyhow!("请先刷新模型目录"))?;
                    anyhow::ensure!(
                        catalog.can_select_model,
                        "当前手机 Amoo 云会话没有开放模型指定接口，将使用手机默认路由"
                    );
                    anyhow::ensure!(
                        catalog.models.iter().any(|m| m.id == *id
                            && m.is_verified_for(self.amoo_ui.account_scope.as_deref())),
                        "该模型未在手机配置，不能用于当前任务"
                    );
                }
                self.work.selected_model = model.clone();
                self.work.status = "下一条任务将使用所选模型；发送前会校验手机回执".into();
            }
            WorkAction::SelectAgent(id) => {
                let agent = self
                    .work
                    .catalog
                    .as_ref()
                    .and_then(|catalog| catalog.agents.iter().find(|agent| agent.id == *id))
                    .ok_or_else(|| anyhow::anyhow!("智能体已不在当前手机列表，请刷新"))?;
                self.work.status = format!(
                    "{} 属于手机 OpenClaw 环境，当前 Amoo 云会话不支持切换到该智能体",
                    agent.name
                );
            }
            WorkAction::NewChat => {
                anyhow::ensure!(
                    !self.busy && !self.work.running,
                    "请等待当前操作完成后新建任务"
                );
                self.work.tab = 0;
                self.work.capabilities_open = false;
                self.work.panel = 2;
                self.work.events.clear();
                self.work.artifacts.clear();
                self.work.error = None;
                self.work.active_key = None;
                self.work.mode = 1;
                self.fields[F_WORK_PROMPT].clear();
                self.work.attachments.clear();
                self.work.attach_menu_open = false;
                self.work.local_preview = None;
                self.work.local_preview_loading = None;
                self.work.local_preview_error = None;
                self.work.remote_preview_url = None;
                self.focus_field(F_WORK_PROMPT, ctx);
                self.with_device("正在创建 Amoo 工作会话…", |adb, device| {
                    Ok(Outcome::AmooCreated(
                        amoo::AgentBus::connect(adb, &device)?.create_session()?,
                    ))
                });
            }
            WorkAction::Session(index) => {
                anyhow::ensure!(
                    !self.busy && !self.work.running,
                    "正在同步或执行工作，请等待完成后切换会话"
                );
                if *index < self.amoo_sessions.len() {
                    self.work.panel = 2;
                    self.work.capabilities_open = false;
                    self.work.remote_preview_url = None;
                    self.work.local_preview = None;
                    self.work.local_preview_loading = None;
                    self.work.local_preview_error = None;
                    self.amoo_session_idx = *index;
                    self.work.events.clear();
                    self.work.artifacts.clear();
                    self.work.error = None;
                    self.work.attachments.clear();
                    self.work.active_key = None;
                    self.work.status = "已切换工作会话".into();
                    self.fields[F_WORK_PROMPT].clear();
                    self.work_files()?;
                    self.work.tab = 0;
                    self.work.selected_model = None;
                    self.scroll = Default::default();
                    if !self.busy {
                        let key = self.amoo_sessions[*index].session_key.clone();
                        self.with_device("正在同步工作会话…", move |adb, device| {
                            Ok(Outcome::AmooRealData(
                                amoo::fetch(adb, &device, Some(&key))?,
                                "已同步工作会话".into(),
                            ))
                        });
                    }
                }
            }
            WorkAction::Template(index) => {
                anyhow::ensure!(
                    !self.work.running,
                    "请等待当前任务完成或停止执行后开始新任务"
                );
                let prompts = [
                    "请整理工作文件夹中的资料：先列出资料清单，提炼关键结论，标出来源文件和不确定点，生成可阅读的总结报告并保存到输出文件夹。",
                    "请分析工作文件夹中的表格：核对列含义与缺失值，统计关键指标，指出异常及依据，生成分析报告与可核对的结果表。",
                    "请根据工作文件夹中的资料撰写一份工作文档：先明确目标和结构，生成完整正文，核查数字和来源，保存成可打开的文档。",
                    "请使用手机上实际可用的图片生成能力完成图片创作，并返回生成图片及可访问的产物位置。开始前确认主题、尺寸和风格；缺少能力时请明确说明。",
                    "请制作一个可离线打开直接玩的方块沙盒小游戏：包括移动、放置和移除方块、材料切换、中文操作说明与重置。生成完整 HTML 文件，读回检查交互逻辑，交付文件并简短说明玩法，不要在对话里重复粘贴全部代码。",
                    "请根据工作资料制作一份结构清楚、版面简洁的 PowerPoint：提炼结论和依据，每页一个主题，生成真正可编辑的 PPTX 文件并检查页面内容。资料不足的地方明确标注，不要编造数据。",
                ];
                self.fields[F_WORK_PROMPT].set(prompts[usize::from(*index).min(prompts.len() - 1)]);
                self.work.mode = if *index == 3 { 2 } else { 1 };
                self.work.tab = 0;
                self.work.capabilities_open = false;
                self.focus_field(F_WORK_PROMPT, ctx);
            }
            WorkAction::Send => self.send_work()?,
            WorkAction::Cancel => {
                if let Some(handle) = &self.work.handle {
                    handle.cancel.store(true, Ordering::Relaxed);
                    self.work.status = "正在停止桌面执行；已提交到手机的任务仍可能继续".into();
                } else if let Some(session) = self.amoo_sessions.get(self.amoo_session_idx) {
                    self.amoo_pending_reply.remove(&session.session_key);
                    self.work.status =
                        "已停止等待；手机任务可能仍在后台继续，可稍后同步查看结果".into();
                }
            }
            WorkAction::OpenRemote(url) => {
                anyhow::ensure!(
                    url.starts_with("https://") || url.starts_with("http://"),
                    "手机未提供可打开的网页链接"
                );
                anyhow::ensure!(!url.chars().any(char::is_control), "附件链接无效");
                let command = if cfg!(target_os = "macos") {
                    "open"
                } else {
                    "xdg-open"
                };
                std::process::Command::new(command).arg(url).spawn()?;
                self.work.status = "已在浏览器打开链接".into();
            }
            WorkAction::PreviewAttachment {
                name,
                kind,
                mime,
                url,
            } => {
                anyhow::ensure!(self.work.preview_rx.is_none(), "图片正在载入，请稍候");
                self.work.preview_attempted.remove(url);
                self.work.preview_errors.remove(url);
                self.begin_work_previews(vec![stepx_companion::openclaw::RealAttachment {
                    name: name.clone(),
                    kind: kind.clone(),
                    mime: mime.clone(),
                    url: url.clone(),
                    ..Default::default()
                }])?;
            }
            WorkAction::SaveAttachment {
                name,
                kind,
                mime,
                url,
            } => {
                anyhow::ensure!(!self.busy, "请等待当前同步完成后保存附件");
                let filename = std::path::Path::new(name)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .filter(|s| !s.is_empty())
                    .unwrap_or("Amoo附件");
                if let Some(path) = native_dialog::FileDialog::new()
                    .set_title("保存 Amoo 附件")
                    .set_filename(filename)
                    .show_save_single_file()?
                {
                    anyhow::ensure!(!path.exists(), "该文件已存在，请选择新名称保存");
                    let attachment = stepx_companion::openclaw::RealAttachment {
                        name: name.clone(),
                        kind: kind.clone(),
                        mime: mime.clone(),
                        url: url.clone(),
                        ..Default::default()
                    };
                    let cached = self.work.image_previews.get(url).cloned();
                    let scope = self.amoo_ui.account_scope.clone();
                    let identity = self.work.preview_identities.get(url).cloned();
                    self.with_device("正在保存手机附件…", move |adb, device| {
                        if let Some(expected) = scope.as_ref() {
                            anyhow::ensure!(
                                stepx_companion::amoo_usage::account_scope(adb, &device)?.as_ref()
                                    == Some(expected),
                                "手机账号已变化，请重新同步后保存附件"
                            );
                        }
                        let persistent = preview_cache::root().zip(scope);
                        let stored = persistent.as_ref().and_then(|(root, scope)| {
                            preview_cache::load_with_identity(
                                root,
                                scope,
                                &attachment.url,
                                identity.as_deref(),
                            )
                        });
                        let bytes = if let Some(source) = stored.or(cached.filter(|p| p.is_file()))
                        {
                            let mut input = std::fs::File::open(source)?;
                            let mut output = std::fs::OpenOptions::new()
                                .write(true)
                                .create_new(true)
                                .open(&path)?;
                            std::io::copy(&mut input, &mut output)?
                        } else {
                            let file =
                                amoo_media::download_artifact(adb, &device, &attachment, &path)?;
                            if attachment.kind == "image" || attachment.mime.starts_with("image/") {
                                if let Some((root, scope)) = persistent.as_ref() {
                                    let _ = preview_cache::save_with_identity(
                                        root,
                                        scope,
                                        &attachment.url,
                                        &file.path,
                                        identity.as_deref(),
                                    );
                                }
                            }
                            file.bytes
                        };
                        Ok(Outcome::Work(WorkOutcome::AttachmentSaved { path, bytes }))
                    });
                }
            }
            WorkAction::OpenPreview(url) => {
                let path = self
                    .work
                    .image_previews
                    .get(url)
                    .context("图片尚未载入，请重试")?;
                open_work_path(path)?;
            }
            WorkAction::OpenWorkspace => {
                let path = self
                    .work
                    .workspace
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("请先选择工作文件夹"))?;
                open_work_path(path)?;
            }
            WorkAction::OpenFile(index) => {
                let file = self
                    .work
                    .files
                    .get(*index)
                    .ok_or_else(|| anyhow::anyhow!("文件已变化，请刷新"))?;
                validate_open_path(self.work.workspace.as_deref(), &file.path)?;
                open_work_path(&file.path)?;
            }
            WorkAction::OpenArtifact(index) => {
                let file = self
                    .work
                    .artifacts
                    .get(*index)
                    .ok_or_else(|| anyhow::anyhow!("产物已变化，请刷新"))?;
                validate_open_path(self.work.workspace.as_deref(), &file.path)?;
                open_work_path(&file.path)?;
            }
            WorkAction::PreviewArtifact(index) => {
                let path = self
                    .work
                    .artifacts
                    .get(*index)
                    .context("产物已变化，请刷新")?
                    .path
                    .clone();
                self.preview_work_file(path)?;
            }
            WorkAction::PreviewFile(index) => {
                let file = self.work.files.get(*index).context("文件已变化，请刷新")?;
                if file.is_dir {
                    validate_open_path(self.work.workspace.as_deref(), &file.path)?;
                    open_work_path(&file.path)?;
                } else {
                    self.preview_work_file(file.path.clone())?;
                }
            }
            WorkAction::PreviewRemote(index) => {
                let item = self
                    .amoo_sessions
                    .get(self.amoo_session_idx)
                    .and_then(|session| session.items.get(*index))
                    .context("附件已变化，请同步会话")?;
                if let AmooChatItem::Attachment { url, path, .. } = item {
                    self.work.remote_preview_url = Some(if url.is_empty() {
                        path.clone()
                    } else {
                        url.clone()
                    });
                    self.work.local_preview = None;
                    self.work.local_preview_loading = None;
                    self.work.local_preview_error = None;
                    self.work.panel = 2;
                }
            }
            WorkAction::ClosePreview => {
                self.work.local_preview = None;
                self.work.local_preview_loading = None;
                self.work.local_preview_error = None;
                self.work.remote_preview_url = None;
            }
            WorkAction::OpenLocalPreview => {
                let preview = self
                    .work
                    .local_preview
                    .as_ref()
                    .context("请先选择文件预览")?;
                validate_open_path(self.work.workspace.as_deref(), &preview.path)?;
                open_work_path(&preview.path)?;
            }
            WorkAction::ExportLocalPreview => {
                anyhow::ensure!(!self.busy, "请等待当前操作完成");
                let preview = self
                    .work
                    .local_preview
                    .as_ref()
                    .context("请先选择文件预览")?;
                validate_open_path(self.work.workspace.as_deref(), &preview.path)?;
                let filename = preview
                    .path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Amoo工作成果");
                if let Some(destination) = native_dialog::FileDialog::new()
                    .set_title("导出工作成果")
                    .set_filename(filename)
                    .show_save_single_file()?
                {
                    let source = preview.path.clone();
                    let root = self.work.workspace.clone();
                    self.submit("正在导出工作成果…", move |_| {
                        validate_open_path(root.as_deref(), &source)?;
                        let mut input = std::fs::File::open(&source)?;
                        let mut output = std::fs::OpenOptions::new()
                            .write(true)
                            .create_new(true)
                            .open(&destination)
                            .context("目标文件已存在或无法写入，请使用新名称")?;
                        let bytes = std::io::copy(&mut input, &mut output)?;
                        Ok(Outcome::Work(WorkOutcome::AttachmentSaved {
                            path: destination,
                            bytes,
                        }))
                    });
                }
            }
            WorkAction::InstallSkill { key, install_id } => {
                anyhow::ensure!(!self.busy && !self.work.running, "请等待当前操作完成");
                self.work.pending_install = Some((key.clone(), install_id.clone()));
            }
            WorkAction::CancelSkillInstall => self.work.pending_install = None,
            WorkAction::ConfirmSkillInstall => self.install_work_skill()?,
            WorkAction::ToggleSkill { key, enabled } => self.toggle_work_skill(key, *enabled)?,
        }
        Ok(())
    }

    fn preview_work_file(&mut self, path: PathBuf) -> Result<()> {
        anyhow::ensure!(!self.busy, "请等待当前操作完成后预览");
        let workspace = self.work.workspace.clone().context("请先选择工作文件夹")?;
        validate_open_path(Some(&workspace), &path)?;
        self.work.panel = 2;
        self.work.local_preview = None;
        self.work.local_preview_error = None;
        self.work.remote_preview_url = None;
        self.work.local_preview_loading = Some(path.clone());
        self.submit("正在读取文件预览…", move |_| {
            let result = read_work_preview(&workspace, &path).map_err(|error| format!("{error:#}"));
            Ok(Outcome::Work(WorkOutcome::LocalPreview {
                workspace,
                path,
                result,
            }))
        });
        Ok(())
    }

    fn send_work(&mut self) -> Result<()> {
        anyhow::ensure!(
            !self.busy && !self.work.running,
            "当前操作尚未完成，草稿已保留"
        );
        let draft = self.fields[F_WORK_PROMPT].text.trim().to_string();
        anyhow::ensure!(!draft.is_empty(), "请输入工作任务");
        let device = self
            .selected
            .clone()
            .ok_or_else(|| anyhow::anyhow!("请先连接手机"))?;
        let session = self
            .amoo_sessions
            .get(self.amoo_session_idx)
            .map(|s| s.session_key.clone());
        anyhow::ensure!(
            !session
                .as_ref()
                .is_some_and(|key| self.amoo_pending_reply.contains_key(key)),
            "该会话仍在等待手机回复，请勿重复发送"
        );
        if self.work.mode != 1 {
            let model = self.work.selected_model.clone();
            let attachments = self.work.attachments.clone();
            let image_task = self.work.mode == 2;
            self.with_device(
                if attachments.is_empty() {
                    "正在把任务交给手机 Amoo…"
                } else {
                    "正在上传附件并交给手机 Amoo…"
                },
                move |adb, device| {
                    let mut bus = amoo::AgentBus::connect(adb, &device)?;
                    let key = match session {
                        Some(key) => key,
                        None => bus.create_session()?,
                    };
                    if let Some(model) = model {
                        bus.set_session_model(&key, &model)?;
                    }
                    let message = if image_task {
                        amoo_media::image_request(&draft)
                    } else {
                        draft.clone()
                    };
                    let mut receipt = if attachments.is_empty() {
                        bus.send(&key, &message)?
                    } else {
                        amoo_media::send_files(adb, &device, &key, &message, &attachments)?
                    };
                    receipt.text = draft.clone();
                    Ok(Outcome::Work(WorkOutcome::Sent {
                        receipt,
                        draft,
                        attachments,
                    }))
                },
            );
        } else {
            if self.work.workspace.is_none() {
                let Some(path) = native_dialog::FileDialog::new()
                    .set_title("选择工作文件夹，供 Amoo 读取资料并保存成果")
                    .show_open_single_dir()?
                else {
                    self.work.status = "草稿已保留；选择工作文件夹后即可开始".into();
                    return Ok(());
                };
                self.work.workspace = Some(path.canonicalize()?);
                self.work_files()?;
            }
            let workspace = self
                .work
                .workspace
                .clone()
                .ok_or_else(|| anyhow::anyhow!("桌面协作需要先选择工作文件夹"))?;
            let mut attachments = Vec::new();
            for path in &self.work.attachments {
                let path = path.canonicalize()?;
                if path.starts_with(&workspace) {
                    attachments.push(path);
                } else {
                    attachments
                        .push(workspace.join(work::import_file(&workspace, &path)?.relative_path));
                }
            }
            self.work.attachments = attachments.clone();
            let request = work::WorkRequest {
                prompt: draft.clone(),
                workspace,
                session_key: session,
                model: self.work.selected_model.clone(),
                attachments,
            };
            let handle = work::start(Adb::discover()?, device, request);
            self.work.handle = Some(handle);
            self.work.submitted_draft = Some(draft);
            self.work.running = true;
            self.work.events.clear();
            self.work.status = "正在启动桌面协作，使用手机模型处理工作文件…".into();
            self.fields[F_WORK_PROMPT].clear();
        }
        Ok(())
    }

    pub(super) fn poll_work(&mut self) -> bool {
        let mut preview_changed = false;
        let mut preview_done = false;
        if let Some(receiver) = &self.work.preview_rx {
            loop {
                match receiver.try_recv() {
                    Ok((url, Ok(path))) => {
                        self.work.image_previews.insert(url, path);
                        preview_changed = true;
                    }
                    Ok((url, Err(error))) => {
                        self.work.preview_errors.insert(url, error);
                        preview_changed = true;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        preview_done = true;
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                }
            }
        }
        if preview_done {
            self.work.preview_rx = None;
            self.load_work_previews();
        }
        let mut events = Vec::new();
        if let Some(handle) = &self.work.handle {
            loop {
                match handle.events.try_recv() {
                    Ok(event) => events.push(event),
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        if self.work.running
                            && !events.iter().any(|event| {
                                matches!(
                                    event,
                                    work::WorkEvent::Complete(_)
                                        | work::WorkEvent::Failed(_)
                                        | work::WorkEvent::Cancelled
                                )
                            })
                        {
                            events.push(work::WorkEvent::Failed(
                                "桌面执行连接已结束，请检查日志后继续；未自动重发手机任务".into(),
                            ));
                        }
                        break;
                    }
                }
            }
        }
        let changed = preview_changed || !events.is_empty();
        for event in events {
            match event {
                work::WorkEvent::SessionReady(key) => {
                    self.work.active_key = Some(key.clone());
                    if let Some(index) = self
                        .amoo_sessions
                        .iter()
                        .position(|session| session.session_key == key)
                    {
                        self.amoo_session_idx = index;
                    } else {
                        self.apply(Outcome::AmooCreated(key));
                    }
                }
                work::WorkEvent::ModelResponse {
                    data,
                    key,
                    run_id,
                    elapsed_ms,
                } => {
                    if self.amoo_ui.account_scope.is_some()
                        && self.amoo_ui.account_scope != data.account_scope
                    {
                        self.work.reset_account_work();
                        break;
                    }
                    self.record_work_response(&data, &key, &run_id, elapsed_ms);
                }
                work::WorkEvent::Status(status) => {
                    self.work.status = status.clone();
                    self.work.log(status);
                }
                work::WorkEvent::Assistant(text) => self.work.log(text),
                work::WorkEvent::Tool(log) => self.work.log(format!(
                    "{} · {} · {}{}",
                    log.step,
                    log.tool,
                    if log.success { "" } else { "失败：" },
                    log.detail
                )),
                work::WorkEvent::Artifact(file) => {
                    self.work.log(format!("已生成 {}", file.relative_path));
                    self.work.artifacts.push(WorkArtifactItem {
                        name: file.relative_path,
                        path: file.path,
                        kind: file.kind,
                    });
                }
                work::WorkEvent::Complete(summary) => {
                    self.work.running = false;
                    self.work.submitted_draft = None;
                    self.work.status = summary.clone();
                    self.work.log(summary);
                    self.work.attachments.clear();
                    let _ = self.work_files();
                    self.last_amoo_poll = Instant::now() - Duration::from_secs(11);
                }
                work::WorkEvent::Failed(error) => {
                    self.work.running = false;
                    if self.fields[F_WORK_PROMPT].text.is_empty() {
                        if let Some(draft) = self.work.submitted_draft.take() {
                            self.fields[F_WORK_PROMPT].set(&draft);
                        }
                    }
                    self.work.error = Some(error.clone());
                    self.work.log(error);
                    self.last_amoo_poll = Instant::now() - Duration::from_secs(11);
                }
                work::WorkEvent::Cancelled => {
                    self.work.running = false;
                    if let Some(draft) = self.work.submitted_draft.take() {
                        if self.fields[F_WORK_PROMPT].text.is_empty() {
                            self.fields[F_WORK_PROMPT].set(&draft);
                        }
                    }
                    self.work.status = "桌面执行已停止；已提交到手机的任务仍可能继续".into();
                    self.work.log(self.work.status.clone());
                }
            }
        }
        if !self.work.running {
            self.work.handle = None;
        }
        changed
    }

    pub(super) fn load_work_previews(&mut self) {
        if (!matches!(self.page, Page::Amoo | Page::Work) && !self.work.running)
            || self.work.preview_rx.is_some() || self.selected.is_none() {
            return;
        }
        let attachments = self
            .amoo_sessions
            .iter()
            .flat_map(|s| s.items.iter().rev())
            .filter_map(|item| match item {
                AmooChatItem::Attachment {
                    name,
                    kind,
                    mime,
                    url,
                    path,
                    ..
                } if (kind == "image" || mime.starts_with("image/"))
                    && !url.is_empty()
                    && !self.work.preview_attempted.contains(url) =>
                {
                    Some(stepx_companion::openclaw::RealAttachment {
                        name: name.clone(),
                        kind: kind.clone(),
                        mime: mime.clone(),
                        url: url.clone(),
                        path: path.clone(),
                        ..Default::default()
                    })
                }
                _ => None,
            })
            .take(3)
            .collect::<Vec<_>>();
        if !attachments.is_empty() {
            let _ = self.begin_work_previews(attachments);
        }
    }

    pub(super) fn work_previews_loading(&self) -> bool {
        self.work.preview_rx.is_some()
    }

    fn begin_work_previews(
        &mut self,
        attachments: Vec<stepx_companion::openclaw::RealAttachment>,
    ) -> Result<()> {
        let device = self.selected.clone().context("请连接手机后载入图片")?;
        let adb = Adb::discover()?;
        if self.work.preview_cache.is_none() {
            self.work.preview_cache = Some(Arc::new(tempfile::tempdir()?));
        }
        let cache = self
            .work
            .preview_cache
            .clone()
            .context("无法创建图片缓存")?;
        for attachment in &attachments {
            self.work.preview_attempted.insert(attachment.url.clone());
        }
        let (sender, receiver) = mpsc::channel();
        self.work.preview_rx = Some(receiver);
        let expected_scope = self.amoo_ui.account_scope.clone();
        let identities = self.work.preview_identities.clone();
        std::thread::spawn(move || {
            // Recheck the selected phone's account before opening private cached data.
            let scope_matches = || {
                expected_scope.is_none()
                    || stepx_companion::amoo_usage::account_scope(&adb, &device)
                        .ok()
                        .flatten()
                        == expected_scope
            };
            if !scope_matches() {
                for attachment in attachments {
                    let _ = sender.send((
                        attachment.url,
                        Err("手机账号已变化，请重新同步后查看图片".into()),
                    ));
                }
                return;
            }
            let persistent = expected_scope
                .as_ref()
                .and_then(|scope| preview_cache::root().map(|root| (root, scope.clone())));
            for attachment in attachments {
                let output = cache
                    .path()
                    .join(format!("amoo-{:016x}.png", rand::random::<u64>()));
                let stored = persistent.as_ref().and_then(|(root, scope)| {
                    preview_cache::load_with_identity(
                        root,
                        scope,
                        &attachment.url,
                        identities.get(&attachment.url).map(String::as_str),
                    )
                });
                let result = if let Some(path) = stored {
                    Ok(path)
                } else {
                    amoo_media::download_artifact(&adb, &device, &attachment, &output)
                        .map(|file| {
                            persistent
                                .as_ref()
                                .and_then(|(root, scope)| {
                                    preview_cache::save_with_identity(
                                        root,
                                        scope,
                                        &attachment.url,
                                        &file.path,
                                        identities.get(&attachment.url).map(String::as_str),
                                    )
                                    .ok()
                                })
                                .unwrap_or(file.path)
                        })
                        .map_err(|e| format!("{e:#}"))
                };
                let result = if scope_matches() {
                    result
                } else {
                    Err("手机账号已变化，请重新同步后查看图片".into())
                };
                if sender.send((attachment.url, result)).is_err() {
                    break;
                }
            }
        });
        Ok(())
    }

    fn install_work_skill(&mut self) -> Result<()> {
        anyhow::ensure!(!self.busy && !self.work.running, "请等待当前操作完成");
        let (key, installer) = self
            .work
            .pending_install
            .clone()
            .ok_or_else(|| anyhow::anyhow!("请先选择技能安装项"))?;
        self.with_device("正在安装手机技能依赖…", move |adb, device| {
            let catalog = amoo_catalog::install_skill(adb, &device, &key, &installer)?;
            Ok(Outcome::Work(WorkOutcome::CatalogChanged(
                catalog,
                "技能安装完成，已重新读取手机状态".into(),
            )))
        });
        Ok(())
    }

    fn toggle_work_skill(&mut self, key: &str, enabled: bool) -> Result<()> {
        anyhow::ensure!(!self.busy && !self.work.running, "请等待当前操作完成");
        let key = key.to_string();
        self.with_device("正在更新手机技能…", move |adb, device| {
            let catalog = amoo_catalog::set_skill_enabled(adb, &device, &key, enabled)?;
            Ok(Outcome::Work(WorkOutcome::CatalogChanged(
                catalog,
                "已更新手机技能状态".into(),
            )))
        });
        Ok(())
    }
}

fn read_work_preview(root: &std::path::Path, path: &std::path::Path) -> Result<WorkLocalPreview> {
    validate_open_path(Some(root), path)?;
    let workspace = work::Workspace::open(root)?;
    let relative = path
        .strip_prefix(workspace.root())
        .context("文件不在工作文件夹内")?;
    let metadata = std::fs::metadata(path)?;
    anyhow::ensure!(metadata.is_file(), "请选择普通文件预览");
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    let image = matches!(
        extension.as_str(),
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp"
    );
    let mut preview = WorkLocalPreview {
        path: path.to_path_buf(),
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        bytes: metadata.len(),
        image,
        text: None,
        markdown: matches!(extension.as_str(), "md" | "markdown" | "csv" | "tsv"),
        note: String::new(),
    };
    if metadata.len() > 32 * 1024 * 1024 {
        preview.image = false;
        preview.note = "文件大于 32 MB，可使用本地应用打开或导出。".into();
    } else if image {
        let (width, height) = image::image_dimensions(path).context("图片无法解码")?;
        anyhow::ensure!(
            u64::from(width) * u64::from(height) <= 32_000_000,
            "图片分辨率超过预览上限，请打开原图查看"
        );
        preview.note = format!("{width} × {height} · 原始图片");
    } else if matches!(extension.as_str(), "html" | "htm") {
        preview.note =
            "网页或小游戏已保存。点击“在浏览器运行”打开实际交互；源文件保留在工作文件夹内。".into();
    } else {
        match workspace.read_text(&relative.to_string_lossy()) {
            Ok(text) => {
                if matches!(extension.as_str(), "csv" | "tsv") {
                    preview.text = Some(delimited_preview(
                        &text,
                        if extension == "tsv" { '\t' } else { ',' },
                    ));
                    preview.note = "表格预览 · 最多显示 20 行、8 列，完整内容可打开原文件。".into();
                } else if extension == "pptx" {
                    preview.markdown = true;
                    preview.text = Some(
                        text.lines()
                            .map(|line| {
                                if let Some(page) = line.strip_prefix("幻灯片: ") {
                                    format!("\n### 第 {page} 页\n")
                                } else {
                                    line.to_owned()
                                }
                            })
                            .collect::<Vec<_>>()
                            .join("\n"),
                    );
                    preview.note = "逐页内容预览 · 点击打开原文件查看排版与编辑幻灯片。".into();
                } else {
                    let truncated = text.chars().count() > 12_000;
                    preview.text = Some(text.chars().take(12_000).collect());
                    preview.note = match extension.as_str() {
                        "docx" | "xlsx" | "pptx" | "pdf" => {
                            "文档文本预览 · 原始排版请打开文件查看。".into()
                        }
                        _ if truncated => "内容较长，仅显示前 12,000 字符。".into(),
                        _ => "文件内容预览".into(),
                    };
                }
            }
            Err(error) => {
                preview.note = format!("此文件暂不能内嵌预览：{error}。可打开原文件或导出。")
            }
        }
    }
    Ok(preview)
}

/// Parse quoted CSV/TSV fields without evaluating formulas or embedding HTML.
fn delimited_preview(source: &str, delimiter: char) -> String {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = source.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' if quoted && chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            '"' if quoted || field.is_empty() => quoted = !quoted,
            c if c == delimiter && !quoted => row.push(std::mem::take(&mut field)),
            '\n' if !quoted => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
                if rows.len() >= 21 {
                    break;
                }
            }
            '\r' if !quoted => {}
            c => field.push(c),
        }
    }
    if rows.len() < 21 && (!field.is_empty() || !row.is_empty()) {
        row.push(field);
        rows.push(row);
    }
    let columns = rows.iter().map(Vec::len).max().unwrap_or(1).min(8);
    let render = |row: &[String]| -> String {
        (0..columns)
            .map(|index| {
                row.get(index)
                    .map(|cell| {
                        cell.chars()
                            .take(160)
                            .collect::<String>()
                            .replace('\\', "\\\\")
                            .replace('|', "\\|")
                            .replace(['\r', '\n'], " ")
                            .replace('`', "\\`")
                            .replace('*', "\\*")
                            .replace('[', "\\[")
                            .replace(']', "\\]")
                            .replace('<', "&lt;")
                    })
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join(" | ")
    };
    let Some(header) = rows.first() else {
        return "空表格".into();
    };
    let mut markdown = format!(
        "| {} |\n| {} |\n",
        render(header),
        vec!["---"; columns].join(" | ")
    );
    for row in rows.iter().skip(1).take(20) {
        markdown.push_str(&format!("| {} |\n", render(row)));
    }
    markdown
}

fn validate_open_path(root: Option<&std::path::Path>, path: &std::path::Path) -> Result<()> {
    let root = root
        .ok_or_else(|| anyhow::anyhow!("请先选择工作文件夹"))?
        .canonicalize()?;
    anyhow::ensure!(
        path.canonicalize()?.starts_with(&root),
        "该文件已移出当前工作文件夹，请刷新"
    );
    let relative = path
        .strip_prefix(&root)
        .context("文件不在当前工作文件夹内")?;
    let mut candidate = root;
    for component in relative.components() {
        anyhow::ensure!(
            matches!(component, std::path::Component::Normal(_)),
            "文件路径无效，请刷新工作资料"
        );
        candidate.push(component);
        anyhow::ensure!(
            !std::fs::symlink_metadata(&candidate)?
                .file_type()
                .is_symlink(),
            "工作预览不跟随符号链接"
        );
    }
    Ok(())
}

#[cfg(test)]
mod preview_tests {
    use super::*;
    #[test]
    fn device_disconnect_drops_previous_session_results_and_inflight_previews() {
        let workspace = tempfile::tempdir().unwrap();
        let document = workspace.path().join("private-result.md");
        std::fs::write(&document, "# Previous phone's private result").unwrap();
        let mut state = WorkUiState::default();
        state.workspace = Some(workspace.path().to_path_buf());
        state.files.push(WorkFileItem {
            path: document.clone(),
            name: "private-result.md".into(),
            bytes: std::fs::metadata(&document).unwrap().len(),
            is_dir: false,
        });
        state.local_preview = Some(read_work_preview(workspace.path(), &document).unwrap());
        state.local_preview_loading = Some(document.clone());
        state.local_preview_error = Some("Previous phone preview error".into());
        state.remote_preview_url = Some("https://example.test/previous-phone.png".into());
        state.artifacts.push(WorkArtifactItem {
            path: document.clone(),
            name: "private-result.md".into(),
            kind: "document".into(),
        });
        state.events.push("Previous phone task content".into());
        state.attachments.push(document.clone());
        state.active_key = Some("previous-phone-session".into());
        state.submitted_draft = Some("Previous phone task draft".into());
        state.error = Some("Previous phone task failure".into());
        state.running = true;
        let url = "https://example.test/previous-phone.png".to_string();
        state.image_previews.insert(url.clone(), document.clone());
        state.preview_attempted.insert(url.clone());
        state.preview_errors.insert(url.clone(), "expired".into());
        state
            .preview_identities
            .insert(url.clone(), "previous-session/message/0".into());
        state.preview_cache = Some(Arc::new(tempfile::tempdir().unwrap()));
        let (preview_tx, preview_rx) = mpsc::channel();
        state.preview_rx = Some(preview_rx);
        let (event_tx, event_rx) = mpsc::channel();
        let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
        state.handle = Some(work::WorkHandle {
            events: event_rx,
            cancel: cancel.clone(),
        });

        state.disconnect();

        // Disconnect must both erase visible results and detach producers, so
        // a late completion from the previous phone cannot repopulate them.
        assert!(cancel.load(Ordering::SeqCst));
        assert!(event_tx
            .send(work::WorkEvent::Complete("stale".into()))
            .is_err());
        assert!(preview_tx
            .send((url.clone(), Ok(document.clone())))
            .is_err());
        assert!(!state.running && state.handle.is_none() && state.preview_rx.is_none());
        assert!(state.local_preview.is_none());
        assert!(state.local_preview_loading.is_none());
        assert!(state.local_preview_error.is_none());
        assert!(state.remote_preview_url.is_none());
        assert!(
            state.artifacts.is_empty() && state.events.is_empty() && state.attachments.is_empty()
        );
        assert!(
            state.active_key.is_none() && state.submitted_draft.is_none() && state.error.is_none()
        );
        assert!(!state.image_previews.contains_key(&url));
        assert!(state.preview_attempted.is_empty() && state.preview_errors.is_empty());
        assert!(state.preview_identities.is_empty() && state.preview_cache.is_none());

        // The explicitly selected computer workspace and its files survive;
        // disconnect is neither a file deletion nor a workspace reset.
        assert_eq!(state.workspace.as_deref(), Some(workspace.path()));
        assert_eq!(state.files.len(), 1);
        assert_eq!(state.files[0].path, document);
        assert_eq!(
            std::fs::read_to_string(&document).unwrap(),
            "# Previous phone's private result"
        );
    }
    #[test]
    fn csv_preview_handles_quoted_delimiters_and_multiline_cells() {
        let table = delimited_preview(
            "名称,说明\r\n甲,\"带,逗号\"\r\n乙,\"第一行\n第二行\"\n丙,\"a\"\"b\"",
            ',',
        );
        assert!(table.contains("| 甲 | 带,逗号 |"));
        assert!(table.contains("| 乙 | 第一行 第二行 |"));
        assert!(table.contains("a\"b"));
    }
    #[test]
    fn local_previews_stay_in_workspace_and_do_not_execute_content() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let text = root.path().join("任务.md");
        std::fs::write(&text, "# 真实文档\n\n**完成**").unwrap();
        let result = read_work_preview(root.path(), &text).unwrap();
        assert!(result.markdown && result.text.unwrap().contains("**完成**"));
        let other = outside.path().join("secret.txt");
        std::fs::write(&other, "private").unwrap();
        assert!(read_work_preview(root.path(), &other).is_err());
        let binary = root.path().join("unknown.bin");
        std::fs::write(&binary, [0, 1, 2]).unwrap();
        assert!(read_work_preview(root.path(), &binary)
            .unwrap()
            .text
            .is_none());
    }
    #[test]
    #[cfg(unix)]
    fn previews_reject_symlinks_even_when_target_is_inside_workspace() {
        let root = tempfile::tempdir().unwrap();
        let text = root.path().join("note.md");
        std::fs::write(&text, "private").unwrap();
        let linked = root.path().join("linked.md");
        std::os::unix::fs::symlink(text, &linked).unwrap();
        assert!(read_work_preview(root.path(), &linked).is_err());
    }
}
fn open_work_path(path: &std::path::Path) -> Result<()> {
    let command = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    std::process::Command::new(command).arg(path).spawn()?;
    Ok(())
}

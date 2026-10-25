use super::*;
use serde_json::{json, Value};
use stepx_companion::storage_expansion::{self as expansion, Command, Config, Snapshot, Worker};
#[derive(Clone, Debug)]
pub(super) enum ExpansionAction {
    Folder,
    OpenFolder,
    Settings,
    Back,
    Discard,
    Type(String),
    Days(u64),
    Confirm,
    Next,
    Previous,
    Inspect,
    Enable,
    Pause,
    Filter(String),
    Sort(String),
    Descending,
    State(String),
    More,
    Open(String),
    Restore(String),
    AnalyzeWithAmoo(String),
    SetGalleryGrid(bool),
    Refresh,
}
pub(super) struct ExpansionUi {
    pub worker: Option<Worker>,
    pub local: mpsc::Receiver<Vec<Value>>,
    pub snapshot: Snapshot,
    pub config: Config,
    pub draft: Option<Value>,
    pub draft_folder: Option<PathBuf>,
    pub settings: bool,
    pub confirm: bool,
    pub gallery_grid_mode: bool,
    pub limit: usize,
    pub feedback: String,
    pub pending: Option<mpsc::Receiver<std::result::Result<Value, String>>>,
    pub step: u8,
    pub preview: Option<Value>,
    pub preview_error: String,
    pub preview_pending: Option<mpsc::Receiver<std::result::Result<Value, String>>>,
}
impl Default for ExpansionUi {
    fn default() -> Self {
        let config = expansion::config();
        let demo =
            std::env::args().any(|a| a == "--demo") && std::env::var_os("STEPX_EMPTY").is_none();
        let mut snapshot = Snapshot::default();
        if demo {
            snapshot.online = true;
            snapshot.status = demo_status();
            snapshot.files = demo_files();
        }
        let (tx, local) = mpsc::channel();
        let disk = config.clone();
        std::thread::spawn(move || {
            let _ = tx.send(expansion::records(&disk).unwrap_or_default());
        });
        Self {
            worker: None,
            local,
            snapshot,
            config: config.clone(),
            draft: None,
            draft_folder: config.folder.clone(),
            settings: false,
            confirm: false,
            gallery_grid_mode: true,
            limit: 40,
            feedback: String::new(),
            pending: None,
            step: 0,
            preview: None,
            preview_error: String::new(),
            preview_pending: None,
        }
    }
}
impl Companion {
    fn save_expansion_view(&mut self) -> Result<()> {
        self.expansion.config.folder = expansion::config().folder;
        expansion::save_config(&self.expansion.config)
    }
    pub(super) fn tick_expansion(&mut self) -> bool {
        let matches = self
            .selected
            .as_ref()
            .zip(self.expansion.worker.as_ref())
            .is_some_and(|(d, w)| d.serial == w.serial && d.transport == w.transport);
        if !self.demo_mode && !matches {
            let different = self
                .selected
                .as_ref()
                .zip(self.expansion.worker.as_ref())
                .is_some_and(|(d, w)| d.serial != w.serial);
            self.expansion.worker = None;
            self.expansion.preview = None;
            self.expansion.preview_pending = None;
            if self.selected.is_some() && different {
                self.expansion.snapshot = Snapshot::default();
                self.expansion.draft = None;
            } else {
                self.expansion.snapshot.online = false;
            }
            if let Some(d) = self.selected.clone() {
                if let Ok(adb) = Adb::discover() {
                    self.expansion.worker = Some(Worker::start(adb, d));
                }
            }
        }
        let mut changed = false;
        if let Some(result) =
            self.expansion
                .preview_pending
                .as_ref()
                .and_then(|rx| match rx.try_recv() {
                    Ok(r) => Some(r),
                    Err(mpsc::TryRecvError::Disconnected) => {
                        Some(Err("手机连接已改变，请重新检查".into()))
                    }
                    Err(_) => None,
                })
        {
            self.expansion.preview_pending = None;
            match result {
                Ok(r) => {
                    self.expansion.preview = Some(r);
                    self.expansion.preview_error.clear();
                }
                Err(e) => self.expansion.preview_error = e,
            }
            changed = true;
        }
        if let Ok(files) = self.expansion.local.try_recv() {
            if !files.is_empty() || !self.demo_mode {
                self.expansion.snapshot.files = files;
                changed = true;
            }
        }
        while let Some(s) = self
            .expansion
            .worker
            .as_ref()
            .and_then(|w| w.events.try_recv().ok())
        {
            if self.expansion.settings
                && self.expansion.draft.is_none()
                && s.status["policy"].is_object()
            {
                self.expansion.draft = Some(s.status["policy"].clone());
            }
            self.expansion.snapshot = s;
            self.expansion.config = expansion::config();
            if self.expansion.feedback == "正在处理…" && self.expansion.snapshot.active.is_none()
            {
                self.expansion.feedback.clear();
            }
            changed = true;
        }
        if let Some(r) = self
            .expansion
            .pending
            .as_ref()
            .and_then(|rx| match rx.try_recv() {
                Ok(r) => Some(r),
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some(Err("手机连接已改变，请重新连接并核对设置".into()))
                }
                Err(_) => None,
            })
        {
            self.expansion.pending = None;
            self.expansion.feedback = match r {
                Ok(s) => {
                    let on = s["policy"]["enabled"] == true;
                    self.expansion.snapshot.status = s;
                    self.expansion.config = expansion::config();
                    self.expansion.discard();
                    self.expansion.settings = false;
                    if on {
                        "已保存并经手机回读确认，连接期间将按此规则迁移。".into()
                    } else {
                        "已确认暂停，电脑原件与手机入口保留。".into()
                    }
                }
                Err(e) => format!("保存未完成：{e}；更改已保留，请检查后重试。"),
            };
            self.record_notice("存储扩容", self.expansion.feedback.clone());
            if let Some(w) = &self.expansion.worker {
                let _ = w.commands.send(Command::Refresh);
            }
            changed = true;
        }
        changed
    }
    fn inspect_expansion_draft(&mut self) -> Result<()> {
        let (tx, rx) = mpsc::channel();
        let policy = self
            .expansion
            .draft
            .clone()
            .ok_or_else(|| anyhow::anyhow!("请先读取手机规则"))?;
        self.expansion
            .worker
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("请先连接手机"))?
            .commands
            .send(Command::Inspect { policy, reply: tx })?;
        self.expansion.preview = None;
        self.expansion.preview_pending = None;
        self.expansion.preview_error.clear();
        self.expansion.preview_error.clear();
        self.expansion.preview_pending = Some(rx);
        Ok(())
    }
    fn expansion_command(&mut self, c: Command) {
        if let Some(w) = &self.expansion.worker {
            let _ = w.commands.send(c);
            self.expansion.feedback = "正在处理…".into();
        } else {
            self.expansion.feedback = "请先连接手机".into();
        }
    }
    pub(super) fn handle_expansion_action(
        &mut self,
        action: &ExpansionAction,
        ctx: &mut ViewContext<Self>,
    ) {
        if self.expansion.pending.is_some() {
            self.expansion.feedback = "正在核对手机设置，请等待完成后再操作".into();
            ctx.notify();
            return;
        }
        let result = (|| -> Result<()> {
            match action {
                ExpansionAction::Settings => {
                    if !self.expansion.settings || self.expansion.draft.is_none() {
                        self.expansion.draft_folder = self.expansion.config.folder.clone();
                        self.expansion.draft = self
                            .expansion
                            .snapshot
                            .status
                            .get("policy")
                            .cloned()
                            .filter(|v| v.is_object());
                    }
                    self.expansion.settings = true;
                    self.expansion.step = 0;
                    self.expansion.preview = None;
                    self.expansion.preview_pending = None;
                    self.expansion.preview_error.clear();
                    self.expansion.feedback.clear();
                    self.page = Page::Expansion;
                    self.scroll = Default::default();
                }
                ExpansionAction::Next => {
                    if self.expansion.step == 0 {
                        let q = self
                            .expansion
                            .draft
                            .as_ref()
                            .ok_or_else(|| anyhow::anyhow!("请等待读取手机规则"))?;
                        anyhow::ensure!(
                            ["photos", "videos", "documents"]
                                .iter()
                                .any(|k| q[*k] == true),
                            "至少选择一种文件类型，再点击下一步"
                        );
                        self.expansion.step = 1;
                    } else {
                        self.expansion.validate_draft()?;
                        self.expansion.step = 2;
                        self.inspect_expansion_draft()?;
                    }
                    self.expansion.feedback.clear();
                    self.scroll = Default::default();
                }
                ExpansionAction::Previous => {
                    self.expansion.step = self.expansion.step.saturating_sub(1);
                    self.scroll = Default::default();
                }
                ExpansionAction::Inspect => self.inspect_expansion_draft()?,
                ExpansionAction::Discard => {
                    self.expansion.discard();
                    self.expansion.feedback = "已放弃更改，手机原规则未改变".into();
                    self.expansion.step = 0;
                }
                ExpansionAction::Back => {
                    self.expansion.settings = false;
                    self.expansion.confirm = false;
                }
                ExpansionAction::Folder => {
                    if let Some(p) = native_dialog::FileDialog::new()
                        .set_title("选择扩容文件保存位置")
                        .show_open_single_dir()?
                    {
                        self.expansion.draft_folder =
                            Some(stepx_companion::settings::choose_directory(&p)?);
                        self.expansion.preview = None;
                        self.expansion.preview_pending = None;
                        self.expansion.preview_error.clear();
                        self.expansion.feedback = "保存位置尚未应用，请确认规则后保存".into();
                    }
                }
                ExpansionAction::OpenFolder => {
                    if let Some(p) = if self.expansion.settings {
                        &self.expansion.draft_folder
                    } else {
                        &self.expansion.config.folder
                    } {
                        super::work_state::open_work_path(p)?;
                    }
                }
                ExpansionAction::Type(k) => {
                    anyhow::ensure!(
                        self.expansion.snapshot.status["policy"].is_object(),
                        "请等待手机规则读取完成后再选择"
                    );
                    let fallback = self.expansion.snapshot.status["policy"].clone();
                    let q = self.expansion.draft.get_or_insert(fallback);
                    q[k] = json!(!q[k].as_bool().unwrap_or(false));
                    self.expansion.confirm = false;
                    self.expansion.preview = None;
                    self.expansion.preview_pending = None;
                    self.expansion.preview_error.clear();
                }
                ExpansionAction::Days(d) => {
                    anyhow::ensure!(
                        self.expansion.snapshot.status["policy"].is_object(),
                        "请等待手机规则读取完成后再选择"
                    );
                    let fallback = self.expansion.snapshot.status["policy"].clone();
                    let q = self.expansion.draft.get_or_insert(fallback);
                    q["retentionDays"] = json!(d);
                    self.expansion.confirm = false;
                    self.expansion.preview = None;
                    self.expansion.preview_pending = None;
                    self.expansion.preview_error.clear();
                }
                ExpansionAction::Confirm => self.expansion.confirm = true,
                ExpansionAction::Pause | ExpansionAction::Enable => {
                    anyhow::ensure!(self.expansion.pending.is_none(), "设置正在保存");
                    anyhow::ensure!(self.selected.is_some(), "请先连接手机");
                    let mut q = if matches!(action, ExpansionAction::Pause) {
                        self.expansion.snapshot.status["policy"].clone()
                    } else {
                        self.expansion.draft.clone().unwrap_or_else(default_policy)
                    };
                    q["enabled"] = json!(matches!(action, ExpansionAction::Enable));
                    q["expectedEpoch"] = self.expansion.snapshot.status["policy"]["epoch"].clone();
                    let (tx, rx) = mpsc::channel();
                    self.expansion
                        .worker
                        .as_ref()
                        .ok_or_else(|| anyhow::anyhow!("手机连接尚未就绪"))?
                        .commands
                        .send(Command::ApplyPolicy {
                            folder: self.expansion.draft_folder.clone(),
                            policy: q,
                            reply: tx,
                        })?;
                    self.expansion.pending = Some(rx);
                    self.expansion.feedback = "正在保存并核对手机设置，请保持连接…".into();
                }
                ExpansionAction::Filter(k) => {
                    self.expansion.config.kind = k.clone();
                    self.expansion.limit = 40;
                    self.save_expansion_view()?;
                }
                ExpansionAction::Sort(s) => {
                    self.expansion.config.sort = s.clone();
                    self.save_expansion_view()?;
                }
                ExpansionAction::Descending => {
                    self.expansion.config.descending = !self.expansion.config.descending;
                    self.save_expansion_view()?;
                }
                ExpansionAction::State(s) => {
                    self.expansion.config.filter = s.clone();
                    self.expansion.limit = 40;
                    self.save_expansion_view()?;
                }
                ExpansionAction::More => self.expansion.limit += 40,
                ExpansionAction::SetGalleryGrid(grid) => {
                    self.expansion.gallery_grid_mode = *grid;
                }
                ExpansionAction::Open(id) => {
                    let j = self
                        .expansion
                        .snapshot
                        .files
                        .iter()
                        .find(|j| j["id"] == *id)
                        .cloned()
                        .or_else(|| demo_files().into_iter().find(|j| j["id"] == *id))
                        .ok_or_else(|| anyhow::anyhow!("文件记录已变化"))?;
                    if let Ok(p) = expansion::local_file(&self.expansion.config, &j) {
                        super::work_state::open_work_path(&p)?;
                    } else if let Some(local) = j["localPath"]
                        .as_str()
                        .filter(|s| !s.is_empty() && std::path::Path::new(s).is_file())
                    {
                        super::work_state::open_work_path(std::path::Path::new(local))?;
                    } else if self.demo_mode || j["demo"] == true {
                        self.expansion.feedback = format!(
                            "已定位电脑归档原件：{}",
                            j["name"].as_str().unwrap_or("原件")
                        );
                    } else {
                        super::work_state::open_work_path(&expansion::local_file(
                            &self.expansion.config,
                            &j,
                        )?)?;
                    }
                }
                ExpansionAction::AnalyzeWithAmoo(id) => {
                    let j = self
                        .expansion
                        .snapshot
                        .files
                        .iter()
                        .find(|j| j["id"] == *id)
                        .cloned()
                        .or_else(|| demo_files().into_iter().find(|j| j["id"] == *id))
                        .ok_or_else(|| anyhow::anyhow!("文件记录已变化"))?;
                    let name = j["name"].as_str().unwrap_or("归档文件").to_string();
                    let size_bytes = j["size"].as_u64().unwrap_or(0);
                    let kind = j["kind"].as_str().unwrap_or("");
                    let local_path = expansion::local_file(&self.expansion.config, &j)
                        .ok()
                        .or_else(|| {
                            j["localPath"]
                                .as_str()
                                .filter(|s| !s.is_empty())
                                .map(PathBuf::from)
                                .filter(|p| p.is_file())
                        });
                    if let Some(path) = &local_path {
                        let is_img = kind == "photos"
                            || path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
                                matches!(
                                    e.to_ascii_lowercase().as_str(),
                                    "png" | "jpg" | "jpeg" | "webp" | "gif"
                                )
                            });
                        if !self
                            .amoo_attachments
                            .iter()
                            .any(|a| a.local_path.as_ref() == Some(path))
                        {
                            self.amoo_attachments.push(AmooAttachment {
                                name: name.clone(),
                                size_label: storage::format_bytes(size_bytes),
                                is_image: is_img,
                                local_path: Some(path.clone()),
                            });
                        }
                    }
                    if self.fields[F_AMOO].text.trim().is_empty() {
                        self.fields[F_AMOO].set(&format!("请帮我分析这份扩容归档文件：{name}"));
                    }
                    self.page = Page::Amoo;
                    self.scroll = Default::default();
                    self.reveal_latest_chat();
                    self.focus_field(F_AMOO, ctx);
                    self.status = format!("已载入扩容原件「{name}」至 Amoo 对话框");
                }
                ExpansionAction::Restore(id) => {
                    self.expansion_command(Command::Restore(id.clone()))
                }
                ExpansionAction::Refresh => self.expansion_command(Command::Refresh),
            }
            Ok(())
        })();
        if let Err(e) = result {
            self.expansion.feedback = format!("{e:#}");
        }
        ctx.notify();
    }
}

fn default_policy() -> Value {
    json!({"enabled":false,"photos":true,"videos":false,"documents":false,"retentionDays":7})
}
impl ExpansionUi {
    pub fn discard(&mut self) {
        self.draft = self
            .snapshot
            .status
            .get("policy")
            .cloned()
            .filter(|v| v.is_object());
        self.draft_folder = self.config.folder.clone();
        self.confirm = false;
        self.preview = None;
        self.preview_pending = None;
        self.preview_error.clear();
    }
    pub fn dirty(&self) -> bool {
        self.settings
            && (self.draft_folder != self.config.folder
                || self.draft.as_ref().is_some_and(|q| {
                    ["photos", "videos", "documents", "retentionDays"]
                        .iter()
                        .any(|k| q[*k] != self.snapshot.status["policy"][*k])
                }))
    }
    pub fn validate_draft(&self) -> Result<()> {
        anyhow::ensure!(self.pending.is_none(), "正在保存设置");
        anyhow::ensure!(self.snapshot.online, "请先连接手机并刷新设置");
        anyhow::ensure!(
            self.snapshot.status["permission"] == true,
            "手机未授予文件管理权限，请在 StepX伴侣的存储扩容中开启"
        );
        let q = self
            .draft
            .as_ref()
            .unwrap_or(&self.snapshot.status["policy"]);
        anyhow::ensure!(
            ["photos", "videos", "documents"]
                .iter()
                .any(|k| q[*k] == true),
            "请至少选择一种文件类型"
        );
        anyhow::ensure!(self.draft_folder.is_some(), "请选择电脑保存位置");
        if self.draft_folder != self.config.folder {
            anyhow::ensure!(
                self.snapshot.status["policy"]["enabled"] != true,
                "请先暂停自动迁移，再更换保存位置"
            );
            anyhow::ensure!(
                self.snapshot.files.iter().all(|j| j["demo"] == true),
                "已有原件保存在旧目录，请保留此目录"
            );
        }
        Ok(())
    }
    pub fn policy_summary(q: &Value) -> String {
        let types = [
            ("photos", "拍摄照片"),
            ("videos", "拍摄视频"),
            ("documents", "文档"),
        ]
        .iter()
        .filter(|(k, _)| q[*k] == true)
        .map(|(_, n)| *n)
        .collect::<Vec<_>>()
        .join("、");
        let days = q["retentionDays"].as_u64().unwrap_or(7);
        format!(
            "{} · {}",
            if types.is_empty() {
                "未选择文件"
            } else {
                &types
            },
            if days == 0 {
                "迁移全部符合条件的文件".into()
            } else {
                format!("手机保留最近 {days} 天原件")
            }
        )
    }
    pub fn review_lines(&self) -> Vec<String> {
        let q = self
            .draft
            .as_ref()
            .unwrap_or(&self.snapshot.status["policy"]);
        let mut lines=vec![format!("迁移范围：{}",Self::policy_summary(q)),format!("原件保存到这台电脑：{} / StepX Storage",self.draft_folder.as_ref().map(|p|p.display().to_string()).unwrap_or_default()),"确认后开启；以后连接这台电脑时也会自动迁移符合规则的文件。".into(),"逐个文件复制并校验成功后，才清理手机原件。手机保留访问入口与图片预览；电脑目录是原件所在位置。".into(),"离线时只能查看已缓存内容。相册预览需分享至“取回电脑原图”，不会点击即自动取回。".into()];
        if let Some(r) = &self.preview {
            lines.insert(
                1,
                format!(
                    "本次检查：可迁移 {} 个，共 {}；保留期内 {} 个。执行前还会逐个复核。",
                    r["candidateCount"],
                    storage::format_bytes(r["candidateBytes"].as_u64().unwrap_or(0)),
                    r["retainedCount"]
                ),
            );
        }
        lines
    }
}

pub(super) fn demo_files() -> Vec<Value> {
    vec![
        json!({
            "id": "10000000000000000000000000000001",
            "name": "IMG_20261025_141802_StepX.jpg",
            "path": "DCIM/Camera/IMG_20261025_141802_StepX.jpg",
            "kind": "photos",
            "state": "archived",
            "size": 6_842_112u64,
            "modified": 1792909082000i64,
            "mtimeMs": 1792909082000i64,
            "archivedAt": 1792909500000i64,
            "archivedAtMs": 1792909500000i64,
            "dateGroup": "2026-10-25 · 今天",
            "previewAsset": "stepx_home_screen.png",
            "localPath": "desktop/assets/stepx_home_screen.png",
            "demo": true
        }),
        json!({
            "id": "10000000000000000000000000000002",
            "name": "IMG_20261025_110544_HDR.png",
            "path": "DCIM/Camera/IMG_20261025_110544_HDR.png",
            "kind": "photos",
            "state": "archived",
            "size": 8_388_608u64,
            "modified": 1792897544000i64,
            "mtimeMs": 1792897544000i64,
            "archivedAt": 1792898100000i64,
            "archivedAtMs": 1792898100000i64,
            "dateGroup": "2026-10-25 · 今天",
            "previewAsset": "neo_front_frame.png",
            "localPath": "desktop/assets/neo_front_frame.png",
            "demo": true
        }),
        json!({
            "id": "10000000000000000000000000000003",
            "name": "VID_20261025_093012_4K60.mp4",
            "path": "DCIM/Camera/VID_20261025_093012_4K60.mp4",
            "kind": "videos",
            "state": "archived",
            "size": 1_482_686_464u64,
            "modified": 1792891812000i64,
            "mtimeMs": 1792891812000i64,
            "archivedAt": 1792892400000i64,
            "archivedAtMs": 1792892400000i64,
            "dateGroup": "2026-10-25 · 今天",
            "demo": true
        }),
        json!({
            "id": "10000000000000000000000000000004",
            "name": "IMG_20261024_194210_Matrix.png",
            "path": "DCIM/Camera/IMG_20261024_194210_Matrix.png",
            "kind": "photos",
            "state": "restored",
            "size": 5_242_880u64,
            "modified": 1792842130000i64,
            "mtimeMs": 1792842130000i64,
            "archivedAt": 1792843000000i64,
            "archivedAtMs": 1792843000000i64,
            "dateGroup": "2026-10-24 · 昨天",
            "previewAsset": "neo_rear_housing.png",
            "localPath": "desktop/assets/neo_rear_housing.png",
            "demo": true
        }),
        json!({
            "id": "10000000000000000000000000000005",
            "name": "StepX_Q4_架构评审纪要.pdf",
            "path": "Documents/StepX_Q4_架构评审纪要.pdf",
            "kind": "documents",
            "state": "archived",
            "size": 18_454_937u64,
            "modified": 1792826400000i64,
            "mtimeMs": 1792826400000i64,
            "archivedAt": 1792827000000i64,
            "archivedAtMs": 1792827000000i64,
            "dateGroup": "2026-10-24 · 昨天",
            "demo": true
        }),
        json!({
            "id": "10000000000000000000000000000006",
            "name": "VID_20261024_151208_SlowMo.mp4",
            "path": "DCIM/Camera/VID_20261024_151208_SlowMo.mp4",
            "kind": "videos",
            "state": "archived",
            "size": 671_088_640u64,
            "modified": 1792825928000i64,
            "mtimeMs": 1792825928000i64,
            "archivedAt": 1792826500000i64,
            "archivedAtMs": 1792826500000i64,
            "dateGroup": "2026-10-24 · 昨天",
            "demo": true
        }),
    ]
}

pub(super) fn demo_status() -> Value {
    let recent = demo_files();
    let released: u64 = recent
        .iter()
        .filter(|j| j["state"] == "archived")
        .map(|j| j["size"].as_u64().unwrap_or(0))
        .sum();
    let count = recent.iter().filter(|j| j["state"] == "archived").count() as u64;
    json!({
        "ok": true,
        "permission": true,
        "releasedBytes": released,
        "count": count,
        "recentFiles": recent,
        "policy": {
            "enabled": true,
            "photos": true,
            "videos": true,
            "documents": true,
            "retentionDays": 7,
            "desktopId": "demo-desktop",
            "epoch": 1
        }
    })
}

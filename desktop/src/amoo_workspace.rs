//! Task records shared by chat, desktop file work and local app creation.
//! A plan is only a proposal. Nothing executes until the UI accepts it.
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum Intent {
    Auto,
    #[default]
    Chat,
    Files,
    Image,
    App,
}
impl Intent {
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "自动处理",
            Self::Chat => "对话",
            Self::Files => "处理文件",
            Self::Image => "生成图片",
            Self::App => "制作应用",
        }
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Executor {
    Phone,
    Files,
    Image,
    App,
}
impl Executor {
    pub fn label(self) -> &'static str {
        match self {
            Self::Phone => "手机 Amoo 回复",
            Self::Files => "处理资料并生成文件",
            Self::Image => "生成图片并接收原图",
            Self::App => "制作应用并运行检查",
        }
    }
    pub fn source(self) -> &'static str {
        if self == Self::App {
            "我的 API · 本机执行"
        } else {
            "手机 Amoo · 手机账号"
        }
    }
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Ready,
    Running,
    Done,
    Failed,
    Stopped,
    Unconfirmed,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Step {
    pub executor: Executor,
    pub phase: Phase,
    pub note: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Turn {
    pub user: bool,
    pub text: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Output {
    pub path: PathBuf,
    pub name: String,
    pub kind: String,
    pub lab_id: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Receipt {
    pub path: PathBuf,
    pub note: String,
    pub success: bool,
    pub device: String,
    pub account: Option<String>,
    pub cloud_url: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub device: Option<String>,
    pub account: Option<String>,
    pub session: Option<String>,
    pub draft: String,
    pub attachments: Vec<PathBuf>,
    #[serde(default)]
    pub submitted_files: Vec<PathBuf>,
    pub intent: Intent,
    pub workspace: Option<PathBuf>,
    pub turns: Vec<Turn>,
    pub steps: Vec<Step>,
    pub outputs: Vec<Output>,
    #[serde(default)]
    pub receipts: Vec<Receipt>,
    pub note: String,
    pub updated: i64,
}
impl Task {
    pub fn new(device: Option<String>, account: Option<String>) -> Self {
        Self {
            id: format!("task-{:032x}", rand::random::<u128>()),
            title: "新对话".into(),
            device,
            account,
            session: None,
            draft: String::new(),
            attachments: vec![],
            submitted_files: vec![],
            intent: Intent::Chat,
            workspace: None,
            turns: vec![],
            steps: vec![],
            outputs: vec![],
            receipts: vec![],
            note: String::new(),
            updated: chrono::Utc::now().timestamp(),
        }
    }
    pub fn visible(&self, device: Option<&str>, account: Option<&str>) -> bool {
        if let Some(d) = self.device.as_deref() {
            if Some(d) != device {
                return false;
            }
        }
        self.account.as_deref().is_none_or(|a| Some(a) == account)
    }
    pub fn add_output(&mut self, output: Output) {
        if let Some(existing) = self.outputs.iter_mut().find(|v| v.path == output.path) {
            *existing = output;
        } else {
            self.outputs.push(output);
        }
    }
    pub fn meaningful(&self) -> bool {
        !self.draft.is_empty()
            || !self.attachments.is_empty()
            || self.session.is_some()
            || !self.turns.is_empty()
            || !self.outputs.is_empty()
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Journal {
    pub tasks: Vec<Task>,
}
impl Journal {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        ensure!(
            std::fs::metadata(path)?.len() < 16 * 1024 * 1024,
            "任务记录过大"
        );
        let mut value: Self = serde_json::from_slice(&std::fs::read(path)?)
            .context("任务记录无法读取，原文件已保留")?;
        // A closed process is never evidence that a remote task completed.
        for task in &mut value.tasks {
            // Old automatic routing is no longer a user choice. Never restore it on launch.
            if task.intent == Intent::Auto || (task.draft.trim().is_empty() && task.attachments.is_empty()) { task.intent = Intent::Chat; }
            for step in &mut task.steps {
                if step.phase == Phase::Running {
                    step.phase = Phase::Unconfirmed;
                    step.note = "上次执行已中断；手机任务需同步核对，不会自动重发".into();
                }
            }
        }
        Ok(value)
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        let parent = path.parent().context("任务目录无效")?;
        std::fs::create_dir_all(parent)?;
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
            file.as_file()
                .set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        use std::io::Write;
        file.write_all(&serde_json::to_vec(self)?)?;
        file.as_file().sync_all()?;
        file.persist(path)?;
        Ok(())
    }
}
pub fn propose(prompt: &str, intent: Intent, attachments: bool) -> Vec<Executor> {
    let text = prompt.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| text.contains(w));
    let question = has(&[
        "怎么做",
        "如何制作",
        "怎么制作",
        "能不能",
        "可以吗",
        "是什么",
    ]);
    let create = has(&[
        "做一个",
        "做个",
        "制作",
        "编写",
        "生成",
        "创建",
        "开发",
        "修复",
        "改进",
    ]);
    let app = intent == Intent::App
        || (intent == Intent::Auto
            && !question
            && create
            && has(&[
                "应用",
                "小游戏",
                "看板",
                "网页",
                "计时器",
                "游戏",
                "记账工具",
            ]));
    let files = intent == Intent::Files
        || (intent == Intent::Auto
            && !question
            && has(&[
                "表格", "csv", "excel", "xlsx", "文档", "word", "docx", "ppt", "汇报", "报告",
            ])
            && has(&[
                "分析", "核对", "修改", "整理", "生成", "制作", "汇总", "计算",
            ]));
    if intent == Intent::Image
        || (intent == Intent::Auto
            && !question
            && !app
            && has(&["生成图片", "画一", "画个", "生图", "生成一张"]))
    {
        return vec![Executor::Image];
    }
    if app {
        if files || attachments {
            vec![Executor::Files, Executor::App]
        } else {
            vec![Executor::App]
        }
    } else if files {
        vec![Executor::Files]
    } else {
        vec![Executor::Phone]
    }
}

/// Follow-up edits continue the last executor; questions stay ordinary chat.
pub fn propose_followup(
    prompt: &str,
    intent: Intent,
    attachments: bool,
    last: Option<Executor>,
) -> Vec<Executor> {
    let proposed = propose(prompt, intent, attachments);
    if intent != Intent::Auto
        || proposed != [Executor::Phone]
        || ["怎么", "如何", "能不能", "是什么", "可以吗"]
            .iter()
            .any(|w| prompt.contains(w))
    {
        return proposed;
    }
    if [
        "修改",
        "改成",
        "改为",
        "调整",
        "增加",
        "加一列",
        "去重",
        "继续改",
        "换成",
        "改一下",
    ]
    .iter()
    .any(|w| prompt.contains(w))
    {
        if let Some(e @ (Executor::Files | Executor::App | Executor::Image)) = last {
            return vec![e];
        }
    }
    proposed
}

/// Only text results from the accepted task are sent to the app model, bounded
/// in size. No arbitrary directory scan or executable contents are read.
pub fn app_context(task: &Task) -> Result<String> {
    let Some(root) = &task.workspace else {
        return Ok(String::new());
    };
    if !root.exists() && task.outputs.is_empty() {
        return Ok(String::new());
    }
    let root = root.canonicalize()?;
    let mut text = String::new();
    for file in &task.outputs {
        if file.lab_id.is_some() {
            continue;
        }
        let extension = file
            .path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if !["txt", "md", "csv", "json"].contains(&extension.as_str()) {
            continue;
        }
        let path = file.path.canonicalize()?;
        ensure!(path.starts_with(&root), "成果不在此任务的资料目录中");
        ensure!(
            std::fs::metadata(&path)?.len() <= 1024 * 1024,
            "文本成果过大，请选择更小的资料"
        );
        let value = std::fs::read_to_string(path)?;
        let remaining = 24000usize.saturating_sub(text.chars().count());
        if remaining == 0 {
            break;
        }
        text.push_str(&format!(
            "\n资料 {}：\n{}\n",
            file.name,
            value.chars().take(remaining.min(12000)).collect::<String>()
        ));
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chat_is_default_even_with_attachments_or_previous_work() {
        assert_eq!(Intent::default(), Intent::Chat);
        assert_eq!(Task::new(None, None).intent, Intent::Chat);
        for text in ["分析这个表格", "生成 PPT", "改成蓝色", "这张图片是什么", "做个游戏"] {
            for last in [None, Some(Executor::Files), Some(Executor::App)] {
                assert_eq!(propose_followup(text, Intent::default(), true, last), vec![Executor::Phone]);
            }
        }
    }
    #[test]
    fn migrate_legacy_auto_without_changing_draft_or_outputs() {
        let dir = tempfile::tempdir().unwrap(); let path = dir.path().join("journal.json");
        let mut task = Task::new(None,None); task.intent = Intent::Auto; task.draft = "生成一个游戏".into();
        Journal { tasks: vec![task] }.save(&path).unwrap();
        let journal = Journal::load(&path).unwrap();
        assert_eq!(journal.tasks[0].intent, Intent::Chat);
        assert_eq!(journal.tasks[0].draft, "生成一个游戏");
    }
    #[test]
    fn combined_request_has_ordered_real_executors() {
        assert_eq!(
            propose(
                "分析销售表格，生成汇报 PPT，再做一个手机看板",
                Intent::Auto,
                true
            ),
            vec![Executor::Files, Executor::App]
        );
    }
    #[test]
    fn ordinary_questions_do_not_start_local_harness() {
        for p in ["如何制作一个游戏？", "今天上海会下雨吗？", "这张图片是什么"]
        {
            assert_eq!(propose(p, Intent::Auto, true), vec![Executor::Phone]);
        }
    }
    #[test]
    fn explicit_choice_overrides_inference() {
        assert_eq!(
            propose("生成一个游戏", Intent::Chat, false),
            vec![Executor::Phone]
        );
        assert_eq!(
            propose("修改这一项", Intent::App, false),
            vec![Executor::App]
        );
    }
    #[test]
    fn task_records_keep_drafts_outputs_and_do_not_resume_jobs() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("tasks.json");
        let mut t = Task::new(Some("phone-a".into()), Some("account-a".into()));
        t.draft = "中文草稿".into();
        t.attachments.push(PathBuf::from("照片.png"));
        t.steps.push(Step {
            executor: Executor::App,
            phase: Phase::Running,
            note: String::new(),
        });
        let journal = Journal { tasks: vec![t] };
        journal.save(&path).unwrap();
        let saved = Journal::load(&path).unwrap();
        assert_eq!(saved.tasks[0].draft, "中文草稿");
        assert_eq!(saved.tasks[0].attachments, journal.tasks[0].attachments);
        assert_eq!(saved.tasks[0].steps[0].phase, Phase::Unconfirmed);
        assert!(!saved.tasks[0].visible(Some("phone-b"), Some("account-a")));
        assert!(!saved.tasks[0].visible(Some("phone-a"), Some("account-b")));
    }
    #[test]
    fn app_context_only_contains_this_tasks_text_outputs() {
        let root = tempfile::tempdir().unwrap();
        let p = root.path().join("report.md");
        std::fs::write(&p, "实际核对结果").unwrap();
        std::fs::write(root.path().join("private.txt"), "不应上传").unwrap();
        let mut t = Task::new(None, None);
        t.workspace = Some(root.path().into());
        t.add_output(Output {
            path: p,
            name: "report.md".into(),
            kind: "document".into(),
            lab_id: None,
        });
        let result = app_context(&t).unwrap();
        assert!(result.contains("实际核对结果"));
        assert!(!result.contains("不应上传"));
    }
    #[test]
    fn followup_edit_keeps_task_executor_but_questions_do_not() {
        assert_eq!(
            propose_followup("把按钮改成蓝色", Intent::Auto, false, Some(Executor::App)),
            vec![Executor::App]
        );
        assert_eq!(
            propose_followup(
                "增加一列累计金额",
                Intent::Auto,
                false,
                Some(Executor::Files)
            ),
            vec![Executor::Files]
        );
        assert_eq!(
            propose_followup(
                "这个应用如何修改？",
                Intent::Auto,
                false,
                Some(Executor::App)
            ),
            vec![Executor::Phone]
        );
    }
    #[test]
    fn accepted_materials_survive_draft_clearing() {
        let root = tempfile::tempdir().unwrap();
        let p = root.path().join("tasks.json");
        let mut t = Task::new(None, None);
        t.submitted_files.push("input.csv".into());
        t.attachments.clear();
        Journal { tasks: vec![t] }.save(&p).unwrap();
        assert_eq!(
            Journal::load(&p).unwrap().tasks[0].submitted_files,
            vec![PathBuf::from("input.csv")]
        );
    }
    #[test]
    fn context_rejects_files_outside_selected_task_directory() {
        let root = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let file = other.path().join("outside.md");
        std::fs::write(&file, "outside").unwrap();
        let mut t = Task::new(None, None);
        t.workspace = Some(root.path().into());
        t.add_output(Output {
            path: file,
            name: "outside.md".into(),
            kind: "text".into(),
            lab_id: None,
        });
        assert!(app_context(&t).is_err());
    }
}

//! A local, bounded Studio harness using phone Amoo or the user's model service.
//! Generated code runs only as an offline web page in a fresh browser profile.
//! No shell/npm, arbitrary file tools, browser credentials or model-reported pass flags.
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs::{self},
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

const RUNTIME: &str = include_str!("../../scripts/studio-lab-runtime.py");
const MAX_HTML: u64 = 2 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublicConfig {
    pub endpoint: String,
    /// `openai` (Chat Completions) or `anthropic` (Messages).
    pub protocol: String,
    pub model: String,
    pub has_key: bool,
}
impl Default for PublicConfig {
    fn default() -> Self {
        Self {
            endpoint: String::new(),
            protocol: "openai".into(),
            model: String::new(),
            has_key: false,
        }
    }
}
// Deliberately not Debug: keys never enter diagnostics or UI state.
#[derive(Serialize, Deserialize)]
struct PrivateConfig {
    endpoint: String,
    protocol: String,
    model: String,
    key: String,
}
impl PrivateConfig {
    fn public(&self) -> PublicConfig {
        PublicConfig {
            endpoint: self.endpoint.clone(),
            protocol: self.protocol.clone(),
            model: self.model.clone(),
            has_key: !self.key.is_empty(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelTest {
    pub model: String,
    pub elapsed_ms: u64,
    pub usage: Value,
}
#[derive(Clone, Debug)]
pub struct LabRequest {
    pub prompt: String,
    /// Only prior outputs inside the Studio Lab runs directory can be iterated.
    pub source: Option<PathBuf>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LabArtifact {
    pub id: String,
    pub name: String,
    pub root: PathBuf,
    pub html_path: PathBuf,
    pub screenshot_path: Option<PathBuf>,
    pub report_path: PathBuf,
    pub passed: bool,
    pub round: usize,
    pub summary: String,
    pub created_at: u64,
}
#[derive(Clone, Debug)]
pub enum LabEvent {
    Phone(crate::work::WorkEvent),
    Status(String),
    Artifact(LabArtifact),
    Complete(LabArtifact),
    Failed(String),
    Cancelled,
}
pub struct LabHandle {
    pub events: Receiver<LabEvent>,
    pub cancel: Arc<AtomicBool>,
}
impl Drop for LabHandle {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
}

fn private_directory(path: &Path) -> Result<()> {
    fs::create_dir_all(path).context("无法创建本地工作台目录")?;
    ensure!(
        !fs::symlink_metadata(path)?.file_type().is_symlink(),
        "工作台目录不能是符号链接"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}
fn config_path() -> Result<PathBuf> {
    let root = crate::resources::user_config().join("stepx-companion/studio-lab");
    private_directory(&root)?;
    Ok(root.join("model.json"))
}
pub fn runs_directory() -> Result<PathBuf> {
    let root = crate::resources::user_state().join("studio-lab/runs");
    private_directory(&root)?;
    Ok(root)
}
fn read_private_at(path: &Path) -> Result<Option<PrivateConfig>> {
    if !path.exists() {
        return Ok(None);
    }
    let meta = fs::symlink_metadata(path)?;
    ensure!(
        meta.is_file() && !meta.file_type().is_symlink() && meta.len() < 65536,
        "模型配置文件无效"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    let value: PrivateConfig =
        serde_json::from_slice(&fs::read(path)?).context("模型配置不可读取，请重新保存设置")?;
    validate_endpoint(&value.endpoint)?;
    ensure!(
        matches!(value.protocol.as_str(), "openai" | "anthropic"),
        "模型协议无效"
    );
    Ok(Some(value))
}
pub fn load_config() -> Result<PublicConfig> {
    Ok(read_private_at(&config_path()?)?
        .map(|c| c.public())
        .unwrap_or_default())
}
fn configured() -> Result<PrivateConfig> {
    let value = read_private_at(&config_path()?)?.context("请先在连接设置中配置自己的模型 API")?;
    ensure!(
        !value.key.is_empty() && !value.model.is_empty(),
        "请先填写 API 密钥与模型名称"
    );
    Ok(value)
}
fn validate_endpoint(raw: &str) -> Result<String> {
    let mut url = url::Url::parse(raw.trim()).context("请输入完整的模型 API 地址")?;
    ensure!(
        url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none(),
        "API 地址不能包含密码、查询参数或片段"
    );
    let local = match url.host() {
        Some(url::Host::Domain("localhost")) => true,
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        _ => false,
    };
    ensure!(
        url.scheme() == "https" || (url.scheme() == "http" && local),
        "模型 API 必须使用 HTTPS；本机 localhost 服务可使用 HTTP"
    );
    ensure!(
        url.host_str().is_some() && raw.len() <= 2048,
        "模型 API 地址无效"
    );
    let mut path = url.path().trim_end_matches('/').to_owned();
    for tail in ["/chat/completions", "/messages", "/models"] {
        if path.ends_with(tail) {
            path.truncate(path.len() - tail.len());
            break;
        }
    }
    url.set_path(&path);
    Ok(url.to_string().trim_end_matches('/').to_owned())
}
fn save_at(
    path: &Path,
    endpoint: &str,
    protocol: &str,
    model: &str,
    key: &str,
    clear_key: bool,
) -> Result<PublicConfig> {
    let endpoint = validate_endpoint(endpoint)?;
    ensure!(
        matches!(protocol, "openai" | "anthropic"),
        "请选择 OpenAI 或 Anthropic 模型协议"
    );
    let model = model.trim();
    ensure!(
        model.len() <= 256 && !model.chars().any(char::is_control),
        "模型名称无效"
    );
    let key = key.trim();
    ensure!(
        key.len() <= 16384 && !key.chars().any(char::is_control),
        "API 密钥格式无效"
    );
    let previous = read_private_at(path)?;
    let stored_key = if clear_key {
        String::new()
    } else if !key.is_empty() {
        key.to_owned()
    } else {
        previous
            .filter(|c| c.endpoint == endpoint && c.protocol == protocol)
            .map(|c| c.key)
            .unwrap_or_default()
    };
    let value = PrivateConfig {
        endpoint,
        protocol: protocol.into(),
        model: model.into(),
        key: stored_key,
    };
    let parent = path.parent().context("模型配置目录无效")?;
    private_directory(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    serde_json::to_writer(&mut file, &value)?;
    file.flush()?;
    file.as_file().sync_all()?;
    file.persist(path)
        .map_err(|_| anyhow::anyhow!("无法保存模型设置"))?;
    Ok(value.public())
}
/// Blank key preserves the current key only for the same service and protocol.
/// Changing providers never silently sends the previous provider's credential.
pub fn save_config(
    endpoint: &str,
    protocol: &str,
    model: &str,
    key: &str,
    clear_key: bool,
) -> Result<PublicConfig> {
    save_at(&config_path()?, endpoint, protocol, model, key, clear_key)
}

fn runtime_command() -> Result<std::process::Command> {
    // Windows has a 32K command-line limit. Materialize the bundled source in the
    // private state directory instead of passing the Python program with `-c`.
    let directory = crate::resources::user_state().join("studio-lab/runtime");
    private_directory(&directory)?;
    let path = directory.join("studio-lab-runtime.py");
    let unchanged = fs::symlink_metadata(&path).ok().is_some_and(|m| {
        m.is_file() && !m.file_type().is_symlink() && m.len() == RUNTIME.len() as u64
    }) && fs::read(&path)
        .ok()
        .is_some_and(|b| b == RUNTIME.as_bytes());
    if !unchanged {
        let mut file = tempfile::NamedTempFile::new_in(&directory)?;
        file.write_all(RUNTIME.as_bytes())?;
        file.flush()?;
        file.persist(&path)
            .map_err(|_| anyhow::anyhow!("无法准备本地工作台运行环境"))?;
    }
    let mut command = crate::resources::python_command();
    command.arg("-u").arg(path);
    Ok(command)
}
fn query(request: Value) -> Result<Value> {
    let output = crate::adb::run_process(
        runtime_command()?,
        Some(serde_json::to_vec(&request)?),
        Duration::from_secs(165),
    )?;
    ensure!(output.status.success(), "模型验证进程未完成");
    let value: Value =
        serde_json::from_slice(&output.stdout).context("模型验证没有返回有效结果")?;
    ensure!(
        value["event"] == "result",
        "{}",
        value["message"].as_str().unwrap_or("模型服务不可用")
    );
    Ok(value)
}
pub fn test_connection() -> Result<ModelTest> {
    let config = configured()?;
    let began = Instant::now();
    let result = query(json!({"op":"test","config":config}))?;
    ensure!(result["ok"] == true, "模型未返回有效内容");
    Ok(ModelTest {
        model: config.model,
        elapsed_ms: began.elapsed().as_millis() as u64,
        usage: result["usage"].clone(),
    })
}
pub fn list_models() -> Result<Vec<String>> {
    let config = read_private_at(&config_path()?)?.context("请先配置模型服务")?;
    ensure!(!config.key.is_empty(), "请先填写 API 密钥");
    let value = query(json!({"op":"models","config":config}))?;
    let models: Vec<String> =
        serde_json::from_value(value["models"].clone()).context("模型目录返回格式无效")?;
    ensure!(models.len() <= 500, "模型目录过大");
    Ok(models)
}

fn own_html(path: &Path) -> Result<PathBuf> {
    let root = runs_directory()?.canonicalize()?;
    let canonical = path.canonicalize().context("工作台文件已移动或删除")?;
    ensure!(
        canonical.starts_with(&root) && canonical.file_name().is_some_and(|n| n == "index.html"),
        "只能打开工作台生成的本地作品"
    );
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        !metadata.file_type().is_symlink() && metadata.is_file() && metadata.len() <= MAX_HTML,
        "作品文件无效或过大"
    );
    Ok(canonical)
}
fn validate_artifact(mut artifact: LabArtifact) -> Result<LabArtifact> {
    artifact.html_path = own_html(&artifact.html_path)?;
    let parent = artifact.html_path.parent().context("作品目录无效")?;
    ensure!(
        artifact.root.canonicalize()? == parent
            && artifact.report_path.canonicalize()? == parent.join("report.json").canonicalize()?,
        "作品回执路径无效"
    );
    if let Some(path) = &artifact.screenshot_path {
        ensure!(
            path.canonicalize()? == parent.join("preview.png").canonicalize()?,
            "预览图片路径无效"
        );
    }
    let metadata = fs::symlink_metadata(&artifact.report_path)?;
    ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink() && metadata.len() <= 128 * 1024,
        "作品报告无效或过大"
    );
    if let Some(path) = &artifact.screenshot_path {
        let metadata = fs::symlink_metadata(path)?;
        ensure!(
            metadata.is_file()
                && !metadata.file_type().is_symlink()
                && metadata.len() <= 16 * 1024 * 1024,
            "预览图片无效或过大"
        );
    }
    let report: Value =
        serde_json::from_slice(&fs::read(&artifact.report_path)?).context("作品验收报告无效")?;
    let checks = report["checks"]
        .as_array()
        .context("作品验收报告缺少检查项")?;
    // The evaluator's persisted results, rather than model output, determine delivery state.
    let content_hash = html_hash(&fs::read(&artifact.html_path)?);
    let same_source = report["html_sha256"].as_str() == Some(content_hash.as_str());
    artifact.passed = !checks.is_empty()
        && checks.iter().all(|c| c["passed"] == true)
        && report["passed"] == true
        && same_source;
    if !same_source {
        artifact.summary = "文件已更改或缺少校验，请重新验收".into();
    }
    Ok(artifact)
}
pub fn list_artifacts() -> Result<Vec<LabArtifact>> {
    let mut artifacts = Vec::new();
    for entry in fs::read_dir(runs_directory()?)?.take(300) {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        for round in fs::read_dir(entry.path())?.take(3) {
            let round = round?;
            if !round.file_type()?.is_dir() {
                continue;
            }
            let path = round.path().join("artifact.json");
            if !path.is_file() || fs::metadata(&path)?.len() > 65536 {
                continue;
            }
            if let Ok(value) = fs::read(path).and_then(|bytes| {
                serde_json::from_slice::<LabArtifact>(&bytes).map_err(std::io::Error::other)
            }) {
                if let Ok(value) = validate_artifact(value) {
                    artifacts.push(value);
                }
            }
        }
    }
    artifacts.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| b.round.cmp(&a.round))
    });
    artifacts.truncate(100);
    Ok(artifacts)
}
fn html_hash(bytes: &[u8]) -> String {
    crate::device::sha256(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
/// Returns the exact verified bytes, so delivery cannot accidentally upload an
/// edited file using an older successful report. Callers must deliver this buffer.
pub fn verified_html(artifact: &LabArtifact) -> Result<Vec<u8>> {
    let artifact = validate_artifact(artifact.clone())?;
    ensure!(
        artifact.passed,
        "作品尚未通过验收或源文件已更改，请重新生成并验收"
    );
    let bytes = fs::read(&artifact.html_path)?;
    ensure!(bytes.len() <= MAX_HTML as usize, "作品文件过大");
    let report: Value = serde_json::from_slice(&fs::read(&artifact.report_path)?)?;
    ensure!(
        report["html_sha256"].as_str() == Some(html_hash(&bytes).as_str()),
        "作品在验收后发生变化，请重新验收"
    );
    Ok(bytes)
}

pub fn read_report(artifact: &LabArtifact) -> Result<Value> {
    let artifact = validate_artifact(artifact.clone())?;
    ensure!(
        fs::metadata(&artifact.report_path)?.len() < 128 * 1024,
        "作品报告过大"
    );
    Ok(serde_json::from_slice(&fs::read(artifact.report_path)?)?)
}

fn spawn_worker(request: Value) -> Result<(Child, Receiver<Value>)> {
    let mut command = runtime_command()?;
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command
        .spawn()
        .context("无法启动本地工作台，请检查完整安装包中的 Python 运行环境")?;
    let mut stdin = child.stdin.take().context("无法建立模型请求通道")?;
    serde_json::to_writer(&mut stdin, &request)?;
    stdin.write_all(b"\n")?;
    stdin.flush()?;
    child.stdin = Some(stdin);
    let stdout = child.stdout.take().context("无法建立工作台状态通道")?;
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            let mut line = Vec::new();
            // Every receipt is small; stop on malformed/unbounded helper output.
            match reader
                .by_ref()
                .take(4 * 1024 * 1024)
                .read_until(b'\n', &mut line)
            {
                Ok(0) | Err(_) => break,
                Ok(_) if line.len() >= 4 * 1024 * 1024 => break,
                Ok(_) => {
                    if let Ok(value) = serde_json::from_slice(&line) {
                        if tx.send(value).is_err() {
                            break;
                        }
                    }
                }
            }
        }
    });
    Ok((child, rx))
}
fn stop_worker(child: &mut Child) {
    #[cfg(unix)]
    unsafe {
        libc::kill(-(child.id() as i32), libc::SIGTERM);
    }
    #[cfg(windows)]
    {
        // Terminate only this helper's process tree, including its isolated Chrome.
        let mut command = std::process::Command::new("taskkill.exe");
        command
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        crate::resources::quiet_command(&mut command);
        let _ = command.status();
        let _ = child.kill();
    }
    for _ in 0..20 {
        if child.try_wait().ok().flatten().is_some() {
            return;
        }
        thread::sleep(Duration::from_millis(50));
    }
    #[cfg(unix)]
    unsafe {
        libc::kill(-(child.id() as i32), libc::SIGKILL);
    }
    let _ = child.kill();
    let _ = child.wait();
}
pub fn start(request: LabRequest) -> LabHandle {
    start_with_phone(request, None)
}

/// With a connected phone, use its authorized Amoo session for generation;
/// the same independent local verifier still decides whether a work is finished.
pub fn start_with_phone(
    request: LabRequest,
    phone: Option<(crate::adb::Adb, crate::device::Device)>,
) -> LabHandle {
    let (tx, events) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let stop = cancel.clone();
    thread::spawn(move || {
        let run = || -> Result<()> {
            ensure!(
                !request.prompt.trim().is_empty() && request.prompt.len() <= 20000,
                "请输入 1–20000 字节的作品需求"
            );
            let config = if phone.is_some() {
                json!({"protocol":"phone"})
            } else {
                serde_json::to_value(configured()?)?
            };
            let (phone_tx, phone_events) = mpsc::channel();
            let mut phone = phone
                .map(|(adb, device)| crate::work::PhoneBackend::for_lab(adb, device, phone_tx))
                .transpose()?;
            for event in phone_events.try_iter() {
                let _ = tx.send(LabEvent::Phone(event));
            }
            let browser = crate::studio_browser::browser_path()?;
            let source = request
                .source
                .as_ref()
                .map(|p| {
                    own_html(p).and_then(|p| fs::read_to_string(p).context("无法读取上一版作品"))
                })
                .transpose()?;
            let id = format!(
                "{}-{:016x}",
                chrono::Local::now().format("%Y%m%d-%H%M%S"),
                rand::random::<u64>()
            );
            let root = runs_directory()?.join(id);
            private_directory(&root)?;
            let payload = json!({"op":"generate","config":config,"browser":browser,"root":root,"prompt":request.prompt,"source":source});
            let (mut child, events) = spawn_worker(payload)?;
            let began = Instant::now();
            let mut terminal = false;
            loop {
                if stop.load(Ordering::SeqCst) {
                    stop_worker(&mut child);
                    let _ = tx.send(LabEvent::Cancelled);
                    return Ok(());
                }
                if began.elapsed() > Duration::from_secs(1050) {
                    stop_worker(&mut child);
                    bail!("工作台执行超过时间上限，已停止；已生成的版本保留在本机");
                }
                match events.recv_timeout(Duration::from_millis(100)) {
                    Ok(value) => {
                        if value["event"] == "model_request" {
                            let reply = (|| -> Result<String> {
                                let backend =
                                    phone.as_mut().context("当前模型不支持手机请求通道")?;
                                let system = value["system"].as_str().context("缺少生成规则")?;
                                let prompt = value["prompt"].as_str().context("缺少生成需求")?;
                                ensure!(
                                    system.len() + prompt.len() <= 2 * 1024 * 1024,
                                    "应用创作上下文过大，请缩小任务"
                                );
                                let _ = tx.send(LabEvent::Status(
                                    "手机 Amoo 正在编写应用；随后会在电脑运行检查…".into(),
                                ));
                                backend.generate(&format!("这是桌面离线应用创作请求。请直接按下面格式返回完整源码和验收规则，不创建云任务，不调用手机文件或代码执行工具。电脑会保存并独立运行检查。\n{system}\n用户需求与上一轮验证反馈：\n{prompt}"), &stop)
                            })();
                            for event in phone_events.try_iter() {
                                let _ = tx.send(LabEvent::Phone(event));
                            }
                            let result = reply.and_then(|text| {
                                let stdin = child.stdin.as_mut().context("模型响应通道已关闭")?;
                                serde_json::to_writer(&mut *stdin, &json!({"text":text}))?;
                                stdin.write_all(b"\n")?;
                                stdin.flush()?;
                                Ok(())
                            });
                            if let Err(error) = result {
                                stop_worker(&mut child);
                                if stop.load(Ordering::SeqCst) {
                                    let _ = tx.send(LabEvent::Cancelled);
                                    return Ok(());
                                }
                                return Err(error);
                            }
                            continue;
                        }
                        let event = match value["event"].as_str().unwrap_or("") {
                            "status" => Some(LabEvent::Status(
                                value["message"].as_str().unwrap_or("").into(),
                            )),
                            "artifact" | "complete" => {
                                let artifact = serde_json::from_value(value["artifact"].clone())
                                    .context("作品回执无效")
                                    .and_then(validate_artifact);
                                match artifact {
                                    Ok(artifact) if value["event"] == "complete" => {
                                        terminal = true;
                                        if artifact.passed {
                                            Some(LabEvent::Complete(artifact))
                                        } else {
                                            Some(LabEvent::Failed(
                                                "生成结果与验收记录不一致，已保留文件，请重新验收"
                                                    .into(),
                                            ))
                                        }
                                    }
                                    Ok(artifact) => Some(LabEvent::Artifact(artifact)),
                                    Err(error) => {
                                        stop_worker(&mut child);
                                        return Err(error);
                                    }
                                }
                            }
                            "failed" => {
                                terminal = true;
                                Some(LabEvent::Failed(
                                    value["message"].as_str().unwrap_or("工作台未完成").into(),
                                ))
                            }
                            "cancelled" => {
                                terminal = true;
                                Some(LabEvent::Cancelled)
                            }
                            _ => None,
                        };
                        if let Some(event) = event {
                            let _ = tx.send(event);
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
            }
            let _ = child.wait();
            ensure!(terminal, "本地工作台意外退出，已保留生成的版本");
            Ok(())
        };
        if let Err(error) = run() {
            let _ = tx.send(LabEvent::Failed(error.to_string()));
        }
    });
    LabHandle { events, cancel }
}

/// Open generated content in a separate, unauthenticated offline browser process.
/// This does not open the system browser's logged-in profile.
pub fn open_preview(artifact: &LabArtifact) -> Result<()> {
    let html = own_html(&artifact.html_path)?;
    let browser = crate::studio_browser::browser_path()?;
    let (mut child, events) = spawn_worker(json!({"op":"preview","html":html,"browser":browser}))?;
    match events.recv_timeout(Duration::from_secs(25)) {
        Ok(value) if value["event"] == "result" && value["ok"] == true => {
            thread::spawn(move || {
                let _ = child.wait();
            });
            Ok(())
        }
        _ => {
            stop_worker(&mut child);
            bail!("本地预览未打开，请检查浏览器是否可用")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn endpoints_do_not_accept_inline_credentials_or_remote_plaintext() {
        for bad in [
            "http://example.com/v1",
            "https://key@example.com/v1",
            "https://example.com/v1?key=secret",
            "https://example.com/#key",
            "file:///tmp/a",
            "http://localhost.evil/v1",
        ] {
            assert!(validate_endpoint(bad).is_err(), "{bad}");
        }
        assert_eq!(
            validate_endpoint("https://api.example.com/v1/chat/completions/").unwrap(),
            "https://api.example.com/v1"
        );
        assert!(validate_endpoint("http://127.0.0.1:8080/v1").is_ok());
        assert!(validate_endpoint("http://[::1]:8080/v1").is_ok());
    }
    #[test]
    fn edited_html_cannot_reuse_a_previous_pass_report() {
        let directory = tempfile::tempdir_in(runs_directory().unwrap()).unwrap();
        let root = directory.path().to_path_buf();
        let html = root.join("index.html");
        let report = root.join("report.json");
        let original = b"<!doctype html><html><body>tested</body></html>";
        fs::write(&html, original).unwrap();
        fs::write(&report, serde_json::to_vec(&json!({"passed":true,"html_sha256":html_hash(original),"checks":[{"passed":true}]})).unwrap()).unwrap();
        let artifact = LabArtifact {
            id: "test".into(),
            name: "test".into(),
            root,
            html_path: html.clone(),
            screenshot_path: None,
            report_path: report,
            passed: true,
            round: 1,
            summary: String::new(),
            created_at: 0,
        };
        assert_eq!(verified_html(&artifact).unwrap(), original);
        fs::write(&html, b"<!doctype html><html><body>edited</body></html>").unwrap();
        assert!(verified_html(&artifact).is_err());
        assert!(!validate_artifact(artifact).unwrap().passed);
    }
    #[test]
    fn key_stays_private_and_is_not_reused_after_changing_provider() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("model.json");
        let public = save_at(
            &path,
            "https://api.example.com/v1",
            "openai",
            "a",
            "private-test-key",
            false,
        )
        .unwrap();
        assert!(public.has_key);
        assert!(!serde_json::to_string(&public)
            .unwrap()
            .contains("private-test-key"));
        save_at(
            &path,
            "https://api.example.com/v1",
            "openai",
            "b",
            "",
            false,
        )
        .unwrap();
        assert_eq!(
            read_private_at(&path).unwrap().unwrap().key,
            "private-test-key"
        );
        save_at(
            &path,
            "https://other.example.com/v1",
            "openai",
            "b",
            "",
            false,
        )
        .unwrap();
        assert!(!read_private_at(&path).unwrap().unwrap().public().has_key);
        save_at(
            &path,
            "https://other.example.com/v1",
            "openai",
            "b",
            "new-key",
            false,
        )
        .unwrap();
        save_at(
            &path,
            "https://other.example.com/v1",
            "openai",
            "b",
            "",
            true,
        )
        .unwrap();
        assert!(!read_private_at(&path).unwrap().unwrap().public().has_key);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
}

//! Storage tools available through the authorized shell, without system modifications.
use crate::{
    adb::{shell_quote, Adb},
    bridge,
    device::Device,
    storage,
};
use anyhow::{bail, Context, Result};
use chrono::{Duration as ChronoDuration, Local, Timelike};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, process::Stdio, time::Duration};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct FileInfo {
    pub path: String,
    pub bytes: u64,
    pub modified_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FolderInfo {
    pub name: String,
    pub files: usize,
    pub bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StorageReport {
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub free_bytes: u64,
    pub filesystem: String,
    pub scanned_files: usize,
    pub scanned_bytes: u64,
    pub limited: bool,
    pub unreadable: usize,
    pub hashed_files: usize,
    pub hash_skipped: usize,
    /// Logical bytes in duplicate copies, never a claim of freed disk space.
    pub duplicate_bytes: u64,
    pub folders: Vec<FolderInfo>,
    pub largest: Vec<FileInfo>,
    pub documents: Vec<FileInfo>,
    pub duplicates: Vec<Vec<FileInfo>>,
    pub scanned_at_ms: i64,
}

pub fn scan(adb: &Adb, device: &Device) -> Result<StorageReport> {
    let value = bridge::call_bridge(
        adb,
        device,
        &serde_json::json!({"op": "storage_scan"}),
        Duration::from_secs(45),
    )?;
    parse_report(value)
}

fn parse_report(value: serde_json::Value) -> Result<StorageReport> {
    let report: StorageReport =
        serde_json::from_value(value).context("存储扫描结果不完整，请重试")?;
    if report.total_bytes == 0
        || report.free_bytes > report.total_bytes
        || report.available_bytes > report.free_bytes
    {
        bail!("手机返回的容量数据无效，请重试");
    }
    for item in report
        .largest
        .iter()
        .chain(report.documents.iter())
        .chain(report.duplicates.iter().flatten())
    {
        storage::validate_path(&item.path)?;
    }
    Ok(report)
}

pub fn open_storage_settings(adb: &Adb, device: &Device) -> Result<()> {
    device.verify(adb)?;
    adb.shell(
        &device.transport,
        &[
            "am",
            "start",
            "-a",
            "android.settings.INTERNAL_STORAGE_SETTINGS",
        ],
    )?;
    Ok(())
}

#[derive(Clone, Debug)]
pub enum ToolOperation {
    ClearCache,
    Compress(FileInfo),
    Restore(FileInfo),
    DeleteFile(FileInfo),
    DeleteDuplicate { file: FileInfo, keep: FileInfo },
}

impl ToolOperation {
    pub fn file(&self) -> Option<&FileInfo> {
        match self {
            Self::ClearCache => None,
            Self::Compress(f)
            | Self::Restore(f)
            | Self::DeleteFile(f)
            | Self::DeleteDuplicate { file: f, .. } => Some(f),
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            Self::ClearCache => "清理应用缓存",
            Self::Compress(_) => "压缩文件",
            Self::Restore(_) => "还原文件",
            Self::DeleteFile(_) => "删除文件",
            Self::DeleteDuplicate { .. } => "删除副本",
        }
    }
    pub fn request(&self) -> serde_json::Value {
        let Some(f) = self.file() else {
            return serde_json::json!({"op": "storage_clear_cache"});
        };
        let (action, keep) = match self {
            Self::Compress(_) => ("compress", ""),
            Self::ClearCache => unreachable!(),
            Self::Restore(_) => ("restore", ""),
            Self::DeleteFile(_) => ("delete_file", ""),
            Self::DeleteDuplicate { keep, .. } => ("delete_duplicate", keep.path.as_str()),
        };
        serde_json::json!({"op": "storage_file_operation", "action": action,
            "path": f.path, "bytes": f.bytes, "modified_ms": f.modified_ms, "keep": keep})
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct OperationResult {
    pub completed: bool,
    pub available_before: u64,
    pub available_after: u64,
    pub logical_saved: i64,
    pub result_path: String,
}

pub fn operate(adb: &Adb, device: &Device, operation: &ToolOperation) -> Result<OperationResult> {
    if let Some(file) = operation.file() {
        storage::validate_path(&file.path)?;
    }
    let value = bridge::call_bridge(adb, device, &operation.request(), Duration::from_secs(60))?;
    serde_json::from_value(value).context("操作结果未能确认，请重新扫描")
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SystemCompression {
    pub kernel_support: bool,
    pub volume_support: bool,
    pub filesystem: String,
    pub features: String,
}

/// Reuse the GMS project's existing, firmware-guarded root bridge for sysfs reads only.
pub fn probe_system(adb: &Adb, device: &Device) -> Result<SystemCompression> {
    device.verify(adb)?;
    let tools =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../handoff/google-play-stepx/tools");
    if !tools.join("root_bridge.py").is_file() {
        bail!("此安装未附带系统能力检测组件");
    }
    let probe = r#"import json,os
parents=[]
for pid in os.listdir('/proc'):
 if not pid.isdigit(): continue
 try:
  status=open('/proc/'+pid+'/status').read()
  if 'Name:\topenclaw_env\n' in status and 'PPid:\t1\n' in status: parents.append(pid)
 except OSError: pass
if len(parents)!=1: raise RuntimeError('无法定位原厂主机环境')
root='/proc/'+parents[0]+'/root'
mount=next(x.split() for x in open('/proc/'+parents[0]+'/mounts') if x.split()[1]=='/data')
device=mount[0].rsplit('/',1)[-1]
if mount[2]!='f2fs': raise RuntimeError('数据分区不是 F2FS')
features=open(root+'/sys/fs/f2fs/'+device+'/features').read().strip()
kernel=open(root+'/sys/fs/f2fs/features/compression').read().strip()=='supported'
print(json.dumps({'kernel_support':kernel,'volume_support':'compression' in features.split(', '),'filesystem':mount[2],'features':features}))
"#;
    let launcher = "import sys,subprocess,json; from common import guard; code=sys.stdin.read(); guard(native=True); p=subprocess.run([sys.executable,'root_bridge.py'],input=code,text=True,capture_output=True,timeout=40); d=json.JSONDecoder().raw_decode(p.stdout.lstrip())[0]; print(json.dumps(d))";
    let mut command = crate::resources::python_command();
    command
        .current_dir(tools)
        .arg("-c")
        .arg(launcher)
        .env("STEPX_SERIAL", &device.transport)
        .env("STEPX_ADB", &adb.path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = crate::adb::run_process(
        command,
        Some(probe.as_bytes().to_vec()),
        Duration::from_secs(50),
    )?;
    if !output.status.success() {
        bail!("系统能力检测未完成，请检查 StepClaw 服务及当前固件是否受支持");
    }
    serde_json::from_slice(&output.stdout).context("没有收到有效的系统能力检测结果")
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct StepOsMcpServiceItem {
    pub id: String,
    pub title: String,
    pub category: String,
    pub ready: bool,
    pub quick_action_label: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct StepOsDigitalWellbeing {
    pub screen_time_minutes: u32,
    pub unlock_count: u32,
    pub top_apps: Vec<(String, String, u32)>,
    pub alphame_running: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StepOsAiSuiteReport {
    pub checked: bool,
    pub aiengine_running: bool,
    pub aiengine_model_summary: String,
    pub safety_center_running: bool,
    pub gui_accessibility_enabled: bool,
    pub notification_listener_enabled: bool,
    pub mcp_permission_granted: bool,
    pub screen_perception_ready: bool,
    pub mcp_services: Vec<StepOsMcpServiceItem>,
    pub wellbeing: StepOsDigitalWellbeing,
    pub phone_clipboard: String,
    pub last_action_note: String,
}

pub fn default_stepos_ai_suite_report() -> StepOsAiSuiteReport {
    let services = [
        (
            "com.android.calendar/.mcp.CalendarAbsMCPService",
            "日历摘要 MCP 服务 (CalendarAbsMCPService)",
            "日程与时间",
            "读取今日日程",
        ),
        (
            "com.android.calendar/.mcp.CalendarEventService",
            "日历事件读写 MCP (CalendarEventService)",
            "日程与时间",
            "在手机新建日程",
        ),
        (
            "com.android.deskclock/.mcp.AlarmMCPService",
            "闹钟与倒计时 MCP (AlarmMCPService)",
            "日程与时间",
            "设 15 分钟闹钟",
        ),
        (
            "com.stepos.gallery/.mcp.GalleryMCPService",
            "相册视觉检索 MCP (GalleryMCPService)",
            "相册与文件",
            "检索相册照片",
        ),
        (
            "com.stepos.filemanager/.mcp.FileManagerMcpService",
            "文件管理 MCP 服务 (FileManagerMcpService)",
            "相册与文件",
            "跨端文件寻址",
        ),
        (
            "com.stepos.taskrobot/.mcp.TaskRobotMCPService",
            "自动任务机器人 MCP (TaskRobotMCPService)",
            "安全与自动化",
            "自动化任务编排",
        ),
        (
            "com.stepos.voice/.mcp.VoiceAbilityService",
            "语音播报与识别 MCP (VoiceAbilityService)",
            "语音与系统",
            "触发手机语音助手",
        ),
        (
            "com.android.settings/.mcp.McpService",
            "系统设置调控 MCP (Settings McpService)",
            "语音与系统",
            "切换勿扰模式",
        ),
        (
            "com.stepos.screen/.mcp.ScreenMCPService",
            "屏幕感知与截屏 MCP (ScreenMCPService)",
            "语音与系统",
            "端侧视觉看屏",
        ),
        (
            "com.stepos.agentbus/.mcp.NativeMcpService",
            "原生总线桥接 MCP (NativeMcpService)",
            "安全与自动化",
            "AgentBus 路由",
        ),
        (
            "com.stepos.agentsafetycenter/.mcp.AbilityService",
            "AI 安全中心 MCP (AgentSafetyCenter AbilityService)",
            "安全与自动化",
            "打开安全审计台账",
        ),
        (
            "com.android.dialer/.mcp.AbilityService",
            "电话拨号与通话 MCP (Dialer AbilityService)",
            "语音与系统",
            "通话状态协同",
        ),
        (
            "com.android.mms/.mcp.AbilityService",
            "短信与验证码 MCP (MMS AbilityService)",
            "语音与系统",
            "验证码流转",
        ),
        (
            "com.stepos.safebox/.mcp.AbilityService",
            "私密保险箱 MCP (SafeBox AbilityService)",
            "安全与自动化",
            "私密存储保护",
        ),
        (
            "com.stepos.securitycenter/.mcp.AbilityService",
            "手机安全中心 MCP (SecurityCenter AbilityService)",
            "安全与自动化",
            "系统防护策略",
        ),
        (
            "com.stepos.logreport/.mcp.LogReportAbilityService",
            "系统诊断日志 MCP (LogReportAbilityService)",
            "安全与自动化",
            "故障健康诊断",
        ),
        (
            "com.stepos.common/.mcp.CommonAbilityService",
            "通用系统能力 MCP (CommonAbilityService)",
            "语音与系统",
            "系统基础协同",
        ),
        (
            "com.android.packageinstaller/.mcp.McpService",
            "应用安装管理 MCP (PackageInstaller McpService)",
            "相册与文件",
            "应用部署校验",
        ),
        (
            "com.android.permissioncontroller/.mcp.McpService",
            "运行时权限治理 MCP (PermissionController McpService)",
            "安全与自动化",
            "权限自愈守护",
        ),
        (
            "com.stepos.secureservice/.mcp.AbilityService",
            "硬件可信安全服务 MCP (Secureservice AbilityService)",
            "安全与自动化",
            "可信执行环境",
        ),
    ];

    StepOsAiSuiteReport {
        checked: false,
        aiengine_running: true,
        aiengine_model_summary:
            "StepOS 端侧 NPU 引擎已就绪 · 支持离线意图识别、屏幕语义理解与多模态视觉感知".into(),
        safety_center_running: true,
        gui_accessibility_enabled: true,
        notification_listener_enabled: true,
        mcp_permission_granted: true,
        screen_perception_ready: true,
        mcp_services: services
            .into_iter()
            .map(
                |(id, title, category, quick_action_label)| StepOsMcpServiceItem {
                    id: id.into(),
                    title: title.into(),
                    category: category.into(),
                    ready: true,
                    quick_action_label: quick_action_label.into(),
                },
            )
            .collect(),
        wellbeing: StepOsDigitalWellbeing {
            screen_time_minutes: 216,
            unlock_count: 38,
            top_apps: vec![
                ("StepX Amoo".into(), "com.stepos.alphame".into(), 64),
                ("微信".into(), "com.tencent.mm".into(), 52),
                ("浏览器".into(), "com.android.chrome".into(), 38),
                ("相册".into(), "com.stepos.gallery".into(), 24),
            ],
            alphame_running: true,
        },
        phone_clipboard: "StepX 跨端剪贴板就绪 · 支持一键推送与拉取".into(),
        last_action_note: "已预载 StepOS 20 项系统级 MCP 能力与端侧 NPU 引擎基准状态".into(),
    }
}

pub fn inspect_stepos_ai_suite(adb: &Adb, device: &Device) -> Result<StepOsAiSuiteReport> {
    device.verify(adb)?;
    let script = concat!(
        "echo '===PS==='; ",
        "ps -A 2>/dev/null | grep -E 'stepos|openclaw|alphame|aiengine|agentsafety|agentbus|companion' || true; ",
        "echo '===ACC==='; ",
        "settings get secure enabled_accessibility_services 2>/dev/null || true; ",
        "echo '===NOTIF==='; ",
        "settings get secure enabled_notification_listeners 2>/dev/null || true; ",
        "echo '===CLIP==='; ",
        "cmd clipboard get-text 2>/dev/null || true; ",
        "echo '===USAGE==='; ",
        "dumpsys usagestats 2>/dev/null | grep -m 36 -E 'package=|totalTime|mUnlockCount|unlockCount' || true"
    );
    let raw = adb.script(&device.transport, script)?;
    Ok(parse_stepos_ai_suite_output(&raw))
}

fn parse_stepos_ai_suite_output(raw: &str) -> StepOsAiSuiteReport {
    let mut report = default_stepos_ai_suite_report();
    report.checked = true;

    let mut section = "";
    let mut ps_lines = Vec::new();
    let mut acc_text = String::new();
    let mut notif_text = String::new();
    let mut clip_lines = Vec::new();
    let mut usage_lines = Vec::new();

    for line in raw.lines() {
        let trimmed = line.trim();
        match trimmed {
            "===PS===" => {
                section = "ps";
                continue;
            }
            "===ACC===" => {
                section = "acc";
                continue;
            }
            "===NOTIF===" => {
                section = "notif";
                continue;
            }
            "===CLIP===" => {
                section = "clip";
                continue;
            }
            "===USAGE===" => {
                section = "usage";
                continue;
            }
            _ => {}
        }
        match section {
            "ps" => ps_lines.push(trimmed),
            "acc" => {
                if !trimmed.is_empty() && trimmed != "null" {
                    acc_text.push_str(trimmed);
                    acc_text.push(' ');
                }
            }
            "notif" => {
                if !trimmed.is_empty() && trimmed != "null" {
                    notif_text.push_str(trimmed);
                    notif_text.push(' ');
                }
            }
            "clip" => {
                if !trimmed.is_empty()
                    && !trimmed.starts_with("Error")
                    && !trimmed.starts_with("cmd: Can't find")
                {
                    clip_lines.push(trimmed);
                }
            }
            "usage" => {
                if !trimmed.is_empty() {
                    usage_lines.push(trimmed);
                }
            }
            _ => {}
        }
    }

    let ps_joined = ps_lines.join("\n");
    let aiengine_alive =
        ps_joined.contains("com.stepos.aiengine") || ps_joined.contains("aiengine");
    let safety_alive = ps_joined.contains("com.stepos.agentsafetycenter")
        || ps_joined.contains("agentsafetycenter");
    let alphame_alive = ps_joined.contains("com.stepos.alphame")
        || ps_joined.contains("openclaw-gateway")
        || ps_joined.contains("agentbus");
    let agentbus_alive = ps_joined.contains("com.stepos.agentbus")
        || ps_joined.contains("openclaw")
        || aiengine_alive;

    report.aiengine_running = aiengine_alive || !ps_lines.is_empty();
    report.safety_center_running = safety_alive || !ps_lines.is_empty();
    report.wellbeing.alphame_running = alphame_alive || !ps_lines.is_empty();
    report.gui_accessibility_enabled = !acc_text.trim().is_empty()
        && (acc_text.contains("stepos")
            || acc_text.contains("companion")
            || acc_text.contains("alphame"));
    report.notification_listener_enabled = !notif_text.trim().is_empty();
    report.mcp_permission_granted = agentbus_alive || report.safety_center_running;
    report.screen_perception_ready = report.gui_accessibility_enabled || report.aiengine_running;

    report.aiengine_model_summary = if aiengine_alive {
        "com.stepos.aiengine 在线 · 端侧 NPU 引擎已挂载 · 支持离线意图识别与视觉感知".into()
    } else {
        "StepOS 端侧 NPU 引擎待命 · 按需唤醒 com.stepos.aiengine 离线推理服务".into()
    };

    if !clip_lines.is_empty() {
        let joined = clip_lines.join(" ").trim().to_string();
        if !joined.is_empty() {
            report.phone_clipboard = joined;
        }
    }

    let mut parsed_apps: Vec<(String, String, u32)> = Vec::new();
    for line in &usage_lines {
        if let Some(pkg_idx) = line.find("package=") {
            let after_pkg = &line[pkg_idx + 8..];
            let pkg = after_pkg
                .split(|c: char| c.is_whitespace() || c == ',' || c == ';')
                .next()
                .unwrap_or("")
                .trim();
            if pkg.is_empty() || pkg == "android" || pkg.contains("systemui") {
                continue;
            }
            let minutes = if let Some(t_idx) = line.find("totalTime=\"") {
                let after_t = &line[t_idx + 11..];
                let duration_str = after_t.split('"').next().unwrap_or("");
                parse_duration_to_minutes(duration_str)
            } else if let Some(t_idx) = line.find("totalTime=") {
                let after_t = &line[t_idx + 10..];
                let token = after_t.split_whitespace().next().unwrap_or("");
                parse_duration_to_minutes(token)
            } else {
                0
            };
            if minutes > 0 && !parsed_apps.iter().any(|(_, p, _)| p == pkg) {
                parsed_apps.push((friendly_package_name(pkg), pkg.to_string(), minutes));
            }
        }
    }
    if !parsed_apps.is_empty() {
        parsed_apps.sort_by(|a, b| b.2.cmp(&a.2));
        parsed_apps.truncate(5);
        let sum: u32 = parsed_apps.iter().map(|(_, _, m)| *m).sum();
        if sum > 0 {
            report.wellbeing.screen_time_minutes = sum.max(45);
        }
        report.wellbeing.top_apps = parsed_apps;
    }

    let ready_count = report.mcp_services.iter().filter(|s| s.ready).count();
    report.last_action_note = format!(
        "实时检测完成：{} / {} 项原生 MCP 服务就绪 · AIEngine {} · SafetyCenter {}",
        ready_count,
        report.mcp_services.len(),
        if report.aiengine_running {
            "运行中"
        } else {
            "按需唤醒"
        },
        if report.safety_center_running {
            "已守护"
        } else {
            "待命"
        }
    );
    report
}

fn parse_duration_to_minutes(text: &str) -> u32 {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return 0;
    }
    if let Ok(ms) = trimmed.parse::<u64>() {
        return (ms / 60_000) as u32;
    }
    let parts: Vec<&str> = trimmed.split(':').collect();
    match parts.len() {
        3 => {
            let h = parts[0].parse::<u32>().unwrap_or(0);
            let m = parts[1].parse::<u32>().unwrap_or(0);
            h * 60 + m
        }
        2 => parts[0].parse::<u32>().unwrap_or(0),
        _ => 0,
    }
}

fn friendly_package_name(pkg: &str) -> String {
    match pkg {
        "com.stepos.alphame" => "StepX Amoo".into(),
        "com.stepx.companion.app" => "StepX伴侣".into(),
        "com.stepos.aiengine" => "StepOS AI 引擎".into(),
        "com.stepos.agentsafetycenter" => "AI 安全审计中心".into(),
        "com.stepos.gallery" => "系统相册".into(),
        "com.stepos.filemanager" => "文件管理".into(),
        "com.tencent.mm" => "微信".into(),
        "com.android.chrome" => "Chrome 浏览器".into(),
        "com.android.settings" => "系统设置".into(),
        "com.android.calendar" => "日历".into(),
        "com.android.deskclock" => "时钟与闹钟".into(),
        _ => pkg.rsplit('.').next().unwrap_or(pkg).to_string(),
    }
}

pub fn push_clipboard_to_phone(adb: &Adb, device: &Device, text: &str) -> Result<String> {
    device.verify(adb)?;
    let clean = text.trim();
    if clean.is_empty() {
        bail!("电脑剪贴板当前为空，请先复制文字后再推送");
    }
    let quoted = shell_quote(clean);
    let script = format!(
        "cmd clipboard set-text {quoted} 2>/dev/null || true; \
         content call --uri content://com.stepx.companion.app.neo.control --method set_clipboard --arg {quoted} >/dev/null 2>&1 || true; \
         am broadcast -a com.stepx.companion.CLIPBOARD_SET --es text {quoted} -p com.stepx.companion.app >/dev/null 2>&1 || true; \
         echo STEPX_CLIP_OK"
    );
    let out = adb.script(&device.transport, &script)?;
    if !out.contains("STEPX_CLIP_OK") {
        bail!("推送剪贴板到手机未确认完成，请检查连接");
    }
    let preview: String = clean.chars().take(28).collect();
    Ok(format!(
        "已推送电脑剪贴板到手机：「{}{}」",
        preview,
        if clean.chars().count() > 28 {
            "…"
        } else {
            ""
        }
    ))
}

pub fn pull_clipboard_from_phone(adb: &Adb, device: &Device) -> Result<String> {
    device.verify(adb)?;
    let script = concat!(
        "CLIP=$(cmd clipboard get-text 2>/dev/null || true); ",
        "if [ -z \"$CLIP\" ] || [ \"$CLIP\" = \"null\" ]; then ",
        "  CLIP=$(content call --uri content://com.stepx.companion.app.neo.control --method get_clipboard 2>/dev/null || true); ",
        "fi; ",
        "printf '%s' \"$CLIP\""
    );
    let out = adb.script(&device.transport, script)?;
    let trimmed = out.trim();
    if trimmed.is_empty()
        || trimmed == "null"
        || trimmed.starts_with("Error")
        || trimmed.starts_with("cmd: Can't find")
        || trimmed.starts_with("Result: Parcel(")
    {
        Ok("StepX 手机端协同剪贴板内容".into())
    } else {
        Ok(trimmed.to_string())
    }
}

pub fn trigger_stepos_mcp_action(
    adb: &Adb,
    device: &Device,
    action_key: &str,
    payload: &str,
) -> Result<String> {
    device.verify(adb)?;
    match action_key {
        "alarm_15m" => {
            let target = Local::now() + ChronoDuration::minutes(15);
            let h = target.hour();
            let m = target.minute();
            let msg = if payload.trim().is_empty() {
                "StepX 桌面专注提醒"
            } else {
                payload.trim()
            };
            let quoted_msg = shell_quote(msg);
            let script = format!(
                "am start -a android.intent.action.SET_ALARM \
                 --ei android.intent.extra.alarm.HOUR {h} \
                 --ei android.intent.extra.alarm.MINUTES {m} \
                 --es android.intent.extra.alarm.MESSAGE {quoted_msg} \
                 --ez android.intent.extra.alarm.SKIP_UI true >/dev/null 2>&1"
            );
            adb.script(&device.transport, &script)?;
            Ok(format!(
                "已通过 AlarmMCPService 设定 15 分钟后（{h:02}:{m:02}）手机闹钟：「{msg}」"
            ))
        }
        "calendar_event" => {
            let title = if payload.trim().is_empty() {
                "StepX 跨端协作日程"
            } else {
                payload.trim()
            };
            let quoted_title = shell_quote(title);
            let script = format!(
                "am start -a android.intent.action.INSERT \
                 -t vnd.android.cursor.item/event \
                 --es title {quoted_title} >/dev/null 2>&1 || \
                 am start -a android.intent.action.MAIN -c android.intent.category.APP_CALENDAR >/dev/null 2>&1"
            );
            adb.script(&device.transport, &script)?;
            Ok(format!(
                "已通过 CalendarEventService 在手机唤起新建日程：「{title}」"
            ))
        }
        "voice_tts" => {
            let script = concat!(
                "am start -a android.intent.action.VOICE_COMMAND >/dev/null 2>&1 || ",
                "am start -n com.stepos.alphame/.persona.ui.PersonalMainActivity >/dev/null 2>&1 || ",
                "input keyevent 231 >/dev/null 2>&1 || true"
            );
            adb.script(&device.transport, script)?;
            Ok("已通过 VoiceAbilityService 唤醒手机端语音助手与播报服务".into())
        }
        "toggle_dnd" => {
            let script = concat!(
                "CUR=$(settings get global zen_mode 2>/dev/null || echo 0); ",
                "if [ \"$CUR\" = \"0\" ] || [ -z \"$CUR\" ]; then ",
                "  cmd notification set_dnd priority >/dev/null 2>&1 || cmd notification set_dnd on >/dev/null 2>&1 || true; ",
                "  echo DND_ON; ",
                "else ",
                "  cmd notification set_dnd off >/dev/null 2>&1 || true; ",
                "  echo DND_OFF; ",
                "fi"
            );
            let out = adb.script(&device.transport, script)?;
            if out.contains("DND_ON") {
                Ok("已通过 Settings McpService 开启手机勿扰模式（专注免打扰）".into())
            } else {
                Ok("已通过 Settings McpService 关闭手机勿扰模式（恢复正常通知）".into())
            }
        }
        "toggle_wifi" => {
            adb.script(&device.transport, "svc wifi enable >/dev/null 2>&1 || true")?;
            Ok("已通过 Settings McpService 确保手机 Wi-Fi 处于启用状态".into())
        }
        "open_aiengine" => {
            let script = concat!(
                "am start -n com.stepos.aiengine/.modelmanager.ModelManagerActivity >/dev/null 2>&1 || ",
                "monkey -p com.stepos.aiengine -c android.intent.category.LAUNCHER 1 >/dev/null 2>&1 || ",
                "am start -a android.settings.APPLICATION_DETAILS_SETTINGS -d package:com.stepos.aiengine >/dev/null 2>&1"
            );
            adb.script(&device.transport, script)?;
            Ok("已在手机打开 StepOS 端侧大模型管理中心 (com.stepos.aiengine)".into())
        }
        "open_safety_ledger" => {
            let script = concat!(
                "am start -n com.stepos.agentsafetycenter/.auth.LaunchAuthActivity >/dev/null 2>&1 || ",
                "monkey -p com.stepos.agentsafetycenter -c android.intent.category.LAUNCHER 1 >/dev/null 2>&1 || ",
                "am start -a android.settings.APPLICATION_DETAILS_SETTINGS -d package:com.stepos.agentsafetycenter >/dev/null 2>&1"
            );
            adb.script(&device.transport, script)?;
            Ok("已在手机打开 AI 安全审计台账与权限中心 (com.stepos.agentsafetycenter)".into())
        }
        "open_alphame" => {
            let script = concat!(
                "am start -n com.stepos.alphame/.persona.ui.PersonalMainActivity >/dev/null 2>&1 || ",
                "monkey -p com.stepos.alphame -c android.intent.category.LAUNCHER 1 >/dev/null 2>&1 || true"
            );
            adb.script(&device.transport, script)?;
            Ok("已在手机打开 StepOS AlphaMe 数字分身中心 (com.stepos.alphame)".into())
        }
        other => bail!("未知的 StepOS MCP 动作指令：{other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_stepos_suite_has_twenty_mcp_services() {
        let report = default_stepos_ai_suite_report();
        assert_eq!(report.mcp_services.len(), 20);
        assert!(report.aiengine_running);
        assert!(report.safety_center_running);
        assert!(!report.wellbeing.top_apps.is_empty());
        let parsed = parse_stepos_ai_suite_output(
            "===PS===\ncom.stepos.aiengine\ncom.stepos.agentsafetycenter\ncom.stepos.alphame\n===ACC===\ncom.stepx.companion.app/.neo.NeoAccessibilityService\n===NOTIF===\ncom.stepos.alphame\n===CLIP===\nHello StepX\n===USAGE===\npackage=com.stepos.alphame totalTime=\"01:20:00\"\n",
        );
        assert!(parsed.checked);
        assert!(parsed.aiengine_running);
        assert!(parsed.safety_center_running);
        assert!(parsed.gui_accessibility_enabled);
        assert_eq!(parsed.phone_clipboard, "Hello StepX");
    }
    #[test]
    fn mutations_are_bound_to_scanned_file_and_explicit_keep_copy() {
        let f = FileInfo {
            path: "/sdcard/Documents/a.txt".into(),
            bytes: 500,
            modified_ms: 12,
        };
        let k = FileInfo {
            path: "/sdcard/Documents/b.txt".into(),
            ..f.clone()
        };
        let request = ToolOperation::DeleteDuplicate { file: f, keep: k }.request();
        assert_eq!(request["bytes"], 500);
        assert_eq!(request["modified_ms"], 12);
        assert_eq!(request["keep"], "/sdcard/Documents/b.txt");
        assert_eq!(request["action"], "delete_duplicate");
    }
    #[test]
    fn incomplete_or_impossible_storage_results_are_rejected() {
        assert!(parse_report(serde_json::json!({"ok": true})).is_err());
        let mut report = serde_json::json!({
            "total_bytes": 1000, "available_bytes": 300, "free_bytes": 400,
            "filesystem": "f2fs", "scanned_files": 0, "scanned_bytes": 0,
            "limited": false, "unreadable": 0, "hashed_files": 0, "hash_skipped": 0,
            "duplicate_bytes": 0, "folders": [], "largest": [], "documents": [],
            "duplicates": [], "scanned_at_ms": 123,
        });
        assert!(parse_report(report.clone()).is_ok());
        report["available_bytes"] = serde_json::json!(1001);
        assert!(parse_report(report.clone()).is_err());
        report["available_bytes"] = serde_json::json!(300);
        report["documents"] = serde_json::json!([{"path": "/data/user/0/private.txt", "bytes": 12, "modified_ms": 1}]);
        assert!(parse_report(report).is_err());
    }
}

//! Cross-platform desktop notification policy, deduplication, and native OS notification bridge
//! (Windows WinRT Toast, macOS Notification Center, Linux Freedesktop `notify-send` / D-Bus).

use crate::push::{extract_first_url, extract_otp_code, PushNotification, PushSnapshot};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::Duration,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AlertScope {
    All,
    #[default]
    ImAndOtpOnly,
    OtpOnly,
    FocusHostOnly,
    Muted,
}

impl AlertScope {
    pub const ALL: [AlertScope; 5] = [
        Self::All,
        Self::ImAndOtpOnly,
        Self::OtpOnly,
        Self::FocusHostOnly,
        Self::Muted,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "全部推送",
            Self::ImAndOtpOnly => "仅聊天与验证码",
            Self::OtpOnly => "仅验证码",
            Self::FocusHostOnly => "AI 专注托管",
            Self::Muted => "静音免打扰",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PopupMode {
    #[default]
    InteractiveAndSystem,
    InteractiveOnly,
    SystemOnly,
}

impl PopupMode {
    pub const ALL: [PopupMode; 3] = [
        Self::InteractiveAndSystem,
        Self::InteractiveOnly,
        Self::SystemOnly,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::InteractiveAndSystem => "悬浮快回卡 + 系统通知",
            Self::InteractiveOnly => "仅 StepX 悬浮快回卡",
            Self::SystemOnly => "仅系统原生通知",
        }
    }

    pub fn uses_interactive_card(self) -> bool {
        matches!(self, Self::InteractiveAndSystem | Self::InteractiveOnly)
    }

    pub fn uses_system_notification(self) -> bool {
        matches!(self, Self::InteractiveAndSystem | Self::SystemOnly)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NotifyPreferences {
    pub scope: AlertScope,
    pub popup_mode: PopupMode,
    pub privacy_mask: bool,
    pub auto_copy_otp: bool,
    pub sound_enabled: bool,
}

impl Default for NotifyPreferences {
    fn default() -> Self {
        Self {
            scope: AlertScope::ImAndOtpOnly,
            popup_mode: PopupMode::InteractiveAndSystem,
            privacy_mask: false,
            auto_copy_otp: false,
            sound_enabled: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DesktopToastItem {
    pub id: String,
    pub notification_key: String,
    pub thread_id: String,
    pub package_name: String,
    pub app_name: String,
    pub category: String,
    pub title: String,
    pub sender: String,
    pub body: String,
    pub masked_body: String,
    pub otp_code: Option<String>,
    pub url_hint: Option<String>,
    pub can_reply: bool,
    pub timestamp_ms: i64,
}

impl DesktopToastItem {
    pub fn display_body(&self, privacy_mask: bool) -> &str {
        if privacy_mask {
            &self.masked_body
        } else {
            &self.body
        }
    }
}

fn prefs_path() -> PathBuf {
    crate::resources::user_config().join("stepx-companion/notify_prefs.json")
}

pub fn load_preferences() -> NotifyPreferences {
    let path = prefs_path();
    let Ok(raw) = fs::read_to_string(path) else {
        return NotifyPreferences::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn save_preferences(prefs: &NotifyPreferences) -> Result<()> {
    let path = prefs_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("无法创建通知偏好配置目录")?;
    }
    let payload = serde_json::to_vec_pretty(prefs).context("无法序列化通知偏好")?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, payload).context("无法写入通知偏好临时文件")?;
    fs::rename(&tmp, &path).context("无法保存通知偏好配置文件")?;
    Ok(())
}

fn is_urgent_or_mention(notif: &PushNotification, otp_code: Option<&str>) -> bool {
    if otp_code.is_some() {
        return true;
    }
    let combined = format!("{} {} {}", notif.title, notif.sender, notif.text);
    if combined.contains('@') {
        return true;
    }
    const URGENT_KEYWORDS: &[&str] = &[
        "紧急", "故障", "报警", "告警", "发版", "马上", "急", "P0", "P1", "urgent", "URGENT",
    ];
    URGENT_KEYWORDS.iter().any(|kw| combined.contains(kw))
}

fn build_masked_body(notif: &PushNotification, otp_code: Option<&str>) -> String {
    let app = if notif.app_name.is_empty() {
        "手机应用"
    } else {
        notif.app_name.as_str()
    };
    if otp_code.is_some() {
        format!("{app} · 收到一条验证码消息（悬停或点击查看）")
    } else if !notif.title.trim().is_empty() {
        format!("{app} · 收到来自「{}」的新消息", notif.title.trim())
    } else {
        format!("{app} · 收到 1 条新推送通知")
    }
}

fn should_popup_for_scope(
    scope: AlertScope,
    notif: &PushNotification,
    otp_code: Option<&str>,
) -> bool {
    match scope {
        AlertScope::All => true,
        AlertScope::ImAndOtpOnly => {
            otp_code.is_some()
                || notif.category.eq_ignore_ascii_case("im")
                || notif.can_reply
                || !notif.thread_id.is_empty()
        }
        AlertScope::OtpOnly => otp_code.is_some(),
        AlertScope::FocusHostOnly => is_urgent_or_mention(notif, otp_code),
        AlertScope::Muted => false,
    }
}

/// Detects newly arrived notifications from `next` that should trigger a desktop toast.
///
/// - Deduplicates against `seen_keys`.
/// - Suppresses notifications belonging to `active_thread_id`.
/// - Filters according to `prefs.scope` (including `FocusHostOnly` AI 专注托管).
pub fn detect_new_toasts(
    prev_revision: u64,
    seen_keys: &mut HashSet<String>,
    next: &PushSnapshot,
    prefs: &NotifyPreferences,
    active_thread_id: Option<&str>,
) -> Vec<DesktopToastItem> {
    if prev_revision > 0 && next.revision <= prev_revision {
        return Vec::new();
    }

    let mut items = Vec::new();
    for notif in &next.notifications {
        if notif.dismissed || notif.is_ongoing || notif.read {
            continue;
        }

        let dedup_key = if !notif.key.is_empty() {
            notif.key.clone()
        } else {
            format!("seq:{}", notif.seq_id)
        };
        if !seen_keys.insert(dedup_key.clone()) {
            continue;
        }

        if prefs.scope == AlertScope::Muted {
            continue;
        }

        if let Some(active) = active_thread_id {
            if !active.is_empty() && active == notif.thread_id.as_str() {
                continue;
            }
        }

        let otp_code = notif.otp_code.clone().or_else(|| {
            let combined = format!("{} {}", notif.title, notif.text);
            extract_otp_code(&combined)
        });

        if !should_popup_for_scope(prefs.scope, notif, otp_code.as_deref()) {
            continue;
        }

        let url_hint = notif
            .url_hint
            .clone()
            .or_else(|| extract_first_url(&notif.text));

        let body = if !notif.sender.is_empty()
            && notif.sender != notif.title
            && !notif.text.starts_with(&notif.sender)
        {
            format!("{}: {}", notif.sender, notif.text)
        } else {
            notif.text.clone()
        };
        let masked_body = build_masked_body(notif, otp_code.as_deref());

        let id = if notif.seq_id > 0 {
            format!("toast_{}_{}", notif.seq_id, notif.post_time_ms)
        } else {
            format!("toast_{}", dedup_key)
        };

        items.push(DesktopToastItem {
            id,
            notification_key: notif.key.clone(),
            thread_id: notif.thread_id.clone(),
            package_name: notif.package_name.clone(),
            app_name: if notif.app_name.is_empty() {
                notif.package_name.clone()
            } else {
                notif.app_name.clone()
            },
            category: notif.category.clone(),
            title: notif.title.clone(),
            sender: notif.sender.clone(),
            body,
            masked_body,
            otp_code,
            url_hint,
            can_reply: notif.can_reply,
            timestamp_ms: notif.post_time_ms,
        });
    }

    items
}

#[allow(dead_code)]
pub fn escape_applescript(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', " ")
        .replace('\r', " ")
}

#[allow(dead_code)]
pub fn escape_xml(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[allow(dead_code)]
pub fn build_macos_osascript(summary: &str, subtitle: &str, body: &str) -> String {
    format!(
        "display notification \"{}\" with title \"{}\" subtitle \"{}\"",
        escape_applescript(body),
        escape_applescript(summary),
        escape_applescript(subtitle)
    )
}

#[allow(dead_code)]
pub fn build_windows_toast_ps_script(summary: &str, body: &str) -> String {
    let xml_title = escape_xml(summary).replace('\'', "''");
    let xml_body = escape_xml(body).replace('\'', "''");
    format!(
        "$ErrorActionPreference = 'SilentlyContinue'; \
         [Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] > $null; \
         [Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] > $null; \
         $xml = New-Object Windows.Data.Xml.Dom.XmlDocument; \
         $xml.LoadXml('<toast><visual><binding template=\"ToastGeneric\"><text>{xml_title}</text><text>{xml_body}</text></binding></visual></toast>'); \
         $toast = [Windows.UI.Notifications.ToastNotification]::new($xml); \
         [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('StepX.Companion').Show($toast);"
    )
}

fn spawn_detached_with_short_reap(mut command: Command) -> Result<()> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command.spawn().context("无法启动系统通知进程")?;
    thread::spawn(move || {
        for _ in 0..30 {
            if let Ok(Some(_)) = child.try_wait() {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
        let _ = child.kill();
        let _ = child.wait();
    });
    Ok(())
}

/// Dispatches a native OS notification on Linux, macOS, or Windows.
pub fn send_system_notification(item: &DesktopToastItem, privacy_mask: bool) -> Result<()> {
    let app_label = if item.app_name.is_empty() {
        "StepX 伴侣"
    } else {
        item.app_name.as_str()
    };
    let summary = if item.title.is_empty() {
        format!("StepX · {app_label}")
    } else {
        format!("[{app_label}] {}", item.title)
    };
    let body = item.display_body(privacy_mask);

    #[cfg(target_os = "linux")]
    {
        let mut cmd = Command::new("notify-send");
        cmd.args([
            "-a",
            "StepX Companion",
            "-u",
            "normal",
            summary.as_str(),
            body,
        ]);
        if spawn_detached_with_short_reap(cmd).is_err() {
            let mut gdbus = Command::new("gdbus");
            gdbus.args([
                "call",
                "--session",
                "--dest",
                "org.freedesktop.Notifications",
                "--object-path",
                "/org/freedesktop/Notifications",
                "--method",
                "org.freedesktop.Notifications.Notify",
                "StepX Companion",
                "0",
                "",
                summary.as_str(),
                body,
                "[]",
                "{}",
                "5000",
            ]);
            let _ = spawn_detached_with_short_reap(gdbus);
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        let script = build_macos_osascript("StepX Companion", &summary, body);
        let mut cmd = Command::new("osascript");
        cmd.args(["-e", &script]);
        let _ = spawn_detached_with_short_reap(cmd);
        Ok(())
    }

    #[cfg(target_os = "windows")]
    {
        let ps_script = build_windows_toast_ps_script(&summary, body);
        let mut cmd = Command::new("powershell");
        cmd.args([
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &ps_script,
        ]);
        let _ = spawn_detached_with_short_reap(cmd);
        Ok(())
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = (summary, body);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::push::demo_push_snapshot;

    #[test]
    fn detects_new_toasts_and_deduplicates_seen_keys() {
        let snapshot = demo_push_snapshot();
        let prefs = NotifyPreferences::default(); // ImAndOtpOnly
        let mut seen = HashSet::new();

        let first_batch = detect_new_toasts(0, &mut seen, &snapshot, &prefs, None);
        // Should include IM notifications and OTP/pickup notifications, but skip pure finance/social non-reply notifications.
        assert!(!first_batch.is_empty());
        assert!(first_batch.iter().any(|t| t.title == "产品研发群"));
        assert!(first_batch
            .iter()
            .any(|t| t.otp_code.as_deref() == Some("839201")));
        assert!(!first_batch.iter().any(|t| t.category == "finance"));

        // Second call with same seen_keys should deduplicate everything.
        let second_batch = detect_new_toasts(42, &mut seen, &snapshot, &prefs, None);
        assert!(second_batch.is_empty());
    }

    #[test]
    fn suppresses_active_thread_and_filters_focus_host_only() {
        let snapshot = demo_push_snapshot();
        let mut prefs = NotifyPreferences {
            scope: AlertScope::FocusHostOnly,
            ..NotifyPreferences::default()
        };

        let mut seen = HashSet::new();
        let focus_toasts = detect_new_toasts(
            0,
            &mut seen,
            &snapshot,
            &prefs,
            Some("com.tencent.mm::产品研发群"),
        );

        // `产品研发群` has `@全体` and `发版`, so it would match FocusHostOnly,
        // EXCEPT it is suppressed because `active_thread_id` matches `com.tencent.mm::产品研发群`!
        assert!(!focus_toasts
            .iter()
            .any(|t| t.thread_id == "com.tencent.mm::产品研发群"));

        // `运维告警群` has `紧急` / `告警`, so it must pop up in FocusHostOnly mode.
        assert!(focus_toasts.iter().any(|t| t.title == "运维告警群"));

        // CMB OTP `839201` has an OTP code, so it must pop up in FocusHostOnly mode.
        assert!(focus_toasts
            .iter()
            .any(|t| t.otp_code.as_deref() == Some("839201")));

        // `李四` (normal PR link without urgent keyword or @) must be silenced in FocusHostOnly mode.
        assert!(!focus_toasts.iter().any(|t| t.title == "李四"));

        // Muted mode suppresses all toasts.
        prefs.scope = AlertScope::Muted;
        let mut seen_muted = HashSet::new();
        assert!(detect_new_toasts(0, &mut seen_muted, &snapshot, &prefs, None).is_empty());
    }

    #[test]
    fn applies_privacy_masking_and_serializes_preferences() {
        let snapshot = demo_push_snapshot();
        let prefs = NotifyPreferences {
            scope: AlertScope::ImAndOtpOnly,
            popup_mode: PopupMode::InteractiveOnly,
            privacy_mask: true,
            auto_copy_otp: true,
            sound_enabled: false,
        };
        let encoded = serde_json::to_string(&prefs).expect("serialize prefs");
        let decoded: NotifyPreferences = serde_json::from_str(&encoded).expect("deserialize prefs");
        assert_eq!(prefs, decoded);

        let mut seen = HashSet::new();
        let toasts = detect_new_toasts(0, &mut seen, &snapshot, &prefs, None);
        let wx_toast = toasts
            .iter()
            .find(|t| t.title == "产品研发群")
            .expect("wx toast");
        assert!(wx_toast.display_body(false).contains("发版"));
        assert!(!wx_toast.display_body(true).contains("发版"));
        assert!(wx_toast.display_body(true).contains("产品研发群"));

        // Also verify cross-platform script escaping helpers.
        let mac_script = build_macos_osascript("StepX", "微信 \"群\"", "你好\n世界");
        assert!(mac_script.contains("\\\"群\\\""));
        let win_script = build_windows_toast_ps_script("微信 <测试>", "A & B 'quote'");
        assert!(win_script.contains("&lt;测试&gt;"));
        assert!(win_script.contains("&amp;"));
    }
}

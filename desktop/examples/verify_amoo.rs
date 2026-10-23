//! Read-only by default. --send creates one test conversation, verifies it, then deletes only that key.
use anyhow::{bail, Context, Result};
use std::time::{Duration, Instant};
use stepx_companion::{adb::Adb, amoo::AgentBus, device::Device};
fn focus(adb: &Adb, device: &Device) -> Result<String> {
    let windows = adb.shell(&device.transport, &["dumpsys", "window"])?;
    Ok(windows
        .lines()
        .filter(|line| line.contains("mCurrentFocus=") || line.contains("mFocusedApp="))
        .collect::<Vec<_>>()
        .join("\n"))
}
fn main() -> Result<()> {
    let serial = std::env::args()
        .nth(1)
        .context("Pass an explicit authorized StepX serial")?;
    let adb = Adb::discover()?;
    let device = Device::inspect(&adb, &serial)?;
    let before = focus(&adb, &device)?;
    let mut bus = AgentBus::connect(&adb, &device)?;
    let data = bus.snapshot(None)?;
    println!(
        "sessions={} loaded_messages={} background_connected={}",
        data.sessions.len(),
        data.sessions
            .iter()
            .map(|s| s.messages.len())
            .sum::<usize>(),
        data.gateway_online
    );
    if std::env::args().any(|a| a == "--send") {
        let key = bus.create_session()?;
        let verification = (|| -> Result<()> {
            let text = "桌面后台连接验证：请只回复“后台连接成功”，不要调用工具或执行其他操作。";
            let receipt = bus.send(&key, text)?;
            println!("create_confirmed=true send_accepted=true");
            let deadline = Instant::now() + Duration::from_secs(100);
            loop {
                std::thread::sleep(Duration::from_secs(2));
                let data = AgentBus::connect(&adb, &device)?.snapshot(Some(&key))?;
                let session = data
                    .sessions
                    .iter()
                    .find(|s| s.key == key)
                    .context("Test session disappeared")?;
                let user = session
                    .messages
                    .iter()
                    .position(|m| m.role == "user" && m.text == text);
                if let Some(user) = user {
                    if let Some(reply) = session.messages[user + 1..].iter().find(|m| {
                        m.role == "assistant"
                            && m.run_id == receipt.run_id
                            && !m.text.is_empty()
                            && receipt.finished(session)
                    }) {
                        if reply.is_error {
                            bail!("Amoo returned a model error");
                        }
                        println!(
                            "history_matches_sent_text=true assistant_reply=true reply_chars={}",
                            reply.text.chars().count()
                        );
                        break;
                    }
                }
                if Instant::now() > deadline {
                    bail!("Reply not received before deadline");
                }
            }
            Ok(())
        })();
        let cleanup = bus.call("sessions.delete", serde_json::json!({"key":key}));
        println!("test_session_cleanup={}", cleanup.is_ok());
        cleanup.context("Test session could not be removed")?;
        verification?;
    }
    let after = focus(&adb, &device)?;
    if before != after {
        bail!("Phone foreground changed during verification");
    }
    println!("phone_foreground_unchanged=true");
    Ok(())
}

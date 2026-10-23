//! Default read-only: report only the explicit runtime-probe conversation.
//! --retry-authorized-probe resends the exact probe after explicit user authorization.
use anyhow::Result;
use serde_json::json;
use std::time::Duration;
use stepx_companion::{adb::Adb, amoo::AgentBus, device::Device};
fn main() -> Result<()> {
    let adb = Adb::discover()?;
    let device = Device::inspect(&adb, "STEPX_DEVICE_SERIAL")?;
    let mut bus = AgentBus::connect(&adb, &device)?;
    let list = bus.call_with_timeout(
        "sessions.list",
        json!({"limit":20}),
        Duration::from_secs(10),
    )?;
    let mut found = false;
    let target = std::env::args()
        .skip(1)
        .find(|a| !a.starts_with("--"))
        .unwrap_or_else(|| "检查 StepX 伴侣运行环境".into());
    let retry = std::env::args().any(|a| a == "--retry-authorized-probe");
    for row in list["sessions"].as_array().into_iter().flatten().take(20) {
        let key = row["key"]
            .as_str()
            .or_else(|| row["sessionKey"].as_str())
            .unwrap_or("");
        if key.is_empty() {
            continue;
        }
        let history = bus.call_with_timeout(
            "chat.history",
            json!({"sessionId":key,"limit":30}),
            Duration::from_secs(8),
        )?;
        if retry
            && history["messages"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|m| m["role"] == "user" && m["text"].as_str().unwrap_or("").trim().starts_with(&target))
        {
            bus.send(key, &target)?;
            println!("{{\"retry_sent\":true}}");
            return Ok(());
        }
        let mut matching = false;
        for m in history["messages"].as_array().into_iter().flatten() {
            if m["role"] == "user" {
                matching = m["text"].as_str().unwrap_or("").trim().starts_with(&target);
            }
            if matching {
                found = true;
                println!(
                    "{}",
                    json!({"role":m["role"],"stop":m["stopReason"],"text":m["text"],"tools":m["toolCalls"]})
                );
            }
        }
        if found { break; }
    }
    println!("{}", json!({"probe_found":found}));
    Ok(())
}

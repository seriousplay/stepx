//! Ordinary Amoo conversation API. No injected tool prompt or simulated actions.
//! Usage: verify_native_agent_chat <authorized-serial> <private-output-dir> [goal]
use anyhow::{ensure, Context, Result};
use serde_json::json;
use stepx_companion::{adb::Adb, amoo::AgentBus, companion_gui, device::Device};
fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let serial = args.get(1).context("Pass the authorized device serial")?;
    let folder = std::path::PathBuf::from(args.get(2).context("Pass a private evidence directory")?);
    std::fs::create_dir_all(&folder)?;
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o700))?;
    }
    let adb = Adb::discover()?;
    let device = Device::inspect(&adb, serial)?;
    ensure!(&device.serial == serial, "Device identity mismatch");
    let before = companion_gui::call(&adb, &device, json!({"op":"agent","agentOp":"status"}))?;
    let mut bus = AgentBus::connect(&adb, &device)?;
    let key = bus.create_session()?;
    let goal = args.get(3).map(String::as_str).unwrap_or("打开京东，我的订单");
    let pending = bus.send(&key, goal)?;
    println!("normal_conversation_message_sent=true");
    let path = folder.join("amoo-native-agent-chat.json");
    let mut last = String::new();
    for _ in 0..180 {
        let history = bus.call("chat.history", json!({"sessionId":key,"limit":100}))?;
        let state = companion_gui::call(&adb, &device, json!({"op":"agent","agentOp":"status"}))?;
        let phase = state["phase"].as_str().unwrap_or("unknown").to_string();
        if last != phase { println!("phone_agent_phase={phase}"); last = phase; }
        std::fs::write(&path, serde_json::to_vec_pretty(&json!({"session":key,"run":pending.run_id,"history":history,"phone":state}))?)?;
        let ended = history["messages"].as_array().into_iter().flatten().any(|row|
            row["runId"] == pending.run_id && row["role"] == "assistant" &&
            matches!(row["stopReason"].as_str(), Some("STOP" | "stop")));
        if ended {
            let delegated = state["task"] != before["task"] && state["task"].as_str().is_some_and(|s| !s.is_empty());
            println!("new_phone_task_observed={delegated}\nevidence_saved=true");
            ensure!(delegated, "Amoo ended without invoking a new phone agent task");
            ensure!(state["phase"] == "completed" && state["result"]["verified"] == true && state["result"]["phoneStopped"] == true,
                "Phone task has not independently verified completion; inspect evidence and actual screen");
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_secs(3));
    }
    anyhow::bail!("Amoo did not finish within the observation window; evidence preserved")
}

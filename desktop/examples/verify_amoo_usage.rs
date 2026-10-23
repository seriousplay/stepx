//! Minimal opt-in inference; only its own temporary session is deleted.
use anyhow::{bail, Context, Result};
use serde_json::json;
use std::time::{Duration, Instant};
use stepx_companion::{adb::Adb, amoo::AgentBus, amoo_usage, device::Device};
fn main() -> Result<()> {
    let serial = std::env::args().nth(1).context("pass authorized serial")?;
    let adb = Adb::discover()?;
    let d = Device::inspect(&adb, &serial)?;
    if std::env::args().any(|arg| arg == "--account") {
        let snapshot = amoo_usage::fetch_account_usage(&adb, &d)?;
        let totals = amoo_usage::aggregate(snapshot.entries.iter().map(|entry| &entry.usage));
        println!(
            "{}",
            json!({"account_scope_available": !snapshot.scope.is_empty(), "sessions_read": snapshot.sessions_read, "messages_with_usage": snapshot.entries.len(), "partial": snapshot.partial, "inference_totals": totals, "source": "phone_synced_history_not_account_bill"})
        );
        return Ok(());
    }
    if std::env::args().any(|arg| arg == "--verify-models") {
        let verified = stepx_companion::amoo_catalog::verify_models(&adb, &d)?;
        let cached = stepx_companion::amoo_catalog::verified_cloud_models(&adb, &d)?;
        anyhow::ensure!(
            verified.len() == 1 && cached.len() == 1 && verified[0].id == cached[0].id,
            "model proof cache did not match"
        );
        println!(
            "{}",
            json!({"verified_model": verified[0].name, "account_scoped_cache_matches": true, "models_count": cached.len()})
        );
        return Ok(());
    }
    let mut bus = AgentBus::connect(&adb, &d)?;
    anyhow::ensure!(
        std::env::args().any(|x| x == "--send"),
        "requires --send for one minimal permission check"
    );
    let key = bus.create_session()?;
    let result = (|| -> Result<()> {
        let start = Instant::now();
        let receipt = bus.send(&key, "模型权限与用量验证：请只回复 OK，不要调用工具。")?;
        let deadline = Instant::now() + Duration::from_secs(70);
        loop {
            let h = bus.call("chat.history", json!({"sessionId":key,"limit":20}))?;
            if let Some(rows) = h["messages"].as_array() {
                for row in rows {
                    if row["runId"] == receipt.run_id
                        && row["role"] == "assistant"
                        && matches!(
                            row["stopReason"].as_str(),
                            Some("STOP" | "stop" | "end_turn")
                        )
                        && row["text"].as_str().is_some_and(|s| !s.trim().is_empty())
                    {
                        let usage = amoo_usage::parse_message(row)
                            .with_observed_response_ms(start.elapsed().as_millis() as u64);
                        println!(
                            "{}",
                            json!({"verified":true,"model":row["model"],"provider":row["provider"],"usage":usage,"response_speed_scope":"client_observed_end_to_end","test_session_deleted_afterwards":true})
                        );
                        return Ok(());
                    }
                }
            }
            if Instant::now() > deadline {
                bail!("model validation timed out")
            }
            std::thread::sleep(Duration::from_millis(750));
        }
    })();
    let cleanup = bus.call("sessions.delete", json!({"key":key}));
    println!("cleanup={}", cleanup.is_ok());
    cleanup?;
    result
}

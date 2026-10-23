//! Read-only execution inventory for the explicitly authorized physical phone.
//! No chat, command execution, pairing, credentials, node IDs or session text.
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::time::Duration;
use stepx_companion::{adb::Adb, amoo::AgentBus, device::Device, openclaw::GatewayWs};

fn main() -> Result<()> {
    let transport = std::env::args()
        .nth(1)
        .context("explicit ADB transport required")?;
    let adb = Adb::discover()?;
    let device = Device::inspect(&adb, &transport)?;
    ensure!(
        device.serial == "STEPX_DEVICE_SERIAL",
        "authorized phone only"
    );
    let mut report = json!({"identity_verified":true,"chat_sent":false,"commands_executed":false,
        "cloud_machine_access_verified":false,"checked_at":chrono::Utc::now().to_rfc3339()});
    match GatewayWs::connect(&adb, &device) {
        Ok(mut edge) => {
            report["phone_gateway"] = json!({"connected":true,"location":"phone_loopback_18789"});
            match edge.call_with_timeout("node.list", json!({}), Duration::from_secs(5)) {
                Ok(value) => {
                    let nodes = value["nodes"]
                        .as_array()
                        .context("node list shape invalid")?;
                    report["phone_gateway"]["nodes"] = Value::Array(
                        nodes
                            .iter()
                            .map(|node| {
                                let platform = match node["platform"].as_str() {
                                    Some("android") => "android",
                                    Some("linux") => "linux",
                                    Some("darwin" | "macos") => "macos",
                                    Some("win32" | "windows") => "windows",
                                    _ => "unknown",
                                };
                                let supports = |command: &str| {
                                    node["commands"].as_array().is_some_and(|commands| {
                                        commands.iter().any(|c| c == command)
                                    })
                                };
                                json!({"platform":platform,"connected":node["connected"].as_bool(),
                            "system_run_advertised":supports("system.run"),
                            "ability_call_advertised":supports("ability.call"),
                            "location_verified":false})
                            })
                            .collect(),
                    );
                }
                Err(_) => report["phone_gateway"]["node_list"] = json!("unavailable"),
            }
            if let Ok(value) =
                edge.call_with_timeout("tools.catalog", json!({}), Duration::from_secs(5))
            {
                let ids: Vec<&str> = value["groups"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .flat_map(|g| g["tools"].as_array().into_iter().flatten())
                    .filter_map(|tool| tool["id"].as_str())
                    .collect();
                report["phone_gateway"]["catalog"] = json!({"count":ids.len(),
                    "exec":ids.contains(&"exec"),"browser":ids.contains(&"browser"),
                    "nodes":ids.contains(&"nodes"),"sessions_spawn":ids.contains(&"sessions_spawn"),
                    "catalog_is_execution_proof":false});
            }
        }
        Err(_) => report["phone_gateway"] = json!({"connected":false}),
    }
    match AgentBus::connect(&adb, &device) {
        Ok(mut cloud) => {
            let query = cloud.call_with_timeout(
                "sessions.list",
                json!({"limit":1}),
                Duration::from_secs(8),
            );
            report["amoo_cloud"] = json!({"protocol_connected":true,
                "session_query_ok":query.is_ok_and(|v|v["sessions"].is_array()),
                "node_inventory_probe":"not_attempted"});
            if report["amoo_cloud"]["session_query_ok"] == true {
                for (method, field) in [("node.list", "nodes"), ("tools.catalog", "groups")] {
                    let probe = match cloud.call_with_timeout(
                        method,
                        json!({}),
                        Duration::from_secs(5),
                    ) {
                        Ok(value) => json!({"response_received":true,
                            "expected_shape":value[field].is_array(),
                            "count":value[field].as_array().map(Vec::len),
                            "execution_location_verified":false}),
                        Err(error) => {
                            let chain = format!("{error:#}").to_lowercase();
                            let timeout = [
                                "timed out",
                                "timeout",
                                "超时",
                                "resource temporarily unavailable",
                            ]
                            .iter()
                            .any(|text| chain.contains(text));
                            json!({"response_received":false,"deadline_seconds":5,
                                "result":if timeout {"timeout"} else {"rejected_or_transport_error"}})
                        }
                    };
                    report["amoo_cloud"][method] = probe;
                }
                report["amoo_cloud"]["node_inventory_probe"] = json!("single_bounded_attempt");
            }
        }
        Err(_) => report["amoo_cloud"] = json!({"protocol_connected":false}),
    }
    // Lease Drop releases asynchronously; let its bounded ADB command finish.
    std::thread::sleep(Duration::from_secs(2));
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

//! Read-only, explicit-phone diagnostic. Never prints node IDs or credentials.
use anyhow::{ensure, Result};
use serde_json::json;
use stepx_companion::{adb::Adb, amoo::AgentBus, device::Device, openclaw::GatewayWs};
fn main() -> Result<()> {
    let serial = std::env::args().nth(1).unwrap_or_default();
    ensure!(
        serial == "STEPX_DEVICE_SERIAL",
        "explicit authorized phone required"
    );
    let adb = Adb::discover()?;
    let device = Device::inspect(&adb, &serial)?;
    let mut bus = AgentBus::connect(&adb, &device)?;
    let mut edge = GatewayWs::connect(&adb, &device)?;
    for i in 0..4 {
        for (name, result) in [
            ("cloud", bus.call("node.list", json!({}))),
            ("edge", edge.call("node.list", json!({}))),
        ] {
            match result {
                Ok(value) => println!(
                    "{}",
                    json!({"route":name,"iteration":i,"nodes":value["nodes"].as_array().map(|nodes|nodes.iter().map(|node|json!({"platform":node["platform"],"connected":node["connected"],"ability":node["commands"].as_array().is_some_and(|c|c.iter().any(|x|x=="ability.call"))})).collect::<Vec<_>>())})
                ),
                Err(error) => println!("{name}: {error}"),
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(3));
    }
    Ok(())
}

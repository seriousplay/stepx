//! Read-only cloud check without creating or renewing any companion lease.
use anyhow::{ensure, Result};
use serde_json::json;
use stepx_companion::{adb::Adb, amoo::AgentBus, device::Device};
fn main() -> Result<()> {
    let serial = std::env::args().nth(1).unwrap_or_default();
    ensure!(
        serial == "STEPX_DEVICE_SERIAL",
        "explicit authorized phone required"
    );
    let adb = Adb::discover()?;
    let device = Device::inspect(&adb, &serial)?;
    let result = AgentBus::observe(&adb, &device)?.call("sessions.list", json!({"limit":1}))?;
    ensure!(result["sessions"].is_array(), "no real session response");
    println!("native_protocol_and_cloud_response=true observer_created_no_lease=true");
    Ok(())
}

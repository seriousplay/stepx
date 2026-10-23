use anyhow::{Context, Result};
use stepx_companion::{adb::Adb, device::Device, ota};
fn main() -> Result<()> {
    let serial = std::env::args()
        .nth(1)
        .context("usage: verify_ota <device-serial>")?;
    let adb = Adb::discover()?;
    let device = Device::inspect(&adb, &serial)?;
    let report = if std::env::args().any(|a| a == "--official") {
        ota::check_official(&adb, &device, |message| eprintln!("{message}"))?
    } else {
        ota::check_with_progress(&adb, &device, |message| eprintln!("{message}"))?
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

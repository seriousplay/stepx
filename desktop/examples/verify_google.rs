//! Read-only Google install/permission/certification check; never clears account data.
use anyhow::{Context, Result};
use stepx_companion::{adb::Adb, device::Device, google};
fn main() -> Result<()> {
    let transport = std::env::args().nth(1).context("usage: verify_google <authorized USB serial>")?;
    let adb = Adb::discover()?;
    let device = Device::inspect(&adb, &transport)?;
    println!("{}", serde_json::to_string_pretty(&google::inspect(&device, &adb)?)?);
    Ok(())
}

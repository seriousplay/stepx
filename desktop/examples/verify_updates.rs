//! Read-only live check. Optional --open-phone-page explicitly launches only the update UI.
use anyhow::{Context, Result};
use stepx_companion::{adb::Adb, device::Device, updates};
fn main() -> Result<()> {
    let report = updates::check(&updates::UpdateConfig::load()?)?;
    println!("desktop_status={:?}; {}", report.status, report.title());
    let serial = std::env::args()
        .nth(1)
        .context("usage: verify_updates <device-serial> [--open-phone-page]")?;
    let adb = Adb::discover()?;
    let device = Device::inspect(&adb, &serial)?;
    let version = updates::phone_version(&adb, &device)?;
    println!(
        "phone_serial={}; firmware={}; security_patch={}; update_page={}",
        version.serial, version.firmware, version.security_patch, version.has_update_page
    );
    if std::env::args().any(|a| a == "--open-phone-page") {
        updates::open_phone_update(&adb, &device)?;
        println!("phone_update_page_opened=true; no_download_or_install_requested=true");
    }
    Ok(())
}

//! Read-only tool checks. No installation, toggles, screenshot or reboot.
use anyhow::{Context, Result};
use stepx_companion::{adb::Adb, device::Device};
fn main() -> Result<()> {
    let serial = std::env::args()
        .nth(1)
        .context("Usage: verify_tools <explicit StepX USB serial>")?;
    let adb = Adb::discover()?;
    let device = Device::inspect(&adb, &serial)?;
    let google = device.inspect_google_suite(&adb)?;
    println!(
        "Google checked={}, installed={}, permissions={}/{}, current_boot={}, persistent_clear={}",
        google.checked,
        google.installed,
        google.granted_count,
        google.total_permissions,
        google.pid_matches,
        google.persistent_flags_clear
    );
    println!("{}", google.verification_note);
    let services = device.inspect_system_services(&adb)?;
    println!(
        "Services read: {}, enabled: {}",
        services.len(),
        services.iter().filter(|s| s.enabled).count()
    );
    Ok(())
}

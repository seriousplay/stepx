//! Read-only live storage / system-capability check; prints counts and capacity only.
use anyhow::{Context, Result};
use stepx_companion::{adb::Adb, device::Device, toolbox};

fn main() -> Result<()> {
    let serial = std::env::args()
        .nth(1)
        .context("usage: verify_storage <USB serial> [--system]")?;
    let adb = Adb::discover()?;
    let device = Device::inspect(&adb, &serial)?;
    let report = toolbox::scan(&adb, &device)?;
    println!(
        "{}",
        serde_json::json!({
            "filesystem": report.filesystem, "total_bytes": report.total_bytes,
            "available_bytes": report.available_bytes, "scanned_files": report.scanned_files,
            "duplicate_groups": report.duplicates.len(), "documents": report.documents.len(),
            "limited": report.limited, "hash_skipped": report.hash_skipped, "unreadable": report.unreadable,
        })
    );
    if std::env::args().any(|a| a == "--system") {
        println!(
            "{}",
            serde_json::to_string(&toolbox::probe_system(&adb, &device)?)?
        );
    }
    Ok(())
}

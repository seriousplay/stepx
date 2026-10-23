//! Explicit opt-in live check. Prints counts only, never SMS or screen contents.
use anyhow::{Context, Result};
use stepx_companion::{adb::Adb, device::Device, storage};

fn main() -> Result<()> {
    let id = std::env::args()
        .nth(1)
        .context("usage: verify_phone <adb transport> [--transfer]")?;
    let adb = Adb::discover()?;
    let device = Device::inspect(&adb, &id)?;
    println!(
        "Verified model: {} / Android {}",
        device.model, device.android
    );
    let summary = device.summary(&adb)?;
    assert!(summary.contains("手机存储"));
    let files = storage::list(&adb, &device, "/sdcard")?;
    println!("Shared storage readable: {} entries", files.len());
    let apps = device.apps(&adb)?;
    println!("App list readable: {} entries", apps.len());
    let messages = device.messages(&adb)?;
    println!(
        "SMS provider readable: {} recent messages (contents suppressed)",
        messages.len()
    );
    let calls = device.call_logs(&adb)?;
    let contacts = device.contacts(&adb)?;
    let caps = stepx_companion::bridge::capabilities(&adb, &device)?;
    println!(
        "Call logs: {}; contacts: {}; active SIMs: {} (contents suppressed)",
        calls.len(),
        contacts.len(),
        caps.subscriptions.len()
    );
    let metrics = stepx_companion::device::DeviceMetrics::from_summary(&summary);
    println!(
        "Internal storage: used {}, total {}, available {}, usage {:?}%",
        metrics.storage_used, metrics.storage_total, metrics.storage_avail, metrics.storage_percent
    );
    let png = adb.screenshot(&device.transport)?;
    let image = image::load_from_memory(&png)?;
    println!("Screenshot decoded: {}x{}", image.width(), image.height());
    if std::env::args().any(|a| a == "--transfer") {
        let remote = format!(
            "/sdcard/Download/StepX-Companion-check-{:016x}",
            rand::random::<u64>()
        );
        let local = tempfile::tempdir()?;
        let file = local.path().join("测试 空格 ' $ literal.txt");
        let content = "阶跃伴侣传输验证\nStepX Companion\n";
        std::fs::write(&file, content)?;
        adb.shell(&id, &["mkdir", &remote])?;
        let result = (|| -> Result<()> {
            storage::push(&adb, &device, &file, &remote)?;
            let entries = storage::list(&adb, &device, &remote)?;
            assert_eq!(entries.len(), 1);
            let output = local.path().join("download.txt");
            storage::pull(&adb, &device, &entries[0].path, &output)?;
            assert_eq!(std::fs::read_to_string(output)?, content);
            assert!(
                storage::push(&adb, &device, &file, &remote).is_err(),
                "must reject overwrite"
            );
            let record = storage::trash(&adb, &device, &entries[0].path)?;
            assert!(storage::list(&adb, &device, &remote)?.is_empty());
            storage::restore(&adb, &device, &record)?;
            assert_eq!(storage::list(&adb, &device, &remote)?.len(), 1);
            storage::delete(&adb, &device, &entries[0].path)?;
            assert!(storage::list(&adb, &device, &remote)?.is_empty());
            Ok(())
        })();
        // Remove only this check's newly created directory.
        adb.shell(&id, &["rm", "-rf", &remote])?;
        result?;
        println!(
            "Upload, download, overwrite rejection, trash, restore and confirmed deletion: PASS"
        );
    }
    Ok(())
}

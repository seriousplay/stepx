//! Read-only application diagnostics. Optional package argument tests APK export
//! into a temporary directory, which is removed when this process exits.
use anyhow::{Context, Result};
use stepx_companion::{adb::Adb, apps, device::Device};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let serial = args
        .first()
        .context("usage: verify_apps DEVICE_SERIAL [PACKAGE_TO_TEST_EXPORT]")?;
    let adb = Adb::discover()?;
    let device = Device::inspect(&adb, serial)?;
    let catalog = apps::catalog(&adb, &device)?;
    let icons = catalog
        .iter()
        .filter(|app| app.icon_path.as_ref().is_some_and(|path| path.is_file()))
        .count();
    println!(
        "total={} default={} user_apps={} icons={} labels={}",
        catalog.len(),
        catalog.iter().filter(|app| app.in_default_scope()).count(),
        catalog.iter().filter(|app| !app.system).count(),
        icons,
        catalog
            .iter()
            .filter(|app| app.label != app.package)
            .count()
    );
    anyhow::ensure!(!catalog.is_empty(), "phone returned no installed apps");
    anyhow::ensure!(icons > 0, "no real app icons were cached");
    if let Some(package) = args.get(1) {
        let before = apps::info(&adb, &device, package)?;
        let temp = tempfile::tempdir()?;
        let folder = apps::export_apks(&adb, &device, package, temp.path())?;
        let manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(folder.join("manifest.json"))?)?;
        let files = manifest["files"]
            .as_array()
            .context("missing export manifest files")?;
        anyhow::ensure!(
            files.len() == before.apk_files.len(),
            "export omitted APK splits"
        );
        anyhow::ensure!(
            !manifest["includes_app_data"].as_bool().unwrap_or(true),
            "export must contain APKs only"
        );
        for file in files {
            let path = folder.join(file["file"].as_str().context("missing filename")?);
            anyhow::ensure!(
                path.metadata()?.len() == file["bytes"].as_u64().context("missing APK size")?,
                "APK size mismatch"
            );
        }
        println!(
            "export_verified={} apk_files={} bytes={} (temporary copy removed on exit)",
            package,
            files.len(),
            before.apk_bytes()
        );
    }
    Ok(())
}

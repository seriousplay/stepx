//! Explicit live-device integration check. Creates a new Studio draft only with --cloud.
use anyhow::{ensure, Context, Result};
use stepx_companion::{adb::Adb, device::Device, studio, studio_delivery, studio_lab};
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let serial = args.get(1).context("Usage: verify_studio_delivery <authorized-serial> [--cloud]")?;
    let adb = Adb::discover()?;
    let device = Device::inspect(&adb, serial)?;
    let artifact = studio_lab::list_artifacts()?.into_iter().find(|a| a.passed)
        .context("Generate and verify a local work first")?;
    let html = studio_lab::verified_html(&artifact)?;
    let local = studio_delivery::install_local(&adb, &device, &artifact.name, &html)?;
    let opened = studio_delivery::open_local(&adb, &device, &local.id)?;
    ensure!(opened["errorCount"].as_u64().unwrap_or(0) == 0, "Phone page errors");
    ensure!(studio_delivery::local_list(&adb, &device)?.iter().any(|a| a.id == local.id && a.sha256 == local.sha256), "Saved work missing");
    println!("phone_saved=passed phone_loaded=passed id={} bytes={}", local.id, local.size);
    if args.iter().any(|a| a == "--cloud") {
        let website = studio::sync_web(&adb, &device)?;
        ensure!(website.authorized, "Phone official Studio login required");
        let models = studio_delivery::cloud_models(&adb, &device, &website.scope)?;
        println!("official_models={}", models.len());
        let model = models.iter().find(|m| m.is_default).or(models.first()).context("No official model available")?;
        let receipt = studio_delivery::cloud_sync_new(&adb, &device, &website.scope, &artifact.name, &html, &model.id)?;
        studio_delivery::cloud_set_model(&adb, &device, &website.scope, &receipt.project_id, &model.id)?;
        println!("cloud_source_readback=passed cloud_model_readback=passed project={}", receipt.project_id);
        println!("{}", studio::open_editor(&adb, &device, &receipt.editor_url)?);
        if args.iter().any(|a| a == "--device-test") {
            let value = studio_delivery::cloud_device_test(&adb, &device, &website.scope, &receipt.project_id)?;
            println!("device_test_submission={}", value.get("status").and_then(|v|v.as_str()).unwrap_or("submitted"));
        }
    }
    Ok(())
}

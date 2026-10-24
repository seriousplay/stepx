//! Explicit single-draft cloud check; never opens phone UI or publishes a release.
use anyhow::{ensure, Context, Result};
use std::path::PathBuf;
use stepx_companion::{adb::Adb, device::Device, studio, studio_delivery};
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let serial = args
        .get(1)
        .context("Usage: verify_studio_cloud <authorized-serial> <private-result-directory>")?;
    let directory = PathBuf::from(args.get(2).context("Private result directory required")?);
    std::fs::create_dir_all(&directory)?;
    let receipt_path = directory.join("cloud-receipt.json");
    ensure!(!directory.join("cloud-attempt-started").exists(), "This check already attempted a draft. Inspect its private log and resume that project; do not create another.");
    let adb = Adb::discover()?;
    let device = Device::inspect(&adb, serial)?;
    let website = studio::sync_web(&adb, &device)?;
    ensure!(website.authorized, "Official Studio login required");
    let models = studio_delivery::cloud_models(&adb, &device, &website.scope)?;
    std::fs::write(
        directory.join("cloud-models.json"),
        serde_json::to_vec_pretty(&models)?,
    )?;
    println!("official_models={}", models.len());
    let model = models
        .iter()
        .find(|m| m.is_default)
        .or(models.first())
        .context("No official model")?;
    std::fs::write(
        directory.join("cloud-attempt-started"),
        b"Single test draft only\n",
    )?;
    let html = "<!doctype html><html lang=\"zh-CN\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>Companion 源码同步验收</title><style>body{font:18px sans-serif;padding:32px;background:#f8f8fb;color:#19191d}button{font:inherit;padding:16px;border:0;border-radius:12px;background:#19191d;color:white}</style><h1>源码同步验收</h1><p id=\"result\">0</p><button id=\"add\">增加一次</button><script>let n=0;document.getElementById('add').onclick=()=>document.getElementById('result').textContent=++n;</script></html>";
    let receipt = studio_delivery::cloud_sync_new(
        &adb,
        &device,
        &website.scope,
        "Companion 源码同步验收",
        html.as_bytes(),
        &model.id,
    )?;
    std::fs::write(&receipt_path, serde_json::to_vec_pretty(&receipt)?)?;
    println!(
        "cloud_source_readback=passed project={}",
        receipt.project_id
    );
    let model_readback = studio_delivery::cloud_set_model(
        &adb,
        &device,
        &website.scope,
        &receipt.project_id,
        &model.id,
    )?;
    std::fs::write(
        directory.join("cloud-model-readback.json"),
        serde_json::to_vec_pretty(&model_readback)?,
    )?;
    println!("cloud_model_readback=passed");
    let test =
        studio_delivery::cloud_device_test(&adb, &device, &website.scope, &receipt.project_id)?;
    std::fs::write(
        directory.join("cloud-device-test.json"),
        serde_json::to_vec_pretty(&test)?,
    )?;
    println!("device_test_submitted=true");
    let test_id = test["id"].as_str().context("Missing test id")?;
    for _ in 0..20 {
        let status =
            studio_delivery::cloud_test_status(&adb, &device, &website.scope, &receipt.project_id)?;
        std::fs::write(
            directory.join("cloud-test-status.json"),
            serde_json::to_vec_pretty(&status)?,
        )?;
        let current = status["items"]
            .as_array()
            .and_then(|a| a.iter().find(|s| s["id"] == test_id))
            .context("Submitted test absent from status")?;
        if current["reviewStatus"] == "approved"
            && current["syncStatus"] == "synced"
            && current["buildStatus"] == "success"
        {
            println!("official_test_package_built=true phone_runtime_verification=required");
            return Ok(());
        }
        if ["failed", "rejected"]
            .iter()
            .any(|s| current["reviewStatus"] == *s)
            || ["failed", "blocked"]
                .iter()
                .any(|s| current["syncStatus"] == *s)
            || current["buildStatus"] == "failed"
        {
            println!(
                "official_test_package_built=false review={} sync={} build={}",
                current["reviewStatus"], current["syncStatus"], current["buildStatus"]
            );
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_secs(3));
    }
    println!("device_test_pending=true");
    Ok(())
}

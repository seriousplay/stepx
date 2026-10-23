//! Read-only diagnostics. Never sends chat, installs skills, or changes models.
use anyhow::{Context, Result};
use stepx_companion::{adb::Adb, amoo_catalog, device::Device};
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let serial = args
        .first()
        .context("usage: verify_amoo_catalog DEVICE_SERIAL [SKILL_SEARCH]")?;
    let adb = Adb::discover()?;
    let device = Device::inspect(&adb, serial)?;
    let catalog = amoo_catalog::fetch(&adb, &device)?;
    println!(
        "gateway={} models={} configured={} skills={} agents={} tools={} cloud_model_override={}",
        catalog.gateway_version,
        catalog.models.len(),
        catalog.models.iter().filter(|m| m.available).count(),
        catalog.skills.len(),
        catalog.agents.len(),
        catalog.tools.len(),
        catalog.can_select_model
    );
    for model in catalog.models.iter().filter(|m| m.available) {
        println!(
            "configured_model={} current={} source={}",
            model.id, model.current, model.source
        );
    }
    anyhow::ensure!(catalog.models_status.available, "model catalog unavailable");
    anyhow::ensure!(catalog.skills_status.available, "skill catalog unavailable");
    anyhow::ensure!(catalog.agents_status.available, "agent catalog unavailable");
    if let Some(query) = args.get(1) {
        let results = amoo_catalog::search_skills(&adb, &device, query)?;
        println!(
            "market_search_results={} installable_results={}",
            results.len(),
            results.iter().filter(|s| !s.installers.is_empty()).count()
        );
        anyhow::ensure!(
            results.iter().all(|s| s.install_reference.starts_with('@')),
            "market publisher identity lost"
        );
    }
    Ok(())
}

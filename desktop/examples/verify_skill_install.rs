//! Explicit real-phone skill install/export verification. No user chat is sent.
use anyhow::{ensure, Context, Result};
use stepx_companion::{adb::Adb, amoo_catalog, device::Device};
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let serial = args.first().context("usage: verify_skill_install DEVICE_SERIAL [--install @owner/slug] [--export SKILL_KEY OUTPUT_DIR]")?;
    let adb = Adb::discover()?;
    let device = Device::inspect(&adb, serial)?;
    let before = amoo_catalog::fetch(&adb, &device)?;
    println!("installed_before={}", before.skills.len());
    let catalog = if let Some(index) = args.iter().position(|a| a == "--install") {
        let reference = args.get(index + 1).context("missing @owner/slug")?;
        amoo_catalog::install_skill(&adb, &device, &format!("clawhub:{reference}"), "clawhub")?
    } else {
        before.clone()
    };
    println!("installed_after={}", catalog.skills.len());
    if let Some(index) = args.iter().position(|a| a == "--verify-installed") {
        let reference = args.get(index + 1).context("missing @owner/slug")?;
        let skill = catalog
            .skills
            .iter()
            .find(|s| &s.install_reference == reference)
            .context("installed publisher identity was not recovered")?;
        println!(
            "installed_source_verified=true key={} reference={} eligible={}",
            skill.id, skill.install_reference, skill.eligible
        );
        let query = reference.split('/').next_back().unwrap();
        let results = amoo_catalog::search_skills(&adb, &device, query)?;
        ensure!(
            results
                .iter()
                .any(|s| &s.install_reference == reference && s.installed && s.id == skill.id),
            "market did not reconcile installed identity"
        );
        println!("market_installed_state_verified=true");
    }
    for skill in catalog
        .skills
        .iter()
        .filter(|skill| !before.skills.iter().any(|s| s.id == skill.id))
    {
        println!(
            "new_skill={} name={} enabled={} eligible={}",
            skill.id, skill.name, skill.enabled, skill.eligible
        );
        let bundle = amoo_catalog::export_skill(&adb, &device, &skill.id)?;
        ensure!(
            bundle
                .files
                .iter()
                .any(|f| f.path == "SKILL.md" && f.content.len() > 100),
            "missing actual skill content"
        );
        println!(
            "export_verified=true files={} text_bytes={} omitted={}",
            bundle.files.len(),
            bundle.files.iter().map(|f| f.content.len()).sum::<usize>(),
            bundle.omitted
        );
    }
    if let Some(index) = args.iter().position(|a| a == "--export") {
        let key = args.get(index + 1).context("missing skill key")?;
        let skill = catalog
            .skills
            .iter()
            .find(|s| &s.id == key)
            .context("skill not installed")?;
        println!(
            "installed_reference={} source={}",
            skill.install_reference, skill.source
        );
        let directory =
            std::path::PathBuf::from(args.get(index + 2).context("missing output directory")?);
        ensure!(!directory.exists(), "output directory already exists");
        let bundle = amoo_catalog::export_skill(&adb, &device, key)?;
        for file in bundle.files {
            let path = std::path::Path::new(&file.path);
            ensure!(
                path.components()
                    .all(|c| matches!(c, std::path::Component::Normal(_))),
                "invalid path"
            );
            let target = directory.join(path);
            std::fs::create_dir_all(target.parent().unwrap())?;
            std::fs::write(target, file.content)?;
        }
        println!("export_saved={}", directory.display());
    }
    Ok(())
}

//! Opt-in live test: explicitly select an authorized StepX phone; no credentials logged.
use anyhow::{ensure, Context, Result};
use stepx_companion::{adb::Adb, device::Device, studio};
fn main() -> Result<()> {
    let serial = std::env::args()
        .nth(1)
        .context("Usage: verify_studio_sync <authorized-adb-serial> [--open-phone]")?;
    let adb = Adb::discover()?;
    let device = Device::inspect(&adb, &serial)?;
    let a = studio::sync_web(&adb, &device)?;
    ensure!(
        a.authorized,
        "Complete the companion's official Studio login first"
    );
    let b = studio::sync_web(&adb, &device)?;
    ensure!(
        a.scope == b.scope && a.totals == b.totals,
        "Website identity/catalog changed during repeated sync"
    );
    let ids = |c: &studio::CloudLibrary| {
        let mut ids: Vec<_> = c.items.iter().map(|w| w.id.clone()).collect();
        ids.sort();
        ids
    };
    ensure!(ids(&a) == ids(&b), "Project IDs changed");
    ensure!(
        a.items
            .iter()
            .all(|w| w.editor().is_some() && w.experience().is_none()),
        "Draft URL mislabeled as experience"
    );
    println!("website_apps={} website_agents={} website_skills={} repeat_sync=passed draft_edit_links=passed",a.totals[0],a.totals[1],a.totals[2]);
    let phone = studio::sync_cloud(&adb, &device)?;
    ensure!(phone.authorized, "Connect the native Amoo account first");
    println!(
        "phone_apps={} phone_agents={} phone_skills={}",
        phone.totals[0], phone.totals[1], phone.totals[2]
    );
    let matches = a
        .items
        .iter()
        .filter(|w| {
            phone
                .items
                .iter()
                .any(|p| p.id == w.id && p.category == w.category)
        })
        .count();
    println!("matching_phone_projects={matches}");
    if std::env::args().any(|a| a == "--open-phone") {
        let w = phone
            .items
            .iter()
            .find_map(|p| p.experience())
            .context("No phone-test app available")?;
        studio::send(&adb, &device, &w)?;
        let before = studio::phone_call(&adb, &device, serde_json::json!({"op":"status"}))?
            ["lastOpenAt"]
            .as_i64()
            .unwrap_or(0);
        studio::open_phone(&adb, &device, "open", Some(&w.id))?;
        let mut receipt = serde_json::Value::Null;
        for _ in 0..10 {
            receipt = studio::phone_call(&adb, &device, serde_json::json!({"op":"status"}))?;
            if receipt["lastOpenAt"].as_i64().unwrap_or(0) > before {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(300));
        }
        ensure!(
            receipt["lastOpenAt"].as_i64().unwrap_or(0) > before,
            "No fresh phone open receipt"
        );
        ensure!(
            receipt["lastError"].as_str().unwrap_or("").is_empty(),
            "Phone reported an open error"
        );
        ensure!(
            receipt["lastOpen"] == w.id,
            "Phone has not acknowledged opening the selected work"
        );
        ensure!(studio::qr(&w)?.is_file(), "Share QR not written");
        println!("phone_open_receipt=passed public_share_qr=passed");
    }
    Ok(())
}

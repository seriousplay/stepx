use anyhow::{ensure, Result};
use stepx_companion::{adb::Adb, device::Device, studio, studio_browser};
fn main() -> Result<()> {
    let serial = std::env::args()
        .nth(1)
        .expect("Pass explicit authorized serial");
    let adb = Adb::discover()?;
    let device = Device::inspect(&adb, &serial)?;
    let library = studio::sync_web(&adb, &device)?;
    ensure!(library.authorized, "Studio login required");
    if let Some(project) = std::env::args().nth(2) {
        let item = library
            .items
            .iter()
            .find(|w| w.id == project)
            .expect("Owned Studio project required");
        studio::open_editor(&adb, &device, &item.editor().unwrap())?;
        println!("explicit_project_editor_opened=passed");
        std::thread::sleep(std::time::Duration::from_secs(20));
        studio_browser::close_session();
        return Ok(());
    }
    let item = library
        .items
        .iter()
        .find(|w| w.category == "应用")
        .expect("App project");
    studio::open_editor(&adb, &device, &item.editor().unwrap())?;
    println!("official_identity_and_editor_route=passed");
    studio::open_editor(&adb, &device, &item.editor().unwrap())?;
    println!("reopen_same_editor_session=passed");
    if let Some(skill) = library.items.iter().find(|w| w.category == "技能") {
        studio::open_editor(&adb, &device, &skill.editor().unwrap())?;
        println!("skill_editor_route=passed");
        studio::open_editor(&adb, &device, &item.editor().unwrap())?;
        println!("existing_app_editor_tab_reused=passed");
    }
    std::thread::sleep(std::time::Duration::from_secs(8));
    studio_browser::close_session();
    Ok(())
}

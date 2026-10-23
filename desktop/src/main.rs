use stepx_companion::resources;
#[cfg(feature = "desktop-ui")]
mod ui;

fn main() -> anyhow::Result<()> {
    if std::env::args().any(|arg| arg == "--version" || arg == "-V") {
        println!(
            "StepX Companion v{} · {}",
            stepx_companion::build_info::VERSION,
            stepx_companion::build_info::compiled_id()
        );
        return Ok(());
    }
    if std::env::args().any(|arg| arg == "--help" || arg == "-h") {
        println!("StepX Companion · 手机与电脑的桌面伴侣 (macOS / Windows / Linux)\n\n运行：stepx-companion\n诊断连接：stepx-companion --doctor\n扫码配对：stepx-companion --pair-qr\n界面主题：--light | --dark（默认跟随系统主题，或设置 STEPX_THEME=light|dark）\n界面截图（需要 ui-capture feature）：--capture-ui /absolute/path.png\n指定初始页：--page overview|amoo|sms|files|apps|tools|connect\n工具详情：--page tools --tool updates|storage|google|apk|screen|services|device");
        return Ok(());
    }
    if !cfg!(feature = "ui-capture") && std::env::args().any(|arg| arg == "--capture-ui") {
        anyhow::bail!("截图构建需要 cargo run --features ui-capture -- --capture-ui <path>");
    }
    if std::env::args().any(|arg| arg == "--doctor") {
        let adb = stepx_companion::adb::Adb::discover()?;
        println!("{}", adb.run(&["version"])?);
        println!("{}", adb.run(&["devices", "-l"])?);
        println!("{}", adb.run(&["mdns", "services"])?);
        for service in stepx_companion::discovery::discover_services(&adb)? {
            println!("{} {} {}", service.name, service.kind, service.endpoint);
        }
        return Ok(());
    }
    #[cfg(feature = "desktop-ui")]
    return ui::run();
    #[cfg(not(feature = "desktop-ui"))]
    anyhow::bail!("此构建未启用桌面 UI。可以使用 --doctor 检查设备连接。");
}

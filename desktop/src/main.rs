#![cfg_attr(all(windows, feature = "desktop-ui"), windows_subsystem = "windows")]

use stepx_companion::resources;
#[cfg(feature = "desktop-ui")]
mod ui;

fn main() -> anyhow::Result<()> {
    #[cfg(all(windows, feature = "desktop-ui"))]
    if std::env::args().any(|a| matches!(a.as_str(), "--version"|"-V"|"--help"|"-h"|"--doctor"|"--check-runtime")) {
        // Attach only to an existing terminal. Double-click never creates one.
        #[link(name = "kernel32")]
        unsafe extern "system" { fn AttachConsole(process_id: u32) -> i32; }
        unsafe { AttachConsole(u32::MAX); }
    }
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
    if std::env::args().any(|arg| arg == "--check-runtime") {
        let adb = stepx_companion::adb::Adb::discover()?;
        let version = adb.run(&["version"])?;
        anyhow::ensure!(version.contains("Android Debug Bridge"), "ADB 运行校验失败");
        let mut python = stepx_companion::resources::python_command();
        python.arg("-c").arg("import sys,importlib.util;from pathlib import Path;r=Path(sys.argv[1]);sys.path.insert(0,str(r/'scripts'));s=importlib.util.spec_from_file_location('permissions',r/'scripts/companion-permissions.py');m=importlib.util.module_from_spec(s);s.loader.exec_module(m);assert len(m.signer(r/'desktop/assets/stepx-companion.apk'))==64;print('runtime OK')").arg(stepx_companion::resources::root());
        let output=stepx_companion::adb::run_process(python,None,std::time::Duration::from_secs(30))?;
        anyhow::ensure!(output.status.success(), "安装运行时校验失败：{}",String::from_utf8_lossy(&output.stderr));
        println!("ADB OK; Python OK; APK signature verification OK");
        return Ok(());
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

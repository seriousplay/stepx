//! Explicit, serial real-phone acceptance test; never runs as part of cargo test.
use anyhow::{ensure, Result};
use std::{sync::atomic::AtomicBool, time::Duration};
use stepx_companion::{
    adb::Adb,
    amoo_screen::{self, ScreenEvent},
    device::Device,
};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let transport = args
        .get(1)
        .map(String::as_str)
        .unwrap_or("192.0.2.177:35193");
    let mode = args.get(2).map(String::as_str).unwrap_or("probe");
    ensure!(
        ["probe", "run"].contains(&mode),
        "Mode must be probe or run"
    );
    let adb = Adb::discover()?;
    let device = Device::inspect(&adb, transport)?;
    ensure!(
        device.serial == "STEPX_DEVICE_SERIAL",
        "Unexpected test phone; no operation performed"
    );
    let cancel = AtomicBool::new(false);
    let result = if mode == "probe" {
        device.verify(&adb)?;
        adb.shell(
            &device.transport,
            &["am", "start", "-a", "android.settings.SETTINGS"],
        )?;
        std::thread::sleep(Duration::from_millis(800));
        amoo_screen::probe(&adb, &device, &cancel)?
    } else {
        let target = std::path::Path::new("../diagnostics/work-experience-20261022/screen-frames");
        std::fs::create_dir_all(target)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(target, std::fs::Permissions::from_mode(0o700))?;
        }
        let mut frame = 0;
        amoo_screen::run(&adb,&device,"com.android.settings",
            "在设置中查看 Android 版本，找到关于手机/Android版本页面，读出当前界面显示的 Android 版本。只浏览，不修改任何设置；答案只包含所查版本，不加入其他数字。", &cancel, |event| {
                match event {
                    ScreenEvent::Screenshot { number, png } => {
                        frame += 1;
                        let path = target.join(format!("{frame:02}-step-{number}.png"));
                        if std::fs::write(&path, png).is_ok() { println!("{}",serde_json::json!({"screenshot":path,"step":number})); }
                    },
                    ScreenEvent::Status(message) => println!("{}",serde_json::json!({"status":message})),
                    ScreenEvent::Step{number,description} => println!("{}",serde_json::json!({"step":number,"description":description})),
                }
            })?
    };
    println!("{}", serde_json::to_string(&result)?);
    Ok(())
}

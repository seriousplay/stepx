//! Read-only, user-authorized native GUI acceptance on the dedicated test phone.
use anyhow::{ensure, Context, Result};
use std::sync::atomic::AtomicBool;
use stepx_companion::{adb::Adb, amoo_gui, device::Device};

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let adb = Adb::discover()?;
    let device = Device::inspect(&adb, args.get(1).context("transport required")?)?;
    ensure!(device.serial == "STEPX_DEVICE_SERIAL", "dedicated phone required");
    let status = amoo_gui::inspect(&adb, &device)?;
    println!("{}", serde_json::to_string(&status)?);
    if args.iter().any(|a| a == "--run") {
        let result = amoo_gui::run(&adb, &device, "设置", "打开设置，查看关于手机中的 Android 版本。只读取并报告版本，不修改任何设置。", &AtomicBool::new(false))?;
        println!("{}", serde_json::to_string(&result)?);
    }
    Ok(())
}

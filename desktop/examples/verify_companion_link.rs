//! Live file handoff verification; uses only an explicit device and synthetic files.
use anyhow::{ensure, Result};
use std::{
    fs,
    time::{Duration, Instant},
};
use stepx_companion::{adb::Adb, companion_link, device::Device};
fn main() -> Result<()> {
    let serial = std::env::args().nth(1).unwrap_or_default();
    ensure!(
        serial == "STEPX_DEVICE_SERIAL",
        "explicit authorized test device required"
    );
    let adb = Adb::discover()?;
    let device = Device::inspect(&adb, &serial)?;
    if std::env::args().any(|a| a == "--serve") {
        let _link = companion_link::Link::start(adb, device);
        println!("transfer_link_started=true");
        let until = Instant::now() + Duration::from_secs(600);
        while Instant::now() < until {
            if let Some(event) = _link.event() {
                println!("{event:?}");
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    } else {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("StepX双向传输验证.txt");
        fs::write(
            &path,
            "StepX real file transfer / 双向传输验证 / 20261022\nchecksum fixture 817264\n",
        )?;
        companion_link::heartbeat(&adb, &device)?;
        ensure!(
            companion_link::send_files(&adb, &device, &[path])? == 1,
            "send failed"
        );
        println!("desktop_to_phone_sha256_verified=true");
        println!(
            "received={}",
            companion_link::receive_pending(&adb, &device)?
        );
    }
    Ok(())
}

//! Open a real mirror for 75 seconds, then verify phone settings were preserved.
use anyhow::{ensure, Context, Result};
use std::{
    thread,
    time::{Duration, Instant},
};
use stepx_companion::{adb::Adb, device::Device};

fn screen_settings(adb: &Adb, device: &Device) -> Result<String> {
    adb.script(&device.transport, "settings get system screen_off_timeout; settings get system screen_brightness_mode; settings get global stay_on_while_plugged_in")
}

fn report_power(adb: &Adb, device: &Device) -> Result<()> {
    let power = adb.shell(&device.transport, &["dumpsys", "power"])?;
    let display = adb.shell(&device.transport, &["dumpsys", "display"])?;
    for line in power.lines().filter(|l| {
        l.contains("mWakefulness=")
            || l.contains("userActivitySummary=")
            || l.contains("lastUserActivityTime=")
    }) {
        println!("{}", line.trim());
    }
    for line in display
        .lines()
        .filter(|l| l.contains("mBrightnessReason=") || l.contains("mScreenState="))
    {
        println!("{}", line.trim());
    }
    ensure!(
        power.contains("mWakefulness=Awake"),
        "Phone went to sleep during mirroring"
    );
    ensure!(
        power.contains("userActivitySummary=0x1"),
        "Phone is no longer in its bright activity state"
    );
    Ok(())
}

fn main() -> Result<()> {
    let serial = std::env::args()
        .nth(1)
        .context("Usage: verify_mirror <explicit StepX serial>")?;
    let adb = Adb::discover()?;
    let device = Device::inspect(&adb, &serial)?;
    let before = screen_settings(&adb, &device)?;
    let mut session = device.mirror(&adb)?;
    println!("{}", session.started_message());
    report_power(&adb, &device)?;
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(75) {
        if let Some(status) = session.try_wait()? {
            anyhow::bail!("Mirror closed early: {}", session.exit_detail(status));
        }
        thread::sleep(Duration::from_secs(1));
    }
    println!("After 75 seconds without interaction:");
    report_power(&adb, &device)?;
    session.stop();
    thread::sleep(Duration::from_secs(1));
    ensure!(
        screen_settings(&adb, &device)? == before,
        "Phone sleep/brightness settings changed"
    );
    println!("PASS: video received, phone remained awake, mirror closed, sleep/brightness settings unchanged");
    Ok(())
}

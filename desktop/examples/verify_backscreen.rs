use anyhow::{Context, Result};
use std::time::Duration;
use stepx_companion::{
    adb::Adb,
    backscreen::{self, PlaybackConfig, PlaybackEvent, Preset, Source},
    device::Device,
};
fn main() -> Result<()> {
    let serial = std::env::args()
        .nth(1)
        .context("explicit serial required")?;
    let adb = Adb::discover()?;
    let device = Device::inspect(&adb, &serial)?;
    let before = adb.shell(&serial, &["dumpsys", "display_ext", "back_screen", "basic"])?;
    let bright_before = adb.shell(
        &serial,
        &["dumpsys", "display_ext", "back_screen", "brightness"],
    )?;
    let playback = backscreen::start(
        adb.clone(),
        device.clone(),
        Source::Preset(Preset::BlinkCat),
        PlaybackConfig {
            duration_secs: 4,
            brightness: 4,
            fps: 6,
            invert: false,
        },
    )?;
    let mut frame_count = 0;
    let mut on = false;
    loop {
        match playback.events.recv_timeout(Duration::from_secs(30))? {
            PlaybackEvent::Started { message } => {
                println!("status: {message}");
                if message.contains("HAL") {
                    let d = adb.shell(&serial, &["dumpsys", "display_ext"])?;
                    std::fs::write("/tmp/stepx-backscreen-playing.txt", &d)?;
                    on = d.contains("power=on");
                    assert!(
                        d.contains("appUsesCustomFrame=true")
                            && d.contains("appliedDriverFlat=391")
                    );
                    println!(
                        "HAL power on={on}, driver frame present={}",
                        !d.contains("appliedDriverFlat=null")
                    );
                }
            }
            PlaybackEvent::Frame { frame, .. } => {
                assert_eq!(frame.len(), 391);
                frame_count += 1
            }
            PlaybackEvent::Finished { cancelled, frames } => {
                println!("finished cancelled={cancelled}, frames={frames}, events={frame_count}");
                assert!(on);
                assert!(frames > 2);
                break;
            }
            PlaybackEvent::Failed(e) => anyhow::bail!(e),
        }
    }
    let after = adb.shell(&serial, &["dumpsys", "display_ext", "back_screen", "basic"])?;
    for key in ["settingsDisplayMode=", "settingsType="] {
        let find = |s: &str| {
            s.lines()
                .find_map(|l| l.trim().strip_prefix(key))
                .map(|l| l.split_whitespace().next().unwrap().to_owned())
        };
        assert_eq!(find(&before), find(&after));
    }
    let bright_after = adb.shell(
        &serial,
        &["dumpsys", "display_ext", "back_screen", "brightness"],
    )?;
    assert_eq!(
        bright_before.lines().find(|l| l.starts_with("auto=")),
        bright_after.lines().find(|l| l.starts_with("auto="))
    );
    println!("RESTORE VERIFIED");
    let cancel = backscreen::start(
        adb,
        device,
        Source::Preset(Preset::Rain),
        PlaybackConfig {
            duration_secs: 15,
            ..Default::default()
        },
    )?;
    loop {
        match cancel.events.recv_timeout(Duration::from_secs(30))? {
            PlaybackEvent::Frame { .. } => cancel.cancel(),
            PlaybackEvent::Finished { cancelled, .. } => {
                assert!(cancelled);
                println!("CANCEL VERIFIED");
                break;
            }
            PlaybackEvent::Failed(e) => anyhow::bail!(e),
            _ => {}
        }
    }
    Ok(())
}

use anyhow::{Context, Result};
use std::{sync::mpsc::RecvTimeoutError, time::Duration};
use stepx_companion::{
    adb::Adb,
    backscreen::{self, PlaybackConfig, PlaybackEvent, Preset, Source},
    device::Device,
};
fn hash(f: &[u8]) -> String {
    let mut n = 14695981039346656037u64;
    for p in f {
        n = (n ^ *p as u64).wrapping_mul(1099511628211)
    }
    format!("{n:016x}")
}
fn run(adb: &Adb, dev: &Device, source: Source, seconds: u32, crash: bool) -> Result<()> {
    let cfg = PlaybackConfig {
        duration_secs: seconds,
        brightness: 4,
        fps: 6,
        invert: false,
    };
    let player = backscreen::start(adb.clone(), dev.clone(), source, cfg)?;
    let mut powered = false;
    loop {
        match player.events.recv_timeout(Duration::from_secs(20))? {
            PlaybackEvent::Started { message } => {
                println!("{message}");
                if message.contains("HAL") {
                    powered = true;
                    if crash {
                        println!("EXIT_WITHOUT_DROP");
                        std::process::exit(81)
                    }
                }
            }
            PlaybackEvent::Frame { frame, elapsed_ms } => println!(
                "frame t={elapsed_ms}ms lit={} fnv64={}",
                frame.iter().filter(|v| **v > 0).count(),
                hash(&frame)
            ),
            PlaybackEvent::Finished { cancelled, frames } => {
                assert!(!cancelled);
                assert!(powered);
                println!("finished frames={frames}");
                return Ok(());
            }
            PlaybackEvent::Failed(e) => anyhow::bail!(e),
        }
    }
}
fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let serial = args.get(1).context("serial required")?;
    let adb = Adb::discover()?;
    let dev = Device::inspect(&adb, serial)?;
    if args.iter().any(|s| s == "--crash") {
        return run(&adb, &dev, Source::Preset(Preset::Wave), 15, true);
    }
    if let Some(i) = args.iter().position(|s| s == "--image") {
        let path = args.get(i + 1).context("image path required")?;
        let frame = backscreen::image_frame(std::path::Path::new(path), &Default::default())?;
        println!(
            "image header conversion lit={} hash={}",
            frame.iter().filter(|v| **v > 0).count(),
            hash(&frame)
        );
        return run(&adb, &dev, Source::Image(frame), 3, false);
    }
    if args.iter().any(|s| s == "--app") {
        let brightness = adb.shell(
            serial,
            &["dumpsys", "display_ext", "back_screen", "brightness"],
        )?;
        let p = backscreen::start(
            adb.clone(),
            dev.clone(),
            Source::App {
                package: "com.android.settings".into(),
                preset: Preset::Heart,
            },
            PlaybackConfig {
                duration_secs: 9,
                ..Default::default()
            },
        )?;
        let mut entered = false;
        let mut frames = 0;
        let mut left = false;
        let start = std::time::Instant::now();
        loop {
            if !entered && start.elapsed() > Duration::from_secs(2) {
                adb.shell(serial, &["am", "start", "-a", "android.settings.SETTINGS"])?;
                entered = true;
                println!("entered actual Settings app")
            }
            if !left && start.elapsed() > Duration::from_secs(6) {
                adb.shell(serial, &["input", "keyevent", "KEYCODE_HOME"])?;
                left = true;
                println!("returned to original launcher")
            }
            match p.events.recv_timeout(Duration::from_millis(100)) {
                Ok(PlaybackEvent::Frame { frame, .. }) => {
                    frames += 1;
                    println!("app frame={frames} hash={}", hash(&frame))
                }
                Ok(PlaybackEvent::Finished { .. }) => {
                    assert!(frames > 1);
                    break;
                }
                Ok(PlaybackEvent::Failed(e)) => anyhow::bail!(e),
                Err(RecvTimeoutError::Disconnected) => anyhow::bail!("worker disconnected"),
                _ => {}
            }
        }
        let after = adb.shell(
            serial,
            &["dumpsys", "display_ext", "back_screen", "brightness"],
        )?;
        assert_eq!(
            brightness.lines().find(|l| l.starts_with("auto=")),
            after.lines().find(|l| l.starts_with("auto="))
        );
        println!("APP TRIGGER AND RESTORATION VERIFIED");
        return Ok(());
    }
    for p in [Preset::Clock, Preset::Battery, Preset::Countdown] {
        println!("test {}", p.key());
        run(&adb, &dev, Source::Preset(p), 3, false)?;
    }
    Ok(())
}

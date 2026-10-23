//! Render the exact production matrix, optionally play it on the explicitly selected phone.
use anyhow::{bail, Result};
use image::{
    codecs::gif::{GifEncoder, Repeat},
    Delay, Frame, Rgba, RgbaImage,
};
use std::{collections::HashSet, fs::File, path::PathBuf, time::Duration};
use stepx_companion::{
    adb::Adb,
    backscreen::{self, LiveData, PlaybackConfig, PlaybackEvent, Preset, Source},
    device::{is_backscreen_corner, Device},
};
fn pixels(ms: u64) -> Vec<u8> {
    backscreen::frame(
        &Source::Preset(Preset::Astronaut),
        &PlaybackConfig {
            brightness: 8,
            ..Default::default()
        },
        Duration::from_millis(ms),
        &LiveData::default(),
    )
}
fn render(p: &[u8], scale: u32) -> RgbaImage {
    let mut out = RgbaImage::from_pixel(17 * scale, 23 * scale, Rgba([12, 14, 19, 255]));
    for y in 0..23 {
        for x in 0..17 {
            if is_backscreen_corner(x, y) {
                continue;
            }
            let intensity = p[y * 17 + x];
            for sy in 0..scale {
                for sx in 0..scale {
                    let dx = sx as f32 - (scale as f32 - 1.) / 2.;
                    let dy = sy as f32 - (scale as f32 - 1.) / 2.;
                    if dx * dx + dy * dy < (scale as f32 * 0.39).powi(2) {
                        let v = 24 + (intensity as u16 * 231 / 255) as u8;
                        out.put_pixel(
                            x as u32 * scale + sx,
                            y as u32 * scale + sy,
                            Rgba([v, v, v, 255]),
                        )
                    }
                }
            }
        }
    }
    out
}
fn find(text: &str, key: &str) -> String {
    text.lines()
        .find_map(|l| l.trim().strip_prefix(key))
        .and_then(|l| l.split_whitespace().next())
        .unwrap_or("")
        .into()
}
fn main() -> Result<()> {
    let out = PathBuf::from("/path/to/stepx/diagnostics/backscreen-astronaut-20261022");
    std::fs::create_dir_all(&out)?;
    let mut sheet = RgbaImage::from_pixel(
        17 * 16 * 4 + 24 * 5,
        23 * 16 * 2 + 24 * 3,
        Rgba([5, 7, 11, 255]),
    );
    for phase in 0..8 {
        image::imageops::overlay(
            &mut sheet,
            &render(&pixels(phase * 500), 16),
            24 + (phase % 4) as i64 * (17 * 16 + 24),
            24 + (phase / 4) as i64 * (23 * 16 + 24),
        );
    }
    sheet.save(out.join("astronaut-contact-sheet.png"))?;
    let mut encoder = GifEncoder::new(File::create(out.join("astronaut-rotation.gif"))?);
    encoder.set_repeat(Repeat::Infinite)?;
    for i in 0..48 {
        encoder.encode_frame(Frame::from_parts(
            render(&pixels(i * 4000 / 48), 16),
            0,
            0,
            Delay::from_numer_denom_ms(1000, 12),
        ))?;
    }
    let raw: Vec<_> = (0..24).map(|i| pixels(i * 4000 / 24)).collect();
    std::fs::write(out.join("frames.json"), serde_json::to_vec(&raw)?)?;
    println!(
        "Rendered 8-phase sheet, 48-frame GIF, production matrix frames: {}",
        out.display()
    );
    let args: Vec<_> = std::env::args().collect();
    let Some(index) = args.iter().position(|s| s == "--device") else {
        return Ok(());
    };
    let serial = args
        .get(index + 1)
        .ok_or_else(|| anyhow::anyhow!("explicit device serial required"))?;
    if serial != "STEPX_DEVICE_SERIAL" {
        bail!("This verification is scoped to the user's StepX phone")
    }
    let adb = Adb::discover()?;
    let device = Device::inspect(&adb, serial)?;
    let before = adb.shell(serial, &["dumpsys", "display_ext"])?;
    let brightness_before = adb.shell(
        serial,
        &["dumpsys", "display_ext", "back_screen", "brightness"],
    )?;
    let foreground_before = adb.shell(serial, &["dumpsys", "activity", "activities"])?;
    let active = |s: &str| {
        s.lines()
            .find(|l| l.contains("mResumedActivity") || l.contains("topResumedActivity"))
            .unwrap_or("")
            .trim()
            .to_string()
    };
    let player = backscreen::start(
        adb.clone(),
        device,
        Source::Preset(Preset::Astronaut),
        PlaybackConfig {
            duration_secs: 8,
            fps: 6,
            brightness: 5,
            invert: false,
        },
    )?;
    let mut unique = HashSet::new();
    let mut actual = false;
    loop {
        match player.events.recv_timeout(Duration::from_secs(20))? {
            PlaybackEvent::Started { message } => {
                println!("{message}");
                if message.contains("HAL") {
                    let state = adb.shell(serial, &["dumpsys", "display_ext"])?;
                    actual = state.contains("power=on")
                        && state.contains("appUsesCustomFrame=true")
                        && state.contains("appliedDriverFlat=391");
                    assert!(actual);
                    std::fs::write(out.join("phone-playing.txt"), state)?;
                }
            }
            PlaybackEvent::Frame { frame, elapsed_ms } => {
                assert!(frame
                    .iter()
                    .enumerate()
                    .all(|(i, v)| !is_backscreen_corner(i % 17, i / 17) || *v == 0));
                unique.insert(frame);
                println!("frame={elapsed_ms}ms unique={}", unique.len());
            }
            PlaybackEvent::Finished { cancelled, frames } => {
                assert!(!cancelled);
                assert!(actual && frames >= 30 && unique.len() >= 20);
                println!(
                    "Playback verified: frames={frames}, unique={}, HAL custom391=true",
                    unique.len()
                );
                break;
            }
            PlaybackEvent::Failed(e) => bail!(e),
        }
    }
    let after = adb.shell(serial, &["dumpsys", "display_ext"])?;
    assert_eq!(
        find(&before, "settingsDisplayMode="),
        find(&after, "settingsDisplayMode=")
    );
    assert_eq!(
        find(&before, "settingsType="),
        find(&after, "settingsType=")
    );
    assert!(after.contains("appUsesCustomFrame=false"));
    let brightness_after = adb.shell(
        serial,
        &["dumpsys", "display_ext", "back_screen", "brightness"],
    )?;
    assert_eq!(
        brightness_before.lines().find(|l| l.starts_with("auto=")),
        brightness_after.lines().find(|l| l.starts_with("auto="))
    );
    let foreground_after = adb.shell(serial, &["dumpsys", "activity", "activities"])?;
    assert_eq!(active(&foreground_before), active(&foreground_after));
    std::fs::write(out.join("phone-restored.txt"), after)?;
    println!("RESTORE VERIFIED; PHONE FOREGROUND UNCHANGED");
    Ok(())
}

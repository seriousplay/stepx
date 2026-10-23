//! Explicitly scoped integration checks; synthetic game tests are labelled as such.
use anyhow::{ensure, Context, Result};
use std::{thread, time::Duration};
use stepx_companion::{
    adb::Adb,
    backscreen::{self, LiveData, PlaybackConfig, Preset, Source},
    companion_app as app,
    device::Device,
};
fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let serial = args
        .iter()
        .position(|x| x == "--device")
        .and_then(|i| args.get(i + 1))
        .context("需要显式 --device")?;
    ensure!(serial == "STEPX_DEVICE_SERIAL", "验证仅限已授权真机");
    let action = args
        .iter()
        .position(|x| x == "--action")
        .and_then(|i| args.get(i + 1))
        .context("需要 --action inspect|install|astronaut|image|standby|game|stop|synthetic")?;
    let adb = Adb::discover()?;
    let device = Device::inspect(&adb, serial)?;
    match action.as_str() {
        "inspect" => println!("{:?}", app::inspect(&adb, &device)?),
        "install" => println!("{:?}", app::install(&adb, &device, |s| println!("{s}"))?),
        "astronaut" => {
            let config = PlaybackConfig {
                brightness: 8,
                fps: 12,
                duration_secs: 4,
                invert: false,
            };
            let frames: Vec<_> = (0..48)
                .map(|i| {
                    backscreen::frame(
                        &Source::Preset(Preset::Astronaut),
                        &config,
                        Duration::from_millis(i * 4000 / 48),
                        &LiveData::default(),
                    )
                })
                .collect();
            println!(
                "{}",
                app::apply_animation(&adb, &device, &frames, 12, 4, "旋转太空人")?
            );
        }
        "image" => {
            let mut frame = vec![0u8; 391];
            for y in 7..16 {
                for x in 4..13 {
                    if (x + y) % 2 == 0 {
                        frame[y * 17 + x] = 255
                    }
                }
            }
            println!(
                "{}",
                app::apply_animation(&adb, &device, &[frame], 1, 4, "验证棋盘")?
            );
        }
        "standby" => println!("{}", app::arm_game(&adb, &device)?),
        "game" => println!("{}", app::start_game(&adb, &device)?),
        "stop" => println!("{}", app::stop(&adb, &device)?),
        "synthetic" => {
            println!(
                "合成输入通关测试：仅验证游戏状态、真机帧输出与恢复，不代表真实三敲/倾斜验证。"
            );
            adb.shell(
                serial,
                &[
                    "am",
                    "start-foreground-service",
                    "--user",
                    "0",
                    "-n",
                    "com.stepx.companion.app/.CompanionService",
                    "--es",
                    "op",
                    "test",
                    "--es",
                    "request_id",
                    "verification-synthetic",
                    "--ez",
                    "test_mode",
                    "true",
                ],
            )?;
            for _ in 0..20 {
                thread::sleep(Duration::from_secs(1));
                let s = adb.shell(
                    serial,
                    &[
                        "cat",
                        "/sdcard/Android/data/com.stepx.companion.app/files/status.json",
                    ],
                )?;
                let v: serde_json::Value = serde_json::from_str(&s)?;
                println!("{}", v);
                if v["lastTest"]["synthetic"] == true && v["running"] == false {
                    break;
                }
            }
        }
        _ => anyhow::bail!("未知验证操作"),
    }
    Ok(())
}

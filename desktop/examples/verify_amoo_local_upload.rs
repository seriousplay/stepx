//! Uploads one synthetic image through the native local ability event route.
//! Explicit device only; no chat, credentials, screen changes, or arbitrary ability.
use anyhow::{bail, Context, Result};
use serde_json::json;
use stepx_companion::{adb::Adb, amoo::AgentBus, device::Device};

fn main() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if !(args.len() == 2 || (args.len() == 3 && args[2] == "--observe"))
        || args[0] != "--device"
        || args[1] != "STEPX_DEVICE_SERIAL"
    {
        bail!("usage: verify_amoo_local_upload --device STEPX_DEVICE_SERIAL [--observe]");
    }
    let adb = Adb::discover()?;
    let device = Device::inspect(&adb, &args[1])?;
    let directory = format!("/sdcard/Download/Amoo Work/{:032x}", rand::random::<u128>());
    let remote = format!("{directory}/00-synthetic-colours.png");
    let temporary = tempfile::tempdir()?;
    let path = temporary.path().join("synthetic-colours.png");
    let mut bitmap = image::RgbaImage::new(300, 100);
    for (x, _, pixel) in bitmap.enumerate_pixels_mut() {
        *pixel = image::Rgba(if x < 100 {
            [238, 50, 50, 255]
        } else if x < 200 {
            [45, 190, 90, 255]
        } else {
            [55, 90, 235, 255]
        });
    }
    bitmap.save(&path)?;
    let mut bus = if args.len() == 3 {
        // The lifecycle verifier owns the leases; this diagnostic must not add,
        // renew, release, or disable any of them.
        let state: serde_json::Value = serde_json::from_str(&adb.shell(
            &device.transport,
            &[
                "cat",
                "/sdcard/Android/data/com.stepx.companion.app/files/amoo-connection.json",
            ],
        )?)?;
        anyhow::ensure!(
            state["active"] == true && state["screenInteractive"] == false,
            "observer diagnostic needs an existing active screen-off task lease"
        );
        AgentBus::observe(&adb, &device)?
    } else {
        AgentBus::connect(&adb, &device)?
    };
    adb.shell(&device.transport, &["mkdir", "-p", "--", &directory])?;
    let result = (|| -> Result<()> {
        adb.run(&[
            "-s",
            &device.transport,
            "push",
            path.to_str().context("test path")?,
            &remote,
        ])?;
        let digest = std::process::Command::new("sha256sum")
            .arg(&path)
            .output()?;
        anyhow::ensure!(digest.status.success(), "local checksum failed");
        let expected = String::from_utf8(digest.stdout)?
            .split_whitespace()
            .next()
            .context("local digest missing")?
            .to_owned();
        let hash = adb.shell(&device.transport, &["sha256sum", &remote])?;
        anyhow::ensure!(
            hash.split_whitespace().next() == Some(expected.as_str()),
            "staged image checksum mismatch"
        );
        let response = bus.upload_local_images(&[remote.clone()])?;
        // Print structure and booleans only; signed object URLs stay in memory.
        println!(
            "{}",
            json!({"native_ok":response["ok"],
            "payload_keys":response["payload"].as_object().map(|p|p.keys().collect::<Vec<_>>()),
            "ability_ok":response["payload"]["ok"]})
        );
        let rows = response["payload"]["content"]["uploaded"]
            .as_array()
            .context("native upload result missing uploaded array")?;
        anyhow::ensure!(rows.len() == 1, "unexpected upload result count");
        let row = &rows[0];
        let source = row["sourcePath"].as_str().unwrap_or("");
        anyhow::ensure!(
            response["ok"] == true
                && response["payload"]["ok"] == true
                && row["success"] == true
                && source.strip_prefix("file://").unwrap_or(source) == remote
                && row["url"].as_str().is_some_and(
                    |url| url.starts_with("https://") && !url.chars().any(char::is_control)
                ),
            "native upload did not return a verified HTTPS image"
        );
        println!("local_image_upload_verified=true staged_checksum_verified=true");
        Ok(())
    })();
    let cleanup = adb.shell(&device.transport, &["rm", "-rf", "--", &directory]);
    println!("synthetic_phone_file_cleaned={}", cleanup.is_ok());
    result
}

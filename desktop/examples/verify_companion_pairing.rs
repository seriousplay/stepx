//! Explicit-device integration driver. Never changes network trust or grants permissions.
use anyhow::{bail, Context, Result};
use std::{
    net::SocketAddr,
    path::Path,
    thread,
    time::{Duration, Instant},
};
use stepx_companion::{adb::Adb, device::Device, discovery, pairing::QrSession};

const DEVICE: &str = "STEPX_DEVICE_SERIAL";
const QR_PHONE: &str = "/sdcard/Android/data/com.stepx.companion.app/files/incoming-qr.png";
const TEST_STATUS: &str = "/sdcard/Android/data/com.stepx.companion.app/files/pairing-test.json";

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut selected = None;
    let mut camera = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--device" if i + 1 < args.len() => {
                selected = Some(args[i + 1].clone());
                i += 2;
            }
            "--qr-only" => {
                camera = true;
                i += 1;
            }
            _ => bail!("Usage: verify_companion_pairing --device {DEVICE} [--qr-only]"),
        }
    }
    if selected.as_deref() != Some(DEVICE) {
        bail!(
            "Explicit --device {DEVICE} is required; other devices are not authorized by this test"
        );
    }
    let adb = Adb::discover()?;
    let phone = Device::inspect(&adb, DEVICE)?;
    if phone.serial != DEVICE {
        bail!("USB device identity mismatch; no pairing was started");
    }
    phone.verify(&adb)?;
    let enabled = adb.shell(DEVICE, &["settings", "get", "global", "adb_wifi_enabled"])?;
    if enabled.trim() != "1" {
        bail!("Wireless debugging is off. Open StepX伴侣 → 连接 → 扫码连接 desktop, explicitly enable wireless connection and confirm the Android Wi-Fi trust dialog, then rerun. This driver does not grant network trust.");
    }
    let permission = adb.shell(DEVICE, &["dumpsys", "package", "com.stepx.companion.app"])?;
    for name in [
        "android.permission.MANAGE_DEBUGGING",
        "android.permission.WRITE_SECURE_SETTINGS",
    ] {
        if !permission
            .lines()
            .any(|line| line.contains(&format!("{name}: granted=true")))
        {
            bail!("Companion permission {name} is not granted; install / repair it from desktop first");
        }
    }
    let qr = QrSession::new()?;
    let request_id = qr.service.trim_start_matches("studio-stepx-");
    // Initialize discovery before opening Android's pairing listener.
    let _ = discovery::discover_services(&adb);
    let result = if camera {
        println!("QR image: {}", qr.path.display());
        println!("Scan this live QR using StepX伴侣 and confirm the computer. Waiting up to 115 seconds.");
        complete(&adb, &phone, &qr, None)
    } else {
        // Keep cleanup outside the closure so launch/readiness failures also delete the QR.
        (|| -> Result<()> {
            phone.verify(&adb)?;
            adb.transfer(DEVICE, "push", &qr.path, Path::new(QR_PHONE))?;
            let launched = adb.shell(
                DEVICE,
                &[
                    "am",
                    "start",
                    "-W",
                    "-f",
                    "0x20000000", // FLAG_ACTIVITY_SINGLE_TOP: onNewIntent handles an already-open page.
                    "-n",
                    "com.stepx.companion.app/.PairingActivity",
                    "--ez",
                    "test_mode",
                    "true",
                    "--es",
                    "test_qr_file",
                    "incoming-qr.png",
                    "--es",
                    "test_request_id",
                    request_id,
                ],
            )?;
            if launched.contains("Error") || launched.contains("Exception") {
                bail!("Could not launch companion pairing activity");
            }
            let acknowledgement = Instant::now() + Duration::from_secs(20);
            loop {
                if let Some(phase) = diagnostic_phase(&adb, &phone, request_id)? {
                    if matches!(phase.as_str(), "submitted" | "pairing" | "paired") {
                        break;
                    }
                }
                if Instant::now() >= acknowledgement {
                    bail!("The current companion did not submit this QR request. Install the latest companion, unlock the phone, and confirm its pairing service and wireless network are ready.");
                }
                thread::sleep(Duration::from_millis(250));
            }
            println!(
            "Started local image decoding in companion; waiting for authenticated Android pairing."
        );
            complete(&adb, &phone, &qr, Some(request_id))
        })()
    };
    // This fixed diagnostic file contains a short-lived credential; remove it after every attempt.
    if !camera && phone.verify(&adb).is_ok() {
        let _ = adb.shell(DEVICE, &["rm", "-f", QR_PHONE]);
    }
    result
}

fn diagnostic_phase(adb: &Adb, phone: &Device, request_id: &str) -> Result<Option<String>> {
    let Ok(raw) = adb.shell(&phone.transport, &["cat", TEST_STATUS]) else {
        return Ok(None);
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return Ok(None);
    };
    if value["requestId"].as_str() != Some(request_id) {
        return Ok(None);
    }
    let phase = value["phase"].as_str().unwrap_or("unknown");
    if matches!(phase, "error" | "cancelled") {
        let error: String = value["error"]
            .as_str()
            .unwrap_or("Pairing was cancelled or not accepted")
            .chars()
            .take(200)
            .collect();
        bail!("Companion QR request {phase}: {error}");
    }
    Ok(Some(phase.to_owned()))
}

fn complete(adb: &Adb, phone: &Device, qr: &QrSession, diagnostic_id: Option<&str>) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(115);
    let mut last_state = String::new();
    let pair_endpoint = loop {
        if Instant::now() >= deadline || qr.expired() {
            bail!("No completed pairing within QR validity window");
        }
        if let Some(request_id) = diagnostic_id {
            diagnostic_phase(adb, phone, request_id)?;
        }
        match qr.try_pair(adb) {
            Ok(Some(endpoint)) => break endpoint,
            Ok(None) => {}
            Err(error) => return Err(error).context("Authenticated QR pairing did not complete"),
        }
        if let Ok(raw) = adb.shell(
            &phone.transport,
            &[
                "cat",
                "/sdcard/Android/data/com.stepx.companion.app/files/pairing.json",
            ],
        ) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&raw) {
                let state = json["pairingState"].as_str().unwrap_or("unknown");
                if state != last_state {
                    println!("Companion pairing state: {state}");
                    last_state = state.to_owned();
                }
            }
        }
        thread::sleep(Duration::from_millis(700));
    };
    println!("Authenticated QR handshake succeeded; locating this phone's TLS connection port.");
    let pair_ip = pair_endpoint.parse::<SocketAddr>()?.ip();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if Instant::now() >= deadline {
            bail!("Pairing succeeded, but this phone's TLS connect service was not discovered");
        }
        for service in discovery::discover_services(adb)? {
            if service.kind != "_adb-tls-connect._tcp" {
                continue;
            }
            let address: SocketAddr = service.endpoint.parse()?;
            // Only contact the same phone address that completed our fresh one-time handshake.
            if address.ip() != pair_ip {
                continue;
            }
            adb.run(&["connect", &service.endpoint])?;
            let connected = Device::inspect(adb, &service.endpoint);
            match connected {
                Ok(device) if device.serial == phone.serial => {
                    device.verify(adb)?;
                    println!("PASS: companion QR decoding → Android pairing → encrypted ADB connection → hardware identity {}", device.serial);
                    return Ok(());
                }
                _ => {
                    let _ = adb.run(&["disconnect", &service.endpoint]);
                    bail!(
                        "TLS connection identity did not match the authorized phone; disconnected"
                    );
                }
            }
        }
        thread::sleep(Duration::from_millis(700));
    }
}

//! Real phone Amoo -> structured scene/image verification. No hardware writes.
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use stepx_companion::{
    adb::Adb,
    amoo::AgentBus,
    backscreen_amoo::{self, Event, Mode},
    device::Device,
};
fn cleanup(adb: &Adb, device: &Device, key: &str) -> Result<()> {
    let mut bus = AgentBus::connect(adb, device)?;
    if let Ok(history) = bus.call("chat.history", json!({"sessionId":key,"limit":100})) {
        for row in history["messages"].as_array().into_iter().flatten() {
            for tool in row["toolCalls"].as_array().into_iter().flatten() {
                if tool["name"] != "task_create" {
                    continue;
                }
                let raw = tool["result"].get("result").unwrap_or(&tool["result"]);
                let value = raw
                    .as_str()
                    .and_then(|s| serde_json::from_str::<Value>(s).ok())
                    .unwrap_or_else(|| raw.clone());
                if let Some(task) = value["taskId"].as_str() {
                    let _ = bus.call(
                        "stepclaw.tasks.delete",
                        json!({"taskId":task,"reason":"backscreen_verification_cleanup"}),
                    );
                }
                if let Some(child) = value["childSessionKey"]
                    .as_str()
                    .filter(|s| s.starts_with("agent:task-bot:subagent:"))
                {
                    let _ = bus.call("sessions.delete", json!({"key":child}));
                }
            }
        }
    }
    bus.call("sessions.delete", json!({"key":key}))?;
    Ok(())
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let serial = args.get(1).context("explicit phone serial required")?;
    if serial != "STEPX_DEVICE_SERIAL" {
        bail!("only explicit StepX test phone allowed")
    }
    let mode = if args.iter().any(|a| a == "--image") {
        Mode::Image
    } else {
        Mode::Auto
    };
    let prompt = args.get(2).context("prompt required")?.clone();
    let adb = Adb::discover()?;
    let device = Device::inspect(&adb, serial)?;
    let foreground = || -> Result<String> {
        Ok(adb
            .shell(&device.transport, &["dumpsys", "window"])?
            .lines()
            .filter(|l| l.contains("mCurrentFocus="))
            .collect::<Vec<_>>()
            .join("\n"))
    };
    let before = foreground()?;
    let job = backscreen_amoo::start(adb.clone(), device.clone(), prompt, mode);
    let deadline = Instant::now() + Duration::from_secs(420);
    loop {
        if Instant::now() >= deadline {
            job.cancel();
            bail!("test deadline exceeded")
        }
        match job.receiver.recv_timeout(Duration::from_secs(20)) {
            Ok(Event::Status(status)) => println!("status={status}"),
            Ok(Event::Complete(generated)) => {
                println!(
                    "result={}",
                    serde_json::to_string(
                        &json!({"summary":generated.summary,"preset":generated.preset,"native_feature":generated.native_feature,"app_package":generated.app_package,"brightness":generated.brightness,"fps":generated.fps,"duration_secs":generated.duration_secs,"image_path":generated.image_path})
                    )?
                );
                if let Some(path) = generated.image_path {
                    let image = image::ImageReader::open(&path)?
                        .with_guessed_format()?
                        .decode()?;
                    println!("image_decoded={}x{}", image.width(), image.height());
                }
                cleanup(&adb, &device, &generated.session_key)?;
                let unchanged = foreground()? == before;
                println!("phone_foreground_unchanged={unchanged} owned_sessions_cleaned=true");
                if !unchanged {
                    bail!("phone foreground changed")
                }
                return Ok(());
            }
            Ok(Event::Failed(error)) => bail!("{error}"),
            Ok(Event::Cancelled) => bail!("cancelled"),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => println!("waiting=true"),
            Err(error) => bail!("{error}"),
        }
    }
}

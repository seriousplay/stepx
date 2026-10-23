//! Explicit real-device vision verification using production attachment transport.
//! Answers exist only in random PNG pixels, never in prompts or attachment names.
use anyhow::{bail, Context, Result};
use image::{Rgb, RgbImage};
use rand::{seq::SliceRandom, Rng};
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
use stepx_companion::work::{self, WorkEvent, WorkRequest};
use stepx_companion::{adb::Adb, amoo::AgentBus, amoo::PendingReply, amoo_media, device::Device};

const TARGET: &str = "STEPX_DEVICE_SERIAL";
const PROMPT: &str = "Analyze the attached image directly. Read the six-digit number at the top and identify the three shapes below, from left to right. Return only JSON with this schema: {\"code\":\"six digits\",\"left_to_right\":[{\"shape\":\"circle|square|triangle\",\"color\":\"red|green|blue|orange|purple|black\"}]}. Include exactly three entries. Do not guess missing visual information; do not open any phone screen.";

#[derive(Clone)]
struct Challenge {
    path: PathBuf,
    answer: Value,
}

fn challenge(folder: &Path) -> Result<Challenge> {
    let mut rng = rand::thread_rng();
    let code = format!("{:06}", rng.gen_range(100_000..1_000_000));
    let path = folder.join(format!("attachment-{:032x}.png", rng.gen::<u128>()));
    let mut colors = [
        ("red", [225, 32, 42]),
        ("green", [25, 150, 65]),
        ("blue", [25, 70, 220]),
        ("orange", [255, 145, 0]),
        ("purple", [145, 40, 190]),
        ("black", [25, 25, 25]),
    ];
    colors.shuffle(&mut rng);
    let mut shapes = ["circle", "square", "triangle"];
    shapes.shuffle(&mut rng);
    let mut bitmap = RgbImage::from_pixel(900, 540, Rgb([255, 255, 255]));
    // Pixel font keeps the fixture independent of local fonts, OCR and metadata.
    const DIGITS: [[&str; 7]; 10] = [
        [
            "01110", "10001", "10001", "10001", "10001", "10001", "01110",
        ],
        [
            "00100", "01100", "00100", "00100", "00100", "00100", "01110",
        ],
        [
            "01110", "10001", "00001", "00010", "00100", "01000", "11111",
        ],
        [
            "11110", "00001", "00001", "01110", "00001", "00001", "11110",
        ],
        [
            "00010", "00110", "01010", "10010", "11111", "00010", "00010",
        ],
        [
            "11111", "10000", "10000", "11110", "00001", "00001", "11110",
        ],
        [
            "01110", "10000", "10000", "11110", "10001", "10001", "01110",
        ],
        [
            "11111", "00001", "00010", "00100", "01000", "01000", "01000",
        ],
        [
            "01110", "10001", "10001", "01110", "10001", "10001", "01110",
        ],
        [
            "01110", "10001", "10001", "01111", "00001", "00001", "01110",
        ],
    ];
    for (i, digit) in code.bytes().enumerate() {
        for (row, cells) in DIGITS[(digit - b'0') as usize].iter().enumerate() {
            for (column, cell) in cells.bytes().enumerate() {
                if cell == b'1' {
                    for dy in 0..14 {
                        for dx in 0..14 {
                            bitmap.put_pixel(
                                170 + i as u32 * 96 + column as u32 * 14 + dx,
                                45 + row as u32 * 14 + dy,
                                Rgb([20, 20, 20]),
                            );
                        }
                    }
                }
            }
        }
    }
    let mut objects = Vec::new();
    for (index, shape) in shapes.iter().enumerate() {
        let center_x = 160 + index as i32 * 290;
        let center_y = 340;
        for y in 235i32..446 {
            for x in center_x - 100..=center_x + 100 {
                let dx = x - center_x;
                let dy = y - center_y;
                let inside = match *shape {
                    "circle" => dx * dx + dy * dy <= 90 * 90,
                    "square" => dx.abs() <= 85 && dy.abs() <= 85,
                    "triangle" => (-100..=90).contains(&dy) && dx.abs() * 2 <= dy + 100,
                    _ => unreachable!(),
                };
                if inside {
                    bitmap.put_pixel(x as u32, y as u32, Rgb(colors[index].1));
                }
            }
        }
        objects.push(json!({"shape":shape,"color":colors[index].0}));
    }
    bitmap.save(&path)?;
    Ok(Challenge {
        path,
        answer: json!({"code":code,"left_to_right":objects}),
    })
}

fn extract_json(text: &str) -> Result<Value> {
    for (start, character) in text.char_indices() {
        if character == '{' {
            if let Some(Ok(value)) = serde_json::Deserializer::from_str(&text[start..])
                .into_iter::<Value>()
                .next()
            {
                if value.get("code").is_some() || value.get("images").is_some() {
                    return Ok(value);
                }
            }
        }
    }
    bail!("vision reply contains no structured answer")
}

fn await_reply(adb: &Adb, device: &Device, receipt: &PendingReply) -> Result<(Value, u64)> {
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(210) {
        std::thread::sleep(Duration::from_secs(3));
        let mut bus = AgentBus::connect(adb, device)?;
        let snapshot = bus.snapshot(Some(&receipt.key))?;
        let session = snapshot
            .sessions
            .iter()
            .find(|s| s.key == receipt.key)
            .context("disposable vision session missing")?;
        if !receipt.finished(session) {
            continue;
        }
        let messages: Vec<_> = session
            .messages
            .iter()
            .filter(|m| m.run_id == receipt.run_id && m.role == "assistant")
            .collect();
        if messages.iter().any(|m| {
            matches!(
                m.stop_reason.to_ascii_lowercase().as_str(),
                "error" | "aborted" | "cancelled" | "tracking_limit"
            )
        }) {
            bail!("vision task ended with an error")
        }
        let text = messages
            .iter()
            .map(|m| m.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        return Ok((extract_json(&text)?, started.elapsed().as_secs()));
    }
    bail!("vision task did not finish within 210 seconds")
}

fn assert_answer(actual: &Value, expected: &Value, stage: &str) -> Result<()> {
    if actual != expected {
        bail!("{stage} visual mismatch: expected {expected}, received {actual}")
    }
    Ok(())
}

fn desktop_report(adb: &Adb, device: &Device, key: &str, sample: &Challenge) -> Result<Value> {
    let workspace = sample.path.parent().context("fixture workspace missing")?;
    let started = Instant::now();
    let handle = work::start(adb.clone(), device.clone(), WorkRequest {
        prompt: "Analyze the attached image directly and create an actual Markdown report named report.md using the desktop write_file tool. The image has a six-digit number above three colored shapes. The report must include a heading, the number, a table of all three objects in left-to-right order, and a JSON code block with this schema: {\"code\":\"six digits\",\"left_to_right\":[{\"shape\":\"circle|square|triangle\",\"color\":\"red|green|blue|orange|purple|black\"}]}. Include exactly three objects. After saving, use desktop read_file to verify the generated report, then finish. Do not decode the image with read_file or exec, do not open the phone screen, and do not invent missing visual information. All desktop actions must use the amoo_work JSON tool protocol.".into(),
        workspace: workspace.to_path_buf(), session_key: Some(key.into()),
        model: None, attachments: vec![sample.path.clone()],
    });
    let mut terminal = false;
    let mut write_verified = false;
    let mut readback_verified = false;
    let result = (|| -> Result<Value> {
        while started.elapsed() < Duration::from_secs(360) {
            match handle.events.recv_timeout(Duration::from_secs(3)) {
                Ok(WorkEvent::Tool(log)) => {
                    write_verified |= log.tool == "write_file" && log.success;
                    readback_verified |= write_verified
                        && log.tool == "read_file"
                        && log.success
                        && log.detail.contains("report.md");
                    println!("desktop_tool={} success={}", log.tool, log.success);
                }
                Ok(WorkEvent::Complete(_)) => {
                    terminal = true;
                    break;
                }
                Ok(WorkEvent::Failed(error)) => {
                    terminal = true;
                    bail!("desktop vision: {error}");
                }
                Ok(WorkEvent::Cancelled) => {
                    terminal = true;
                    bail!("desktop vision cancelled");
                }
                Ok(_) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(_) => bail!("desktop vision worker disconnected"),
            }
        }
        if !terminal {
            bail!("desktop vision did not finish within 360 seconds")
        }
        if !write_verified || !readback_verified {
            bail!("desktop report was not actually written and read back")
        }
        let artifacts = work::load_artifacts(workspace)?;
        let artifact = artifacts
            .iter()
            .find(|a| a.path.file_name().is_some_and(|n| n == "report.md"))
            .context("desktop task produced no actual report.md artifact")?;
        if !artifact
            .path
            .canonicalize()?
            .starts_with(workspace.canonicalize()?)
        {
            bail!("desktop report escaped verification workspace")
        }
        let bytes = std::fs::read(&artifact.path)?;
        if bytes.is_empty() || bytes.len() as u64 != artifact.bytes {
            bail!("desktop artifact bytes not verified")
        }
        let text = std::str::from_utf8(&bytes)?;
        let answer = extract_json(text)?;
        assert_answer(&answer, &sample.answer, "desktop report")?;
        if !text.contains('#') || !text.contains('|') {
            bail!("desktop report missing requested Markdown formatting")
        }
        Ok(
            json!({"stage":"desktop_report","answer":answer,"seconds":started.elapsed().as_secs(),
            "artifact":artifact.path,"bytes":bytes.len(),"write_verified":write_verified,"readback_verified":readback_verified}),
        )
    })();
    if !terminal {
        handle.cancel.store(true, Ordering::SeqCst);
        let stop_deadline = Instant::now() + Duration::from_secs(45);
        while Instant::now() < stop_deadline {
            if matches!(
                handle.events.recv_timeout(Duration::from_secs(2)),
                Ok(WorkEvent::Cancelled | WorkEvent::Complete(_) | WorkEvent::Failed(_))
            ) {
                break;
            }
        }
    }
    result
}

fn cleanup_session(adb: &Adb, device: &Device, key: &str, names: &HashSet<String>) -> Result<()> {
    let mut bus = AgentBus::connect(adb, device)?;
    let _ = bus.call("chat.abort", json!({"sessionId":key}));
    if let Ok(history) = bus.call("chat.history", json!({"sessionId":key,"limit":100})) {
        for message in history["messages"].as_array().into_iter().flatten() {
            if message["role"] == "user" {
                if let Some(text) = message["text"].as_str() {
                    if let Some((_, rest)) = text.split_once("<extra>") {
                        if let Some((raw, _)) = rest.split_once("</extra>") {
                            if let Ok(extra) = serde_json::from_str::<Value>(raw) {
                                for attachment in
                                    extra["attachments"].as_array().into_iter().flatten()
                                {
                                    let name = attachment["filename"].as_str().unwrap_or("");
                                    let path = attachment["devicePath"].as_str().unwrap_or("");
                                    if !names.contains(name) {
                                        continue;
                                    }
                                    if let Some(rest) =
                                        path.strip_prefix("/sdcard/Download/Amoo Work/")
                                    {
                                        if let Some((folder, basename)) = rest.split_once('/') {
                                            if folder.len() == 32
                                                && folder.bytes().all(|b| b.is_ascii_hexdigit())
                                                && !basename.contains('/')
                                                && basename.ends_with(name)
                                            {
                                                adb.shell(
                                                    &device.transport,
                                                    &["rm", "-f", "--", path],
                                                )?;
                                                let parent =
                                                    format!("/sdcard/Download/Amoo Work/{folder}");
                                                let _ = adb.shell(
                                                    &device.transport,
                                                    &["rmdir", "--", &parent],
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            for tool in message["toolCalls"].as_array().into_iter().flatten() {
                if tool["name"] != "task_create" {
                    continue;
                }
                let result = tool["result"]["result"]
                    .as_str()
                    .and_then(|s| serde_json::from_str::<Value>(s).ok())
                    .unwrap_or_default();
                if let Some(id) = result["taskId"].as_str() {
                    let _ = bus.call(
                        "stepclaw.tasks.cancel",
                        json!({"taskId":id,"reason":"vision_verification_cleanup"}),
                    );
                    bus.call(
                        "stepclaw.tasks.delete",
                        json!({"taskId":id,"reason":"vision_verification_cleanup"}),
                    )?;
                }
                if let Some(child) = result["childSessionKey"]
                    .as_str()
                    .filter(|s| s.starts_with("agent:task-bot:subagent:"))
                {
                    bus.call("sessions.delete", json!({"key":child}))?;
                }
            }
        }
    }
    bus.call("sessions.delete", json!({"key":key}))?;
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let fixtures_only = args.iter().any(|s| s == "--fixture-only");
    let desktop_only = args.iter().any(|s| s == "--desktop-only");
    let include_desktop = desktop_only || args.iter().any(|s| s == "--desktop");
    if !fixtures_only && args.first().map(String::as_str) != Some(TARGET) {
        bail!("pass explicit authorized target {TARGET}, or --fixture-only")
    }
    let folder = std::env::temp_dir().join(format!(
        "stepx-vision-verification-{:016x}",
        rand::random::<u64>()
    ));
    std::fs::create_dir(&folder)?;
    let challenges = [
        challenge(&folder)?,
        challenge(&folder)?,
        challenge(&folder)?,
    ];
    // Expected answers remain OUTSIDE the desktop task's accessible workspace.
    let desktop_sample = if include_desktop {
        let workspace = folder.join("desktop-workspace");
        std::fs::create_dir(&workspace)?;
        Some(challenge(&workspace)?)
    } else {
        None
    };
    let names: HashSet<_> = challenges
        .iter()
        .chain(desktop_sample.iter())
        .map(|c| c.path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    std::fs::write(
        folder.join("expected.json"),
        serde_json::to_vec_pretty(&json!(challenges
            .iter()
            .chain(desktop_sample.iter())
            .map(|c| json!({"path":c.path,"expected":c.answer}))
            .collect::<Vec<_>>()))?,
    )?;
    println!("synthetic_evidence={}", folder.display());
    if fixtures_only {
        return Ok(());
    }
    let adb = Adb::discover()?;
    let device = Device::inspect(&adb, TARGET)?;
    let focus = || -> Result<String> {
        Ok(adb
            .shell(&device.transport, &["dumpsys", "window"])?
            .lines()
            .filter(|l| l.contains("mCurrentFocus="))
            .collect::<Vec<_>>()
            .join("\n"))
    };
    let foreground = focus()?;
    let mut sessions = Vec::new();
    let mut results = Vec::new();
    let run = (|| -> Result<()> {
        let mut bus = AgentBus::connect(&adb, &device)?;
        if !desktop_only {
            let key = bus.create_session()?;
            sessions.push(key.clone());
            println!("single_image_started=true");
            let receipt =
                amoo_media::send_files(&adb, &device, &key, PROMPT, &[challenges[0].path.clone()])?;
            let (answer, seconds) = await_reply(&adb, &device, &receipt)?;
            results.push(json!({"stage":"single","answer":answer,"seconds":seconds}));
            assert_answer(&answer, &challenges[0].answer, "single image")?;
            println!("single_image_verified=true seconds={seconds}");

            println!("followup_started=true");
            let receipt = bus.send(&key, "Refer only to the image I attached in my previous message. Return its six-digit code and the shape/color of its MIDDLE object as JSON: {\"code\":\"six digits\",\"middle\":{\"shape\":\"...\",\"color\":\"...\"}}. No new attachment is supplied. Do not open the phone screen.")?;
            let (answer, seconds) = await_reply(&adb, &device, &receipt)?;
            let expected = json!({"code":challenges[0].answer["code"],"middle":challenges[0].answer["left_to_right"][1]});
            results.push(json!({"stage":"followup","answer":answer,"seconds":seconds}));
            assert_answer(&answer, &expected, "follow-up")?;
            println!("followup_verified=true seconds={seconds}");

            let key = bus.create_session()?;
            sessions.push(key.clone());
            println!("two_images_started=true");
            let receipt = amoo_media::send_files(&adb, &device, &key,
            "Analyze BOTH attached images directly, in attachment order. Each contains a six-digit number at the top and three colored shapes below. Return only JSON: {\"images\":[{\"code\":\"six digits\",\"left_to_right\":[{\"shape\":\"circle|square|triangle\",\"color\":\"red|green|blue|orange|purple|black\"}]}]}. Include two image entries, each with exactly three objects. Do not guess missing visual information or open a phone screen.",
            &[challenges[1].path.clone(), challenges[2].path.clone()])?;
            let (answer, seconds) = await_reply(&adb, &device, &receipt)?;
            let expected = json!({"images":[challenges[1].answer,challenges[2].answer]});
            results.push(json!({"stage":"two_images","answer":answer,"seconds":seconds}));
            assert_answer(&answer, &expected, "two images")?;
            println!("two_images_verified=true seconds={seconds}");
        }
        if let Some(sample) = &desktop_sample {
            let key = bus.create_session()?;
            sessions.push(key.clone());
            println!("desktop_report_started=true");
            let result = desktop_report(&adb, &device, &key, sample)?;
            println!(
                "desktop_report_verified=true artifact={}",
                result["artifact"].as_str().unwrap_or("")
            );
            results.push(result);
        }
        Ok(())
    })();
    let mut cleanup_ok = true;
    for key in &sessions {
        if cleanup_session(&adb, &device, key, &names).is_err() {
            cleanup_ok = false;
        }
    }
    let unchanged = focus()? == foreground;
    println!("owned_session_cleanup={cleanup_ok}");
    println!("phone_foreground_unchanged={unchanged}");
    std::fs::write(
        folder.join("results.json"),
        serde_json::to_vec_pretty(&json!({
            "passed":run.is_ok(),"results":results,"cleanup":cleanup_ok,
            "phone_foreground_unchanged":unchanged,"error":run.as_ref().err().map(|e| format!("{e:#}"))
        }))?,
    )?;
    run?;
    if !cleanup_ok || !unchanged {
        bail!("vision verification cleanup/foreground check failed")
    }
    println!("production_vision_verified=true");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixtures_hold_answers_only_in_pixels() {
        let folder = tempfile::tempdir().unwrap();
        let sample = challenge(folder.path()).unwrap();
        assert_eq!(image::open(&sample.path).unwrap().width(), 900);
        assert_eq!(sample.answer["left_to_right"].as_array().unwrap().len(), 3);
        assert!(!PROMPT.contains(sample.answer["code"].as_str().unwrap()));
        assert!(!sample
            .path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .contains(sample.answer["code"].as_str().unwrap()));
    }
    #[test]
    fn structured_reply_can_be_fenced_without_accepting_plain_prose() {
        assert_eq!(
            extract_json("```json\n{\"code\":\"123456\"}\n```").unwrap()["code"],
            "123456"
        );
        assert!(extract_json("I cannot see the image").is_err());
    }
}

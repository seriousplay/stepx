//! Explicit live media verification. Uses only synthetic files and newly created sessions.
use anyhow::{bail, Context, Result};
use serde_json::json;
use std::{
    path::Path,
    time::{Duration, Instant},
};
use stepx_companion::{adb::Adb, amoo::AgentBus, amoo_media, device::Device};
fn main() -> Result<()> {
    let serial = std::env::args()
        .nth(1)
        .context("explicit device required")?;
    let image = std::env::args().any(|a| a == "--image");
    let photo = std::env::args().any(|a| a == "--photo");
    let adb = Adb::discover()?;
    let device = Device::inspect(&adb, &serial)?;
    let focus = || -> Result<String> {
        Ok(adb
            .shell(&device.transport, &["dumpsys", "window"])?
            .lines()
            .filter(|l| l.contains("mCurrentFocus="))
            .map(|line| {
                line.split(" u0 ")
                    .nth(1)
                    .unwrap_or(line)
                    .trim_end_matches('}')
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n"))
    };
    let before = focus()?;
    let mut bus = AgentBus::connect(&adb, &device)?;
    let key = bus.create_session()?;
    let mut upload_directory = String::new();
    let result = (|| -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let paths = if image {
            vec![]
        } else if photo {
            let path = temporary.path().join("synthetic-colours.png");
            let mut bitmap = image::RgbaImage::new(300, 100);
            for (x, _, p) in bitmap.enumerate_pixels_mut() {
                *p = image::Rgba(if x < 100 {
                    [238, 50, 50, 255]
                } else if x < 200 {
                    [45, 190, 90, 255]
                } else {
                    [55, 90, 235, 255]
                });
            }
            bitmap.save(&path)?;
            vec![path]
        } else {
            let path = temporary.path().join("amoo-media-check.csv");
            std::fs::write(
                &path,
                b"verification_code,sales\nAMOO_MEDIA_749826,17\nAMOO_MEDIA_749826,26\n",
            )?;
            vec![path]
        };
        let prompt = if image {
            amoo_media::image_request("生成一张简洁的蓝色纸船漂浮在平静湖面的插画，正方形。此为桌面功能验证，不要打开任何手机界面。")
        } else if photo {
            "请通过后台图片理解能力读取所附图片，告诉我从左到右的三个色块分别是什么颜色。不要猜测，禁止打开手机界面。".into()
        } else {
            "请读取附件中的 CSV，计算 sales 列的合计，并回答文件内 verification_code 的原文。不要猜测，也不要执行其他任务。".into()
        };
        let uploaded = amoo_media::upload_attachments(&adb, &device, &paths)?;
        upload_directory = uploaded.directory.clone();
        println!(
            "uploaded_count={} checksum_verified={}",
            uploaded.items.len(),
            uploaded.items.iter().all(|i| i.sha256.len() == 64)
        );
        let receipt = bus.send_with_attachments(
            &key,
            &uploaded.message(&prompt),
            &uploaded.wire_attachments()?,
        )?;
        println!("send_accepted=true");
        let deadline = Instant::now() + Duration::from_secs(240);
        loop {
            std::thread::sleep(Duration::from_secs(3));
            let mut b = AgentBus::connect(&adb, &device)?;
            let data = b.snapshot(Some(&key))?;
            let session = data
                .sessions
                .iter()
                .find(|s| s.key == key)
                .context("test session missing")?;
            if receipt.finished(session) {
                let usage = stepx_companion::amoo_usage::aggregate(
                    session.messages.iter().map(|m| &m.usage),
                );
                println!(
                    "usage_input={:?} usage_output={:?} usage_total={:?}",
                    usage.input_tokens, usage.output_tokens, usage.total_tokens
                );
                let raw = b.call("chat.history", json!({"sessionId":key,"limit":100}))?;
                std::fs::write(
                    if image {
                        "/tmp/stepx-media-final-image-history.json"
                    } else if photo {
                        "/tmp/stepx-media-final-photo-history.json"
                    } else {
                        "/tmp/stepx-media-final-upload-history.json"
                    },
                    serde_json::to_vec_pretty(&raw)?,
                )?;
                let text = session
                    .messages
                    .iter()
                    .filter(|m| m.role == "assistant")
                    .map(|m| m.text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                if image {
                    let a = session
                        .messages
                        .iter()
                        .flat_map(|m| m.attachments.iter())
                        .find(|a| a.kind == "image" || a.mime.starts_with("image/"))
                        .context("image task ended without image artifact")?;
                    let path = Path::new("/tmp/stepx-amoo-generated-boat-final.png");
                    if path.exists() {
                        std::fs::remove_file(path)?
                    }
                    let saved = amoo_media::download_artifact(&adb, &device, a, path)?;
                    let png = image::open(&saved.path)?;
                    println!(
                        "generated_image_verified=true bytes={} dimensions={}x{}",
                        saved.bytes,
                        png.width(),
                        png.height()
                    );
                    // Optional bounded hand-off to the native UI capture verifier.
                    // The caller creates this marker only after checking the preview;
                    // the disposable session is always cleaned up below.
                    if let Some(release) = std::env::args()
                        .position(|a| a == "--preview-release")
                        .and_then(|i| std::env::args().nth(i + 1))
                    {
                        let release = std::path::PathBuf::from(release);
                        anyhow::ensure!(!release.exists(), "preview marker already exists");
                        println!("preview_ready=true");
                        let deadline = Instant::now() + Duration::from_secs(120);
                        while !release.exists() && Instant::now() < deadline {
                            std::thread::sleep(Duration::from_millis(200));
                        }
                        anyhow::ensure!(release.exists(), "native preview verification timeout");
                    }
                } else if photo {
                    if !text.contains('红') || !text.contains('绿') || !text.contains('蓝') {
                        bail!("image attachment visual content not verified")
                    }
                    println!("photo_attachment_verified=true");
                } else {
                    if !text.contains("AMOO_MEDIA_749826") || !text.contains("43") {
                        bail!("uploaded file content not verified")
                    }
                    println!("upload_content_verified=true");
                }
                break;
            }
            if Instant::now() > deadline {
                bail!("media task timeout")
            }
        }
        Ok(())
    })();
    if result.is_err() {
        let stopped = bus.call("chat.abort", json!({"sessionId":key}));
        println!("temporary_run_stopped={}", stopped.is_ok());
    }
    if !upload_directory.is_empty() {
        let _ = adb.shell(&device.transport, &["rm", "-rf", "--", &upload_directory]);
    }
    // Only follow IDs explicitly created by this disposable parent session.
    if let Ok(history) = bus.call("chat.history", json!({"sessionId":key,"limit":100})) {
        std::fs::write(
            if image {
                "/tmp/stepx-media-final-image-history.json"
            } else if photo {
                "/tmp/stepx-media-final-photo-history.json"
            } else {
                "/tmp/stepx-media-final-upload-history.json"
            },
            serde_json::to_vec_pretty(&history)?,
        )?;
        for message in history["messages"].as_array().into_iter().flatten() {
            for tool in message["toolCalls"].as_array().into_iter().flatten() {
                if tool["name"] != "task_create" {
                    continue;
                }
                let value = tool["result"]["result"]
                    .as_str()
                    .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
                    .unwrap_or_default();
                if let Some(task) = value["taskId"].as_str() {
                    let _ = bus.call(
                        "stepclaw.tasks.cancel",
                        json!({"taskId":task,"reason":"desktop_media_verification_cleanup"}),
                    );
                    let deleted = bus.call(
                        "stepclaw.tasks.delete",
                        json!({"taskId":task,"reason":"desktop_media_verification_cleanup"}),
                    );
                    println!("temporary_task_cleanup={}", deleted.is_ok());
                }
                if let Some(child) = value["childSessionKey"]
                    .as_str()
                    .filter(|s| s.starts_with("agent:task-bot:subagent:"))
                {
                    let deleted = bus.call("sessions.delete", json!({"key":child}));
                    println!("temporary_child_cleanup={}", deleted.is_ok());
                }
            }
        }
    }
    let cleanup = bus.call("sessions.delete", json!({"key":key}));
    println!("temporary_session_cleanup={}", cleanup.is_ok());
    cleanup?;
    let after = focus()?;
    let unchanged = after == before;
    if !unchanged {
        println!("focus_before={before} focus_after={after}");
    }
    println!("phone_foreground_unchanged={unchanged}");
    if !unchanged {
        bail!("phone foreground changed")
    }
    result
}

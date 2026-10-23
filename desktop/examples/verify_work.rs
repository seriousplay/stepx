//! --send is explicit: only a new throwaway phone session and synthetic CSV are used.
use anyhow::{bail, Context, Result};
use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
use stepx_companion::{
    adb::Adb,
    amoo::AgentBus,
    device::Device,
    work::{self, WorkEvent, WorkRequest},
};
fn main() -> Result<()> {
    let serial = std::env::args()
        .nth(1)
        .context("usage: verify_work SERIAL [--send]")?;
    let temp = tempfile::tempdir()?;
    let a = 11 + rand::random::<u32>() % 80;
    let b = 11 + rand::random::<u32>() % 80;
    let expected = (a + b).to_string();
    std::fs::write(
        temp.path().join("sales.csv"),
        format!("item,total\nfictional-a,{a}\nfictional-b,{b}\n"),
    )?;
    let files = work::list_workspace(temp.path())?;
    anyhow::ensure!(files.len() == 1, "workspace list mismatch");
    if !std::env::args().any(|arg| arg == "--send") {
        println!("local_workspace_verified=true; pass --send for a disposable live task");
        return Ok(());
    }
    let adb = Adb::discover()?;
    let device = Device::inspect(&adb, &serial)?;
    let focus = |adb: &Adb, device: &Device| -> Result<String> {
        Ok(adb
            .shell(&device.transport, &["dumpsys", "window"])?
            .lines()
            .filter(|line| line.contains("mCurrentFocus=") || line.contains("mFocusedApp="))
            .collect::<Vec<_>>()
            .join("\n"))
    };
    let before = focus(&adb, &device)?;
    let mut bus = AgentBus::connect(&adb, &device)?;
    let key = bus.create_session()?;
    let task=work::start(adb.clone(),device.clone(),WorkRequest{prompt:"这是隔离的桌面功能验证。只使用桌面工具 csv_summary 读取 sales.csv，随后使用 write_file 生成 report.md，写明读取到的 total 列合计与计算依据。不要调用手机工具或外部网站，不要创建其他内容。必须实际保存文件后完成。".into(),workspace:temp.path().to_path_buf(),session_key:Some(key.clone()),model:None,attachments:vec![temp.path().join("sales.csv")]});
    let mut terminal = false;
    let mut usage_events = 0;
    let result = (|| -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(240);
        while Instant::now() < deadline {
            match task.events.recv_timeout(Duration::from_secs(5)) {
                Ok(WorkEvent::ModelResponse {
                    data, elapsed_ms, ..
                }) => {
                    let usage = stepx_companion::amoo_usage::aggregate(
                        data.sessions
                            .iter()
                            .flat_map(|s| s.messages.iter().map(|m| &m.usage)),
                    );
                    anyhow::ensure!(
                        elapsed_ms > 0 && usage.output_tokens.is_some(),
                        "missing real response metrics"
                    );
                    usage_events += 1;
                    println!("model_response_usage_verified=true output_tokens={} elapsed_ms={elapsed_ms}",usage.output_tokens.unwrap());
                }
                Ok(WorkEvent::Tool(log)) => println!("tool={} success={}", log.tool, log.success),
                Ok(WorkEvent::Artifact(artifact)) => {
                    println!("artifact={} bytes={}", artifact.kind, artifact.bytes)
                }
                Ok(WorkEvent::Complete(_)) => {
                    terminal = true;
                    break;
                }
                Ok(WorkEvent::Failed(error)) => {
                    terminal = true;
                    bail!("{error}");
                }
                Ok(WorkEvent::Cancelled) => {
                    terminal = true;
                    bail!("test task cancelled");
                }
                Ok(_) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(_) => bail!("worker disconnected"),
            }
        }
        anyhow::ensure!(terminal, "live task timeout");
        let artifacts = work::load_artifacts(temp.path())?;
        anyhow::ensure!(
            !artifacts.is_empty(),
            "model replied but did not produce a desktop file"
        );
        anyhow::ensure!(
            artifacts
                .iter()
                .any(|artifact| std::fs::read_to_string(&artifact.path)
                    .map(|text| text.contains(&expected))
                    .unwrap_or(false)),
            "expected synthetic CSV total missing"
        );
        anyhow::ensure!(usage_events > 0, "harness did not emit actual model usage");
        println!("live_desktop_deliverable_verified=true journal_recovered=true usage_events={usage_events}");
        Ok(())
    })();
    if !terminal {
        task.cancel.store(true, Ordering::SeqCst);
        let stop = Instant::now() + Duration::from_secs(45);
        while Instant::now() < stop {
            if matches!(
                task.events.recv_timeout(Duration::from_secs(2)),
                Ok(WorkEvent::Cancelled | WorkEvent::Failed(_) | WorkEvent::Complete(_))
            ) {
                break;
            }
        }
    }
    let cleanup = bus.call("sessions.delete", serde_json::json!({"key":key}));
    println!("temporary_session_cleanup={}", cleanup.is_ok());
    cleanup.context("Could not remove own test session")?;
    anyhow::ensure!(
        before == focus(&adb, &device)?,
        "Phone foreground changed during verification"
    );
    println!("phone_foreground_unchanged=true");
    result
}

//! Explicit live Work acceptance: synthetic workspace, real phone model and harness.
use anyhow::{ensure, Context, Result};
use serde_json::json;
use std::{
    path::PathBuf,
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
    let args: Vec<_> = std::env::args().collect();
    ensure!(
        args.iter().any(|a| a == "--send"),
        "--send required for actual model tasks"
    );
    let transport = args.get(1).context("ADB transport required")?;
    let root =
        PathBuf::from(args.get(2).context("synthetic workspace required")?).canonicalize()?;
    ensure!(
        root.starts_with("/path/to/stepx/diagnostics/work-experience-20261022/scenarios"),
        "acceptance workspace only"
    );
    let prompt = std::fs::read_to_string(root.join("request.txt"))?;
    let adb = Adb::discover()?;
    let device = Device::inspect(&adb, transport)?;
    ensure!(
        device.serial == "STEPX_DEVICE_SERIAL",
        "authorized physical phone required"
    );
    let mut bus = AgentBus::connect(&adb, &device)?;
    let key = bus.create_session()?;
    let task = work::start(
        adb.clone(),
        device.clone(),
        WorkRequest {
            prompt,
            workspace: root.clone(),
            session_key: Some(key.clone()),
            model: None,
            attachments: vec![],
        },
    );
    let started = Instant::now();
    let mut events = vec![];
    let mut artifacts = vec![];
    let mut finished = false;
    let mut success = false;
    while started.elapsed() < Duration::from_secs(720) {
        match task.events.recv_timeout(Duration::from_secs(5)) {
            Ok(event) => {
                let entry = match &event {
                    WorkEvent::SessionReady(_) => json!({"kind":"session_ready"}),
                    WorkEvent::ModelResponse { elapsed_ms, .. } => {
                        json!({"kind":"model_response","elapsed_ms":elapsed_ms})
                    }
                    WorkEvent::Status(text) => json!({"kind":"status","text":text}),
                    WorkEvent::Assistant(text) => json!({"kind":"summary","text":text}),
                    WorkEvent::Tool(log) => {
                        json!({"kind":"tool","tool":log.tool,"success":log.success,"detail":log.detail})
                    }
                    WorkEvent::Artifact(file) => {
                        artifacts.push(file.clone());
                        json!({"kind":"artifact","path":file.relative_path,"bytes":file.bytes})
                    }
                    WorkEvent::Complete(text) => {
                        finished = true;
                        success = true;
                        json!({"kind":"complete","text":text})
                    }
                    WorkEvent::Failed(text) => {
                        finished = true;
                        json!({"kind":"failed","text":text})
                    }
                    WorkEvent::Cancelled => {
                        finished = true;
                        json!({"kind":"cancelled"})
                    }
                };
                println!(
                    "{}",
                    json!({"elapsed_s":started.elapsed().as_secs(),"event":entry})
                );
                events.push(entry);
                if finished {
                    break;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => (),
            Err(_) => break,
        }
    }
    if !finished {
        task.cancel.store(true, Ordering::SeqCst);
        let until = Instant::now() + Duration::from_secs(45);
        while Instant::now() < until {
            if matches!(
                task.events.recv_timeout(Duration::from_secs(2)),
                Ok(WorkEvent::Cancelled | WorkEvent::Complete(_) | WorkEvent::Failed(_))
            ) {
                break;
            }
        }
    }
    let cleanup = bus.call("sessions.delete", json!({"key":key})).is_ok();
    let evidence = json!({"completed":finished,"model_declared_complete":success,"elapsed_s":started.elapsed().as_secs(),
        "temporary_session_cleanup":cleanup,"events":events,"artifacts":artifacts,
        "artifact_contents_verified":false});
    std::fs::write(
        root.join("execution.json"),
        serde_json::to_vec_pretty(&evidence)?,
    )?;
    ensure!(
        finished && success && cleanup,
        "Work task did not finish successfully or cleanup failed"
    );
    ensure!(
        !artifacts.is_empty(),
        "Model completed without producing an artifact"
    );
    Ok(())
}

//! Opt-in live model test. Reads the locally saved Studio Lab model config; no key in argv/logs.
use anyhow::{bail, Context, Result};
use std::time::Duration;
use stepx_companion::studio_lab::{self, LabEvent, LabRequest};
fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|s| s == "--models") {
        let models = studio_lab::list_models()?;
        println!("models_available={}", models.len());
        return Ok(());
    }
    if args.iter().any(|s| s == "--connection") {
        let result = studio_lab::test_connection()?;
        println!(
            "model={} connection=passed elapsed_ms={}",
            result.model, result.elapsed_ms
        );
        return Ok(());
    }
    let prompt = args
        .iter()
        .position(|s| s == "--generate")
        .and_then(|i| args.get(i + 1))
        .context("Usage: verify_studio_lab --connection | --models | --generate <prompt>")?;
    let handle = studio_lab::start(LabRequest {
        prompt: prompt.clone(),
        source: None,
    });
    loop {
        match handle.events.recv_timeout(Duration::from_secs(1060))? {
            LabEvent::Status(value) => println!("status={value}"),
            LabEvent::Artifact(artifact) => println!(
                "round={} passed={} {}",
                artifact.round, artifact.passed, artifact.summary
            ),
            LabEvent::Complete(artifact) => {
                println!(
                    "completed=true html={} report={}",
                    artifact.html_path.display(),
                    artifact.report_path.display()
                );
                return Ok(());
            }
            LabEvent::Failed(message) => bail!("{message}"),
            LabEvent::Cancelled => bail!("Task cancelled"),
        }
    }
}

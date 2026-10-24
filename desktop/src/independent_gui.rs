//! Desktop observer/controller. Model requests and execution live entirely on phone.
use crate::{
    adb::Adb,
    amoo_gui::{GuiRunResult, GuiRunState},
    amoo_screen::ScreenEvent,
    companion_gui,
    device::Device,
    gui_agent::Control,
};
use anyhow::{ensure, Result};
use serde_json::json;
use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
pub fn run(
    adb: &Adb,
    device: &Device,
    goal: &str,
    _id: Option<&str>,
    control: &Control,
    mut emit: impl FnMut(ScreenEvent),
) -> Result<GuiRunResult> {
    let start = companion_gui::call(
        adb,
        device,
        json!({"op":"agent","agentOp":"start","goal":goal}),
    )?;
    let id = start["task"].as_str().unwrap_or("").to_owned();
    ensure!(!id.is_empty(), "手机没有返回任务标识");
    let began = Instant::now();
    let mut paused = false;
    let mut stopping = false;
    let mut progress = String::new();
    loop {
        if control.stop.load(Ordering::Relaxed) && !stopping {
            companion_gui::call(
                adb,
                device,
                json!({"op":"agent","agentOp":"stop","task":id}),
            )?;
            stopping = true;
        }
        if control.pause.load(Ordering::Relaxed) != paused && !stopping {
            paused = control.pause.load(Ordering::Relaxed);
            companion_gui::call(
                adb,
                device,
                json!({"op":"agent","agentOp":if paused{"pause"}else{"resume"},"task":id}),
            )?;
        }
        let value = match companion_gui::call(
            adb,
            device,
            json!({"op":"agent","agentOp":"status","task":id}),
        ) {
            Ok(v) => v,
            Err(_) => {
                return Ok(GuiRunResult {
                    state: GuiRunState::Disconnected,
                    message: "desktop 连接中断；任务继续由手机执行，可在伴侣查看或停止。".into(),
                    invocation_id: id,
                    raw_result: None,
                    elapsed_ms: began.elapsed().as_millis() as u64,
                })
            }
        };
        ensure!(
            value["task"].as_str() == Some(id.as_str()),
            "手机任务已经改变，请重新查看当前任务"
        );
        let next = value["progress"].as_str().unwrap_or("");
        if next != progress {
            progress = next.into();
            emit(ScreenEvent::Status(progress.clone()));
        }
        if !matches!(
            value["phase"].as_str(),
            Some("running" | "queued" | "paused" | "cancelling")
        ) {
            let completed = value["phase"] == "completed"
                && value["result"]["verified"] == true
                && value["result"]["phoneStopped"] == true;
            let state = if completed {
                GuiRunState::Completed
            } else if value["phase"] == "cancelled" {
                GuiRunState::WaitingCancelled
            } else {
                GuiRunState::Failed
            };
            return Ok(GuiRunResult {
                state,
                message: progress,
                invocation_id: id,
                elapsed_ms: began.elapsed().as_millis() as u64,
                raw_result: Some(value),
            });
        }
        if began.elapsed() > Duration::from_secs(1250) {
            return Ok(GuiRunResult {
                state: GuiRunState::TimedOut,
                message: "desktop 等待超时，请在手机查看任务状态。".into(),
                invocation_id: id,
                elapsed_ms: began.elapsed().as_millis() as u64,
                raw_result: Some(value),
            });
        }
        std::thread::sleep(Duration::from_secs(2));
    }
}

//! Passive conversation reads never occupy the foreground command worker.
//! Any foreground action invalidates an older snapshot before it can overwrite a send.
use super::*;

pub(super) struct SnapshotJob {
    generation: u64,
    serial: String,
    key: Option<String>,
    receiver: mpsc::Receiver<std::result::Result<Option<RealAmooData>, String>>,
}
struct SyncRequest {
    key: Option<String>,
    pending: Vec<String>,
    task: bool,
    reply: mpsc::Sender<std::result::Result<Option<RealAmooData>, String>>,
    generation: u64,
}
pub(super) struct SyncWorker {
    transport: String,
    sender: mpsc::Sender<SyncRequest>,
}
impl SyncWorker {
    fn start(adb: Adb, device: Device) -> Self {
        let (sender, receiver) = mpsc::channel::<SyncRequest>();
        let transport = device.transport.clone();
        std::thread::spawn(move || {
            let mut bus: Option<(bool, amoo::AgentBus)> = None;
            let mut previous: Option<(u64, u64)> = None;
            loop {
                let request = match receiver.recv_timeout(Duration::from_secs(12)) {
                    Ok(request) => request,
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        bus = None;
                        continue;
                    }
                    Err(_) => break,
                };
                // Escalation/de-escalation releases the previous task lease on this worker,
                // never on the UI thread. An idle observer must not keep the phone awake.
                if bus.as_ref().is_some_and(|(task, _)| *task != request.task) {
                    bus = None;
                }
                let started = Instant::now();
                let reused = bus.is_some();
                let result = (|| -> Result<RealAmooData> {
                    if bus.is_none() {
                        bus = Some((
                            request.task,
                            if request.task {
                                amoo::AgentBus::connect(&adb, &device)?
                            } else {
                                amoo::AgentBus::observe(&adb, &device)?
                            },
                        ));
                    }
                    bus.as_mut()
                        .unwrap()
                        .1
                        .snapshot_with_pending(request.key.as_deref(), &request.pending)
                })()
                .map_err(|e| format!("{e:#}"));
                if result.is_err() {
                    bus = None;
                }
                if std::env::var_os("STEPX_PERF_TRACE").is_some() {
                    eprintln!(
                        "AMOO_SYNC ok={} reused={} elapsed_ms={}",
                        result.is_ok(),
                        reused,
                        started.elapsed().as_millis()
                    );
                }
                let result = result.map(|data| {
                    use std::hash::{Hash, Hasher};
                    let mut hash = std::collections::hash_map::DefaultHasher::new();
                    serde_json::to_vec(&data)
                        .unwrap_or_default()
                        .hash(&mut hash);
                    let signature = (request.generation, hash.finish());
                    if previous == Some(signature) {
                        None
                    } else {
                        previous = Some(signature);
                        Some(data)
                    }
                });
                if result.is_err() {
                    previous = None;
                }
                let _ = request.reply.send(result);
            }
        });
        Self { transport, sender }
    }
}
fn matches_request(
    job: &SnapshotJob,
    generation: u64,
    serial: Option<&str>,
    key: Option<&str>,
    busy: bool,
) -> bool {
    !busy
        && job.generation == generation
        && serial == Some(job.serial.as_str())
        && key == job.key.as_deref()
}
impl Companion {
    pub(super) fn begin_amoo_sync(
        &mut self,
        key: Option<String>,
        pending: Vec<String>,
        initial: bool,
    ) {
        if self.amoo_ui.snapshot_job.is_some() {
            return;
        }
        let Some(device) = self.selected.clone() else {
            return;
        };
        let Ok(adb) = Adb::discover() else {
            return;
        };
        let (sender, receiver) = mpsc::channel();
        self.amoo_ui.snapshot_job = Some(SnapshotJob {
            generation: self.amoo_ui.snapshot_generation,
            serial: device.serial.clone(),
            key: key.clone(),
            receiver,
        });
        if self
            .amoo_ui
            .snapshot_worker
            .as_ref()
            .is_none_or(|w| w.transport != device.transport)
        {
            self.amoo_ui.snapshot_worker = Some(SyncWorker::start(adb, device));
        }
        if self
            .amoo_ui
            .snapshot_worker
            .as_ref()
            .unwrap()
            .sender
            .send(SyncRequest {
                key,
                pending: pending.clone(),
                task: initial || !pending.is_empty(),
                reply: sender,
                generation: self.amoo_ui.snapshot_generation,
            })
            .is_err()
        {
            self.amoo_ui.snapshot_worker = None;
            self.amoo_ui.snapshot_job = None;
        }
    }

    pub(super) fn poll_amoo_sync(&mut self) -> bool {
        let Some(job) = self.amoo_ui.snapshot_job.as_ref() else {
            return false;
        };
        let result = match job.receiver.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return false,
            Err(mpsc::TryRecvError::Disconnected) => Err("后台同步已结束，下次自动重试".into()),
        };
        let job = self.amoo_ui.snapshot_job.take().unwrap();
        let key = self
            .amoo_sessions
            .get(self.amoo_session_idx)
            .map(|s| s.session_key.as_str());
        if !matches_request(
            &job,
            self.amoo_ui.snapshot_generation,
            self.selected.as_ref().map(|d| d.serial.as_str()),
            key,
            self.busy || self.work.running || self.amoo_ui.gui_running,
        ) {
            // A newer foreground operation or session switch owns the state.
            // A discarded result must not become the deduplication baseline:
            // recreate the worker so the next accepted read is delivered in full.
            self.amoo_ui.snapshot_worker = None;
            return false;
        }
        match result {
            Ok(None) => return false,
            Ok(Some(data)) => self.apply(Outcome::AmooRealData(data, "已刷新后台对话".into())),
            Err(_) => self.apply(Outcome::AmooObservationUnavailable),
        }
        true
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn passive_reads_cannot_overwrite_newer_actions_devices_or_sessions() {
        let (_sender, receiver) = mpsc::channel();
        let job = SnapshotJob {
            generation: 3,
            serial: "phone-a".into(),
            key: Some("chat-a".into()),
            receiver,
        };
        assert!(matches_request(
            &job,
            3,
            Some("phone-a"),
            Some("chat-a"),
            false
        ));
        assert!(!matches_request(
            &job,
            4,
            Some("phone-a"),
            Some("chat-a"),
            false
        ));
        assert!(!matches_request(
            &job,
            3,
            Some("phone-b"),
            Some("chat-a"),
            false
        ));
        assert!(!matches_request(
            &job,
            3,
            Some("phone-a"),
            Some("chat-b"),
            false
        ));
        assert!(!matches_request(&job, 3, None, Some("chat-a"), false));
        assert!(!matches_request(
            &job,
            3,
            Some("phone-a"),
            Some("chat-a"),
            true
        ));
    }
}

//! Connection discovery runs independently of page errors and foreground work.
use super::*;
use stepx_companion::{pairing::WirelessIdentity, reconnect};
struct Snapshot {
    devices: Vec<Transport>,
    services: Vec<Service>,
    identity: Option<WirelessIdentity>,
}
#[derive(Default)]
pub(super) struct ConnectionWatch {
    job: Option<(Option<String>, mpsc::Receiver<Result<Snapshot>>)>,
    misses: u8,
    pub detached_serial: Option<String>,
    pub identity_at: Option<Instant>,
}
impl Companion {
    pub(super) fn begin_connection_watch(&mut self) {
        if self.demo_mode || self.connection_watch.job.is_some() {
            return;
        }
        let current = self.selected.clone();
        let origin = current.as_ref().map(|d| d.transport.clone());
        let prefs = self.preferences.clone();
        let hosts = self.pairing_hosts.clone();
        let identity_due = self
            .connection_watch
            .identity_at
            .is_none_or(|t| t.elapsed() > Duration::from_secs(20));
        if identity_due {
            self.connection_watch.identity_at = Some(Instant::now());
        }
        let (tx, rx) = mpsc::channel();
        self.connection_watch.job = Some((origin, rx));
        std::thread::spawn(move || {
            let result = (|| -> Result<Snapshot> {
                let adb = Adb::discover()?;
                let devices =
                    parse_devices(&adb.run_timeout(&["devices", "-l"], Duration::from_secs(3))?);
                let online = current.as_ref().is_some_and(|d| {
                    devices
                        .iter()
                        .any(|t| t.id == d.transport && t.state == "device")
                });
                if online {
                    let identity = if identity_due {
                        current
                            .as_ref()
                            .and_then(|d| reconnect::network_identity(&adb, d))
                    } else {
                        None
                    };
                    Ok(Snapshot {
                        devices,
                        services: vec![],
                        identity,
                    })
                } else {
                    let (devices, services) = reconnect::scan(&adb, &prefs, &hosts)?;
                    Ok(Snapshot {
                        devices,
                        services,
                        identity: None,
                    })
                }
            })();
            let _ = tx.send(result);
        });
    }
    pub(super) fn tick_connection_watch(&mut self, ctx: &mut ViewContext<Self>) {
        let Some((origin, rx)) = &self.connection_watch.job else {
            return;
        };
        let result = match rx.try_recv() {
            Ok(r) => r,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(_) => Err(anyhow::anyhow!("连接检测未完成")),
        };
        let origin = origin.clone();
        self.connection_watch.job = None;
        if self.selected.as_ref().map(|d| d.transport.clone()) != origin {
            return;
        }
        let Ok(snapshot) = result else {
            return;
        };
        if let (Some(d), Some(mut identity)) = (&self.selected, snapshot.identity) {
            // A Wi-Fi transition may temporarily remove the port, not the remembered peer.
            if identity.endpoint.is_empty() {
                if let Some(prior) = self.preferences.wireless.get(&d.serial) {
                    if identity.guid == prior.guid {
                        identity.endpoint = prior.endpoint.clone();
                    }
                }
            }
            if self.preferences.wireless.get(&d.serial) != Some(&identity) {
                self.preferences.wireless.insert(d.serial.clone(), identity);
                if let Err(e) = self.preferences.save() {
                    eprintln!("Unable to remember wireless connection: {e}");
                }
            }
        }
        let online = origin.as_ref().is_some_and(|id| {
            snapshot
                .devices
                .iter()
                .any(|d| &d.id == id && d.state == "device")
        });
        if online {
            self.connection_watch.misses = 0;
        } else {
            self.connection_watch.misses = self.connection_watch.misses.saturating_add(1);
        }
        // Two absent snapshots avoid clearing a connection during ADB discovery. Tasks and drafts survive.
        if origin.is_some() && !online && self.connection_watch.misses < 2 {
            return;
        }
        if !online {
            if self.busy {
                return;
            } // Apply after the foreground result; discovery itself is never blocked by it.
            self.apply(Outcome::Transports(snapshot.devices, snapshot.services));
            ctx.notify();
        } else if self.page == Page::Connect {
            self.devices = snapshot.devices;
            ctx.notify();
        }
    }
}

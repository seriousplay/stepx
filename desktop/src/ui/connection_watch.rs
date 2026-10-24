//! Connection health never occupies the foreground task queue or clears accepted task state on one miss.
use super::*;
#[derive(Default)]
pub(super) struct ConnectionWatch {
    job: Option<(String, mpsc::Receiver<Result<Vec<Transport>>>)>,
    misses: u8,
}
impl Companion {
    pub(super) fn begin_connection_watch(&mut self) {
        if self.connection_watch.job.is_some() {
            return;
        }
        let Some(d) = self.selected.as_ref() else {
            return;
        };
        let transport = d.transport.clone();
        let (tx, rx) = mpsc::channel();
        self.connection_watch.job = Some((transport, rx));
        std::thread::spawn(move || {
            let r = Adb::discover()
                .and_then(|adb| adb.run(&["devices", "-l"]).map(|s| parse_devices(&s)));
            let _ = tx.send(r);
        });
    }
    pub(super) fn tick_connection_watch(&mut self, ctx: &mut ViewContext<Self>) {
        let Some((transport, rx)) = &self.connection_watch.job else {
            return;
        };
        let result = match rx.try_recv() {
            Ok(r) => r,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(_) => Err(anyhow::anyhow!("连接检测未完成")),
        };
        let transport = transport.clone();
        self.connection_watch.job = None;
        if self.selected.as_ref().map(|d| &d.transport) != Some(&transport) {
            self.connection_watch.misses = 0;
            return;
        }
        let Ok(devices) = result else {
            return;
        };
        let online = devices
            .iter()
            .any(|d| d.id == transport && d.state == "device");
        if online {
            self.connection_watch.misses = 0;
        } else {
            self.connection_watch.misses = self.connection_watch.misses.saturating_add(1);
        }
        // Confirm disconnection twice; preserve task/draft state while transient USB/network discovery settles.
        if !online
            && (self.connection_watch.misses < 2
                || self.busy
                || self.work.running
                || self.amoo_ui.gui_running)
        {
            return;
        }
        if !online {
            self.apply(Outcome::Transports(devices, self.mdns_services.clone()));
            ctx.notify();
        } else if self.page == Page::Connect {
            self.devices = devices;
            ctx.notify();
        }
    }
}

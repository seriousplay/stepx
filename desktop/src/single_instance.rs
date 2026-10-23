//! Per-user desktop ownership. OS locks survive neither crashes nor reboots.
//! A loopback-only channel asks the existing UI to show its main window.
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions, TryLockError},
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[derive(Serialize, Deserialize)]
struct Endpoint {
    port: u16,
    token: [u8; 32],
    pid: u32,
}

pub enum Claim {
    Primary(Instance),
    Activated,
}

pub struct Instance {
    // Never unlink this file: other launchers may already hold a handle to it.
    _lock: File,
    address: SocketAddr,
    requested: Arc<AtomicBool>,
    stopped: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Instance {
    pub fn claim(directory: &Path) -> Result<Claim> {
        std::fs::create_dir_all(directory).context("无法创建客户端运行目录")?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
            // The app-specific directory contains only activation metadata.
            std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700))?;
            options.mode(0o600);
        }
        let lock = options.open(directory.join("desktop.lock"))?;
        let endpoint_path = directory.join("desktop-instance.json");
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            match lock.try_lock() {
                Ok(()) => break,
                Err(TryLockError::WouldBlock) => {
                    if activate(&endpoint_path).is_ok() {
                        return Ok(Claim::Activated);
                    }
                    if Instant::now() >= deadline {
                        anyhow::bail!("StepX Companion 已在运行，暂未响应唤起请求；请稍后再试");
                    }
                    // Only the second launch waits; the existing UI is unaffected.
                    thread::sleep(Duration::from_millis(50));
                }
                Err(TryLockError::Error(error)) => return Err(error.into()),
            }
        }
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let address = listener.local_addr()?;
        let endpoint = Endpoint {
            port: address.port(),
            token: rand::random(),
            pid: std::process::id(),
        };
        let mut metadata = tempfile::NamedTempFile::new_in(directory)?;
        serde_json::to_writer(&mut metadata, &endpoint)?;
        metadata.flush()?;
        metadata.persist(&endpoint_path).map_err(|e| e.error)?;
        let requested = Arc::new(AtomicBool::new(false));
        let stopped = Arc::new(AtomicBool::new(false));
        let (notify, stop) = (requested.clone(), stopped.clone());
        let worker = thread::Builder::new()
            .name("desktop-activation".into())
            .spawn(move || {
                for connection in listener.incoming() {
                    if stop.load(Ordering::Acquire) {
                        break;
                    }
                    let Ok(mut connection) = connection else {
                        break;
                    };
                    let timeout = Some(Duration::from_millis(250));
                    let _ = connection.set_read_timeout(timeout);
                    let _ = connection.set_write_timeout(timeout);
                    let mut token = [0u8; 32];
                    if connection.read_exact(&mut token).is_ok() && token == endpoint.token {
                        notify.store(true, Ordering::Release);
                        let _ = connection.write_all(b"OK");
                    }
                }
            })?;
        Ok(Claim::Primary(Self {
            _lock: lock,
            address,
            requested,
            stopped,
            worker: Some(worker),
        }))
    }

    pub fn activation_requests(&self) -> Arc<AtomicBool> {
        self.requested.clone()
    }
}

fn activate(path: &Path) -> Result<()> {
    // Bound the metadata read and never accept a non-loopback address from disk.
    let endpoint: Endpoint = serde_json::from_reader(File::open(path)?.take(1024))?;
    let timeout = Duration::from_millis(250);
    let mut stream = TcpStream::connect_timeout(
        &SocketAddr::from((Ipv4Addr::LOCALHOST, endpoint.port)),
        timeout,
    )?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    #[cfg(windows)]
    unsafe {
        // The launcher has the foreground grant from the user's click. Pass it
        // to the existing process before it restores/focuses the main window.
        #[link(name = "user32")]
        extern "system" {
            fn AllowSetForegroundWindow(process_id: u32) -> i32;
        }
        AllowSetForegroundWindow(endpoint.pid);
    }
    stream.write_all(&endpoint.token)?;
    let mut reply = [0u8; 2];
    stream.read_exact(&mut reply)?;
    anyhow::ensure!(&reply == b"OK", "客户端唤起响应无效");
    Ok(())
}

impl Drop for Instance {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        // Wake a blocking accept without periodically polling while idle.
        let _ = TcpStream::connect_timeout(&self.address, Duration::from_millis(250));
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duplicate_launch_activates_owner_and_normal_exit_releases_lock() {
        let dir = tempfile::tempdir().unwrap();
        let Claim::Primary(first) = Instance::claim(dir.path()).unwrap() else {
            panic!()
        };
        assert!(matches!(
            Instance::claim(dir.path()).unwrap(),
            Claim::Activated
        ));
        assert!(first.requested.swap(false, Ordering::AcqRel));
        drop(first);
        assert!(matches!(
            Instance::claim(dir.path()).unwrap(),
            Claim::Primary(_)
        ));
    }
    #[test]
    fn unknown_activation_token_does_not_focus_window() {
        let dir = tempfile::tempdir().unwrap();
        let Claim::Primary(first) = Instance::claim(dir.path()).unwrap() else {
            panic!()
        };
        let mut socket = TcpStream::connect(first.address).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        socket.write_all(&[0u8; 32]).unwrap();
        let mut reply = [0u8; 2];
        assert!(socket.read_exact(&mut reply).is_err());
        assert!(!first.requested.load(Ordering::Acquire));
    }
    #[test]
    fn child_holds_lock() {
        let Some(dir) = std::env::var_os("STEPX_INSTANCE_TEST_CHILD") else {
            return;
        };
        let dir = std::path::PathBuf::from(dir);
        let Claim::Primary(_guard) = Instance::claim(&dir).unwrap() else {
            panic!()
        };
        std::fs::write(dir.join("ready"), b"ready").unwrap();
        thread::sleep(Duration::from_secs(15));
    }
    #[test]
    fn killed_process_does_not_leave_a_stale_lock() {
        let dir = tempfile::tempdir().unwrap();
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "single_instance::tests::child_holds_lock"])
            .env("STEPX_INSTANCE_TEST_CHILD", dir.path())
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !dir.path().join("ready").exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        let ready = dir.path().join("ready").exists();
        let duplicate = ready && matches!(Instance::claim(dir.path()), Ok(Claim::Activated));
        let _ = child.kill();
        let _ = child.wait();
        assert!(
            ready && duplicate,
            "child did not own/activate the instance"
        );
        assert!(matches!(
            Instance::claim(dir.path()).unwrap(),
            Claim::Primary(_)
        ));
    }
}

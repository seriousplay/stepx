//! Hermetic cancellation regression: a local fake endpoint stalls an actual worker.
//! Does not call a real model or touch the user's saved model settings.
use anyhow::{ensure, Result};
use std::{
    io::Read,
    net::TcpListener,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};
use stepx_companion::studio_lab::{self, LabEvent, LabRequest};
fn main() -> Result<()> {
    let temp = tempfile::tempdir()?;
    std::env::set_var("STEPX_CONFIG_DIR", temp.path().join("config"));
    std::env::set_var("STEPX_STATE_DIR", temp.path().join("state"));
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    let stop = Arc::new(AtomicBool::new(false));
    let server_stop = stop.clone();
    let (tx, ready) = mpsc::channel();
    let server = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let mut buffer = [0; 4096];
        let _ = socket.read(&mut buffer);
        let _ = tx.send(());
        while !server_stop.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(25));
        }
    });
    studio_lab::save_config(
        &format!("http://127.0.0.1:{port}/v1"),
        "openai",
        "test-only",
        "test-only",
        false,
    )?;
    let handle = studio_lab::start(LabRequest {
        prompt: "Cancellation test".into(),
        source: None,
    });
    ready.recv_timeout(Duration::from_secs(10))?;
    let began = Instant::now();
    handle.cancel.store(true, Ordering::SeqCst);
    loop {
        match handle.events.recv_timeout(Duration::from_secs(5))? {
            LabEvent::Cancelled => break,
            LabEvent::Failed(error) => anyhow::bail!("Unexpected worker error: {error}"),
            _ => {}
        }
    }
    stop.store(true, Ordering::Relaxed);
    server.join().unwrap();
    ensure!(
        began.elapsed() < Duration::from_secs(3),
        "Cancellation was too slow"
    );
    println!(
        "stalled_model_request_cancelled=true elapsed_ms={}",
        began.elapsed().as_millis()
    );
    Ok(())
}

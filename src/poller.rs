use std::path::Path;
use std::process;
use std::time::{Instant, SystemTime};

use tokio::time;
use tracing::{debug, error, info, trace};

use crate::config::Config;
use crate::database::RuntimeLock;
use crate::log::{Operation, StatCollector};
use crate::poll_guard::PollGuard;
use crate::snapshots;

/// If the directory is a repo, attempts to create a snapshot.
/// Otherwise, recurses into each child directory.
#[tracing::instrument]
fn process_directory(current_path: &Path, guard: &mut PollGuard) {
    let mut op: Option<snapshots::CaptureStatus> = None;
    let mut error: Option<String> = None;
    let start_time = Instant::now();

    if guard.dir_changed(current_path) {
        debug!(
            "Potential change detected in repo: path = {path}",
            path = current_path.to_str().unwrap_or("")
        );
        match snapshots::capture(current_path) {
            Ok(Some(status)) => op = Some(status),
            Ok(None) => (),
            Err(err) => {
                error = Some(format!("{err}"));
            }
        }
    } else {
        trace!(
            "No files in repo have changed: path = {path}",
            path = current_path.to_str().unwrap_or("")
        );
    }

    let latency = (Instant::now() - start_time).as_secs_f32();
    let repo = current_path
        .to_str()
        .unwrap_or("<invalid path>")
        .to_string();
    let mut operation = Operation::Snapshot {
        repo,
        op,
        error,
        latency,
    };
    if operation.should_log() {
        info!(operation = operation.log_str().as_str(), "info_operation")
    }
}

#[tracing::instrument]
fn do_task(stats: &mut StatCollector, guard: &mut PollGuard) {
    let runtime_lock = RuntimeLock::load();
    if runtime_lock.pid != Some(process::id()) {
        error!(
            "Shutting down because other poller took lock: {:?}",
            runtime_lock.pid
        );
        process::exit(1);
    }

    let config = Config::load();

    let loop_start = Instant::now();
    for repo in config.git_repos() {
        let dir_start = Instant::now();
        process_directory(repo.as_path(), guard);
        stats.record_dir(Instant::now() - dir_start);
    }
    stats.record_loop(Instant::now() - loop_start);

    if stats.should_log() {
        info!(operation = stats.log_str().as_str(), "poller_stats");
    }
}

pub async fn start() {
    let file = match RuntimeLock::acquire_exclusive() {
        Ok(f) => f,
        Err(e) => {
            error!("Shutting down because another poller is running or lock file is locked: {e}");
            process::exit(1);
        }
    };

    let mut runtime_lock = RuntimeLock::empty();
    runtime_lock.pid = Some(process::id());
    runtime_lock.start_time = Some(SystemTime::now());
    if let Err(e) = runtime_lock.write_metadata(&file) {
        error!("Failed to write runtime lock metadata: {e}");
    }
    info!(pid = std::process::id());

    let (shutdown_tx, mut shutdown_rx) = tokio::sync::broadcast::channel(1);

    #[cfg(unix)]
    tokio::spawn(run_ipc_server(shutdown_tx.clone()));

    let mut stats = StatCollector::new();
    let mut guard = PollGuard::new();
    loop {
        tokio::select! {
            _ = time::sleep(time::Duration::from_secs(5)) => {
                let _keep_lock = &file;
                do_task(&mut stats, &mut guard);
            }
            _ = shutdown_rx.recv() => {
                info!("Shutting down poller task...");
                break;
            }
        }
    }
}

#[cfg(unix)]
pub fn socket_path() -> std::path::PathBuf {
    RuntimeLock::default_path().parent().unwrap().join("dura.sock")
}

#[cfg(unix)]
pub async fn send_uds_command(command: &str) -> Result<String, Box<dyn std::error::Error>> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let path = socket_path();
    let mut stream = tokio::net::UnixStream::connect(path).await?;
    let req = serde_json::json!({ "command": command }).to_string();
    stream.write_all(req.as_bytes()).await?;
    stream.shutdown().await?;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await?;
    Ok(String::from_utf8(buf)?)
}

#[cfg(unix)]
pub async fn run_ipc_server(shutdown_tx: tokio::sync::broadcast::Sender<()>) {
    use tokio::net::UnixListener;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    
    let path = socket_path();
    if path.exists() {
        let _ = std::fs::remove_file(&path);
    }

    let listener = match UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) => {
            error!("Failed to bind to UDS socket: {e}");
            return;
        }
    };

    let mut shutdown_rx = shutdown_tx.subscribe();
    loop {
        tokio::select! {
            accept_res = listener.accept() => {
                match accept_res {
                    Ok((mut stream, _)) => {
                        let shutdown_tx_clone = shutdown_tx.clone();
                        tokio::spawn(async move {
                            let mut buf = [0; 1024];
                            match stream.read(&mut buf).await {
                                Ok(size) if size > 0 => {
                                    let req_str = String::from_utf8_lossy(&buf[..size]);
                                    let res = handle_ipc_command(&req_str, &shutdown_tx_clone).await;
                                    let _ = stream.write_all(res.as_bytes()).await;
                                }
                                _ => {}
                            }
                        });
                    }
                    Err(e) => {
                        error!("Error accepting UDS connection: {e}");
                    }
                }
            }
            _ = shutdown_rx.recv() => {
                break;
            }
        }
    }
    let _ = std::fs::remove_file(&path);
}

#[cfg(unix)]
async fn handle_ipc_command(req_str: &str, shutdown_tx: &tokio::sync::broadcast::Sender<()>) -> String {
    #[derive(serde::Deserialize)]
    struct IpcRequest {
        command: String,
    }

    let req: IpcRequest = match serde_json::from_str(req_str.trim()) {
        Ok(r) => r,
        Err(_) => return serde_json::json!({"status": "error", "message": "Invalid JSON"}).to_string(),
    };

    match req.command.as_str() {
        "kill" => {
            let _ = shutdown_tx.send(());
            serde_json::json!({"status": "ok", "message": "Shutting down"}).to_string()
        }
        "reload" => {
            serde_json::json!({"status": "ok", "message": "Config reloaded"}).to_string()
        }
        "status" => {
            let config = Config::load();
            serde_json::json!({"status": "ok", "message": format!("Watching {} paths", config.repos.len())}).to_string()
        }
        _ => serde_json::json!({"status": "error", "message": "Unknown command"}).to_string(),
    }
}

#[cfg(not(unix))]
pub fn socket_path() -> std::path::PathBuf {
    RuntimeLock::default_path()
}

#[cfg(not(unix))]
pub async fn send_uds_command(_command: &str) -> Result<String, Box<dyn std::error::Error>> {
    Err("UDS IPC is not supported on this platform".into())
}

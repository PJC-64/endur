use std::path::Path;
use std::process;
use std::time::{Instant, SystemTime};

use tracing::{debug, error, info};

use crate::config::Config;
use crate::database::RuntimeLock;
use crate::log::Operation;
use crate::snapshots;

/// If the directory is a repo, attempts to create a snapshot.
#[tracing::instrument]
fn process_directory(current_path: &Path) {
    let mut op: Option<snapshots::CaptureStatus> = None;
    let mut error: Option<String> = None;
    let start_time = Instant::now();

    debug!(
        "Checking repo for changes: path = {path}",
        path = current_path.to_str().unwrap_or("")
    );
    match snapshots::capture(current_path) {
        Ok(Some(status)) => op = Some(status),
        Ok(None) => (),
        Err(err) => {
            error = Some(format!("{err}"));
        }
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
    if let Err(e) = runtime_lock.write_metadata() {
        error!("Failed to write runtime lock metadata: {e}");
    }
    info!(pid = std::process::id());
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::broadcast::channel::<()>(1);
    let (reload_tx, mut reload_rx) = tokio::sync::mpsc::unbounded_channel::<()>();
    let (event_tx, mut event_rx) =
        tokio::sync::mpsc::unbounded_channel::<(std::path::PathBuf, std::path::PathBuf)>();

    let event_tx_clone = event_tx.clone();
    let mut watcher_manager = match crate::watcher::WatcherManager::new(move |repo, file| {
        let _ = event_tx_clone.send((repo, file));
    }) {
        Ok(wm) => wm,
        Err(e) => {
            error!("Failed to initialize file watcher: {e}");
            process::exit(1);
        }
    };

    let mut currently_watched = std::collections::HashSet::new();

    // Initial load
    let config = Config::load();
    for repo in config.git_repos() {
        watcher_manager.watch_repo(&repo);
        currently_watched.insert(repo);
    }

    #[cfg(any(unix, windows))]
    tokio::spawn(run_ipc_server(shutdown_tx.clone(), reload_tx.clone()));

    #[cfg(all(not(unix), not(windows)))]
    let _ = (&shutdown_tx, &reload_tx);

    let mut pending_captures = std::collections::HashMap::<std::path::PathBuf, Instant>::new();

    // Run initial capture on all repos
    for repo in &currently_watched {
        let _keep_lock = &file;
        process_directory(repo);
    }

    let signal_fut = wait_for_signal();
    tokio::pin!(signal_fut);

    loop {
        tokio::select! {
            _ = &mut signal_fut => {
                info!("Shutdown signal received. Starting graceful shutdown...");
                let _ = shutdown_tx.send(());
                break;
            }
            Some(_) = reload_rx.recv() => {
                info!("Reloading configuration...");
                let config = Config::load();
                let new_repos: std::collections::HashSet<std::path::PathBuf> = config.git_repos().collect();
                for repo in &currently_watched {
                    if !new_repos.contains(repo) {
                        watcher_manager.unwatch_repo(repo);
                    }
                }
                for repo in &new_repos {
                    if !currently_watched.contains(repo) {
                        watcher_manager.watch_repo(repo);
                    }
                }
                currently_watched = new_repos;
            }
            Some((repo, _file)) = event_rx.recv() => {
                pending_captures.insert(repo, Instant::now() + tokio::time::Duration::from_millis(500));
            }
            _ = shutdown_rx.recv() => {
                info!("Shutting down poller task...");
                break;
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                let now = Instant::now();
                let mut due_repos = Vec::new();
                for (repo, due_time) in &pending_captures {
                    if now >= *due_time {
                        due_repos.push(repo.clone());
                    }
                }
                for repo in due_repos {
                    pending_captures.remove(&repo);
                    let _keep_lock = &file;
                    process_directory(&repo);
                }
            }
        }
    }

    // Graceful cleanup of runtime lock metadata
    let mut runtime_lock = RuntimeLock::load();
    runtime_lock.pid = None;
    if let Err(e) = runtime_lock.write_metadata() {
        error!("Failed to clear runtime lock metadata: {e}");
    }
}

#[cfg(any(unix, windows))]
pub fn socket_path() -> std::path::PathBuf {
    RuntimeLock::default_path()
        .parent()
        .unwrap()
        .join("endur.sock")
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

#[cfg(windows)]
pub async fn send_uds_command(command: &str) -> Result<String, Box<dyn std::error::Error>> {
    use std::io::{Read, Write};
    use uds_windows::UnixStream;
    let path = socket_path();
    let command_clone = command.to_string();

    let res = tokio::task::spawn_blocking(move || -> Result<String, std::io::Error> {
        let mut stream = UnixStream::connect(path)?;
        let req = serde_json::json!({ "command": command_clone }).to_string();
        stream.write_all(req.as_bytes())?;
        stream.shutdown(std::net::Shutdown::Write)?;
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf)?;
        String::from_utf8(buf).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    })
    .await??;
    Ok(res)
}

#[cfg(unix)]
pub async fn run_ipc_server(
    shutdown_tx: tokio::sync::broadcast::Sender<()>,
    reload_tx: tokio::sync::mpsc::UnboundedSender<()>,
) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::UnixListener;

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
                        let reload_tx_clone = reload_tx.clone();
                        tokio::spawn(async move {
                            let mut buf = [0; 1024];
                            match stream.read(&mut buf).await {
                                Ok(size) if size > 0 => {
                                    let req_str = String::from_utf8_lossy(&buf[..size]);
                                    let res = handle_ipc_command(&req_str, &shutdown_tx_clone, &reload_tx_clone).await;
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

#[cfg(windows)]
pub async fn run_ipc_server(
    shutdown_tx: tokio::sync::broadcast::Sender<()>,
    reload_tx: tokio::sync::mpsc::UnboundedSender<()>,
) {
    use std::io::{Read, Write};
    use uds_windows::UnixListener;

    let path = socket_path();
    if path.exists() {
        let _ = std::fs::remove_file(&path);
    }

    let listener = match UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) => {
            error!("Failed to bind to Windows UDS socket: {e}");
            return;
        }
    };

    let _ = listener.set_nonblocking(true);
    let shutdown_tx_clone = shutdown_tx.clone();
    let reload_tx_clone = reload_tx.clone();

    let rt = tokio::runtime::Handle::current();

    std::thread::spawn(move || {
        let mut shutdown_rx = shutdown_tx_clone.subscribe();

        loop {
            if shutdown_rx.try_recv().is_ok() {
                break;
            }

            match listener.accept() {
                Ok((mut stream, _)) => {
                    let shutdown_tx_task = shutdown_tx_clone.clone();
                    let reload_tx_task = reload_tx_clone.clone();
                    let rt_task = rt.clone();

                    std::thread::spawn(move || {
                        let _ = stream.set_nonblocking(false);
                        let mut buf = [0; 1024];
                        if let Ok(size) = stream.read(&mut buf) {
                            if size > 0 {
                                let req_str = String::from_utf8_lossy(&buf[..size]);
                                let res = rt_task.block_on(async {
                                    handle_ipc_command(&req_str, &shutdown_tx_task, &reload_tx_task)
                                        .await
                                });
                                let _ = stream.write_all(res.as_bytes());
                            }
                        }
                    });
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                Err(_) => {
                    break;
                }
            }
        }

        let _ = std::fs::remove_file(&path);
    });
}

#[cfg(any(unix, windows))]
async fn handle_ipc_command(
    req_str: &str,
    shutdown_tx: &tokio::sync::broadcast::Sender<()>,
    reload_tx: &tokio::sync::mpsc::UnboundedSender<()>,
) -> String {
    #[derive(serde::Deserialize)]
    struct IpcRequest {
        command: String,
    }

    let req: IpcRequest = match serde_json::from_str(req_str.trim()) {
        Ok(r) => r,
        Err(_) => {
            return serde_json::json!({"status": "error", "message": "Invalid JSON"}).to_string()
        }
    };

    match req.command.as_str() {
        "kill" => {
            let _ = shutdown_tx.send(());
            serde_json::json!({"status": "ok", "message": "Shutting down"}).to_string()
        }
        "reload" => {
            let _ = reload_tx.send(());
            serde_json::json!({"status": "ok", "message": "Config reloaded"}).to_string()
        }
        "status" => {
            let config = Config::load();
            serde_json::json!({
                "status": "ok",
                "version": env!("CARGO_PKG_VERSION"),
                "message": format!("Watching {} paths", config.repos.len())
            })
            .to_string()
        }
        _ => serde_json::json!({"status": "error", "message": "Unknown command"}).to_string(),
    }
}

#[cfg(all(not(unix), not(windows)))]
pub fn socket_path() -> std::path::PathBuf {
    RuntimeLock::default_path()
}

#[cfg(all(not(unix), not(windows)))]
pub async fn send_uds_command(_command: &str) -> Result<String, Box<dyn std::error::Error>> {
    Err("UDS IPC is not supported on this platform".into())
}

#[cfg(unix)]
async fn wait_for_signal() {
    use tokio::signal::unix::{signal, SignalKind};

    let mut sigint = signal(SignalKind::interrupt()).expect("failed to register SIGINT handler");
    let mut sigterm = signal(SignalKind::terminate()).expect("failed to register SIGTERM handler");
    let mut sighup = signal(SignalKind::hangup()).expect("failed to register SIGHUP handler");

    tokio::select! {
        _ = sigint.recv() => {
            info!("Received SIGINT signal");
        }
        _ = sigterm.recv() => {
            info!("Received SIGTERM signal");
        }
        _ = sighup.recv() => {
            info!("Received SIGHUP signal");
        }
    }
}

#[cfg(windows)]
async fn wait_for_signal() {
    use tokio::signal::windows::{ctrl_break, ctrl_c};

    let mut sigctrl_c = ctrl_c().expect("failed to register Ctrl+C handler");
    let mut sigctrl_break = ctrl_break().expect("failed to register Ctrl+Break handler");

    tokio::select! {
        _ = sigctrl_c.recv() => {
            info!("Received Ctrl+C event");
        }
        _ = sigctrl_break.recv() => {
            info!("Received Ctrl+Break event");
        }
    }
}

#[cfg(all(not(unix), not(windows)))]
async fn wait_for_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to listen for ctrl_c");
    info!("Received Ctrl+C event (fallback)");
}

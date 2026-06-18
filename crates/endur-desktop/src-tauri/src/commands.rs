use endur::config::{Config, WatchConfig};
use endur::database::RuntimeLock;
use endur::poller;
use endur::service;
use endur::snapshots::{self, SnapshotInfo};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
pub struct DaemonStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub uptime_secs: Option<u64>,
    pub version: Option<String>,
    pub client_version: String,
}

#[tauri::command]
pub async fn get_daemon_status() -> Result<DaemonStatus, String> {
    let mut version = None;
    let is_running = match poller::send_uds_command("status").await {
        Ok(res) => {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&res) {
                version = val
                    .get("version")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
            }
            true
        }
        Err(_) => false,
    };
    let lock = RuntimeLock::load();
    let uptime_secs = if is_running && lock.pid.is_some() {
        lock.start_time
            .and_then(|t| t.elapsed().ok())
            .map(|d| d.as_secs())
    } else {
        None
    };
    Ok(DaemonStatus {
        running: is_running,
        pid: if is_running { lock.pid } else { None },
        uptime_secs,
        version,
        client_version: endur::VERSION.to_string(),
    })
}

#[tauri::command]
pub async fn control_daemon(action: String) -> Result<(), String> {
    match action.as_str() {
        "start" => {
            let home_dir =
                dirs::home_dir().ok_or_else(|| "Could not determine home directory".to_string())?;
            let mut endur_path = home_dir.join(".cargo/bin/endur");
            if !endur_path.exists() {
                endur_path = PathBuf::from("endur");
            }
            let logfile_path = RuntimeLock::get_endur_cache_home().join("endur.log");
            let mut cmd = std::process::Command::new(endur_path);
            cmd.arg("serve").arg("--logfile").arg(logfile_path);
            #[cfg(unix)]
            {
                use std::os::unix::process::CommandExt;
                unsafe {
                    cmd.pre_exec(|| {
                        extern "C" {
                            fn setsid() -> i32;
                        }
                        setsid();
                        Ok(())
                    });
                }
            }
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x00000008 | 0x00000200);
            }
            cmd.spawn()
                .map_err(|e| format!("Failed to spawn daemon: {e}"))?;
            Ok(())
        }
        "stop" => {
            let _ = poller::send_uds_command("kill").await;
            if RuntimeLock::is_active() {
                let mut lock = RuntimeLock::load();
                lock.pid = None;
                lock.save();
            }
            Ok(())
        }
        "restart" => {
            // 1. Terminate the running daemon
            let _ = poller::send_uds_command("kill").await;

            // Wait up to 1 second for the process to exit and release lock
            let mut exited = false;
            for _ in 0..10 {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                if poller::send_uds_command("status").await.is_err() && !RuntimeLock::is_active() {
                    exited = true;
                    break;
                }
            }

            if !exited {
                // Force reset lock PID if it didn't shut down cleanly
                if RuntimeLock::is_active() {
                    let mut lock = RuntimeLock::load();
                    lock.pid = None;
                    lock.save();
                }
            }

            // 2. Start it again
            let home_dir =
                dirs::home_dir().ok_or_else(|| "Could not determine home directory".to_string())?;
            let mut endur_path = home_dir.join(".cargo/bin/endur");
            if !endur_path.exists() {
                endur_path = PathBuf::from("endur");
            }
            let logfile_path = RuntimeLock::get_endur_cache_home().join("endur.log");
            let mut cmd = std::process::Command::new(endur_path);
            cmd.arg("serve").arg("--logfile").arg(logfile_path);
            #[cfg(unix)]
            {
                use std::os::unix::process::CommandExt;
                unsafe {
                    cmd.pre_exec(|| {
                        extern "C" {
                            fn setsid() -> i32;
                        }
                        setsid();
                        Ok(())
                    });
                }
            }
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x00000008 | 0x00000200);
            }
            cmd.spawn()
                .map_err(|e| format!("Failed to spawn daemon: {e}"))?;
            Ok(())
        }
        _ => Err("Invalid action".to_string()),
    }
}

#[tauri::command]
pub fn get_watched_repositories() -> Result<Vec<String>, String> {
    let config = Config::load();
    let repos: Vec<String> = config.repos.keys().cloned().collect();
    Ok(repos)
}

#[tauri::command]
pub async fn toggle_watch_repo(path: String, watch: bool) -> Result<(), String> {
    {
        let mut config = Config::load();
        if watch {
            config
                .set_watch(path.clone(), WatchConfig::default())
                .map_err(|e| e.to_string())?;
        } else {
            config
                .set_unwatch(path.clone())
                .map_err(|e| e.to_string())?;
        }
        config.save();
    }
    let _ = poller::send_uds_command("reload").await;
    Ok(())
}

#[tauri::command]
pub fn get_snapshots(
    repo_path: String,
    show_all: Option<bool>,
) -> Result<Vec<SnapshotInfo>, String> {
    let path = Path::new(&repo_path);
    snapshots::list_snapshots(path, show_all.unwrap_or(false)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_snapshot_diff(repo_path: String, hash: String) -> Result<String, String> {
    let output = std::process::Command::new("git")
        .args(["show", "--stat", "-p", &hash])
        .current_dir(&repo_path)
        .output()
        .map_err(|e| format!("Failed to execute git show: {e}"))?;
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[tauri::command]
pub fn restore_files(
    repo_path: String,
    hash: String,
    files: Option<Vec<String>>,
) -> Result<(), String> {
    let path = Path::new(&repo_path);
    snapshots::restore(path, &hash, files.as_deref()).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_metrics_summary(human_readable: bool) -> Result<String, String> {
    let log_path = RuntimeLock::get_endur_cache_home().join("endur.log");
    if !log_path.exists() {
        return Ok("No log file found.".to_string());
    }
    let mut file = std::fs::File::open(log_path).map_err(|e| e.to_string())?;
    let mut output = Vec::new();
    endur::metrics::get_snapshot_metrics(&mut file, &mut output, human_readable, true)
        .map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&output).into_owned())
}

#[tauri::command]
pub fn get_log_tail(lines: usize) -> Result<String, String> {
    let log_path = RuntimeLock::get_endur_cache_home().join("endur.log");
    if !log_path.exists() {
        return Ok("No logs recorded yet.".to_string());
    }
    let content = std::fs::read_to_string(log_path).map_err(|e| e.to_string())?;

    let mut formatted_lines = Vec::new();
    for line in content.lines().rev() {
        if let Some(formatted) = endur::tui::format_log_line(line) {
            formatted_lines.push(formatted);
            if formatted_lines.len() >= lines {
                break;
            }
        }
    }
    Ok(formatted_lines.join("\n"))
}

#[tauri::command]
pub fn get_snapshot_files(repo_path: String, hash: String) -> Result<Vec<(char, String)>, String> {
    let path = Path::new(&repo_path);
    snapshots::get_snapshot_files(path, &hash).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn prune_snapshots(
    repo_path: String,
    target_commit: Option<String>,
    keep_last_n: Option<usize>,
    before_duration: Option<String>,
    run_gc: bool,
) -> Result<String, String> {
    let path = Path::new(&repo_path);
    let options = endur::prune::PruneOptions {
        target_commit,
        keep_last_n,
        before_duration,
        dry_run: false,
        run_gc,
    };
    let report = endur::prune::prune(path, &options).map_err(|e| e.to_string())?;
    Ok(format!(
        "Successfully pruned {} snapshots.",
        report.pruned.len()
    ))
}

#[tauri::command]
pub fn is_service_installed() -> Result<bool, String> {
    Ok(service::is_installed())
}

#[tauri::command]
pub fn is_service_running() -> Result<bool, String> {
    service::is_running().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn control_service(action: String) -> Result<(), String> {
    match action.as_str() {
        "install" => service::install().map_err(|e| e.to_string()),
        "uninstall" => service::uninstall().map_err(|e| e.to_string()),
        "start" => service::start().map_err(|e| e.to_string()),
        "stop" => service::stop().map_err(|e| e.to_string()),
        _ => Err("Invalid service action".to_string()),
    }
}

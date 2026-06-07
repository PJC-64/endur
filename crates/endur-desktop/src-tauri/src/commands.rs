use std::path::{Path, PathBuf};
use serde::Serialize;
use endur::config::{Config, WatchConfig};
use endur::database::RuntimeLock;
use endur::poller;
use endur::snapshots::{self, SnapshotInfo};

#[derive(Serialize)]
pub struct DaemonStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub uptime_secs: Option<u64>,
}

#[tauri::command]
pub async fn get_daemon_status() -> Result<DaemonStatus, String> {
    let is_running = poller::send_uds_command("status").await.is_ok();
    let lock = RuntimeLock::load();
    let uptime_secs = if is_running && lock.pid.is_some() {
        lock.start_time.and_then(|t| t.elapsed().ok()).map(|d| d.as_secs())
    } else {
        None
    };
    Ok(DaemonStatus {
        running: is_running,
        pid: if is_running { lock.pid } else { None },
        uptime_secs,
    })
}

#[tauri::command]
pub async fn control_daemon(action: String) -> Result<(), String> {
    match action.as_str() {
        "start" => {
            let home_dir = dirs::home_dir().ok_or_else(|| "Could not determine home directory".to_string())?;
            let mut endur_path = home_dir.join(".cargo/bin/endur");
            if !endur_path.exists() {
                endur_path = PathBuf::from("endur");
            }
            let logfile_path = RuntimeLock::get_endur_cache_home().join("endur.log");
            let mut cmd = std::process::Command::new(endur_path);
            cmd.arg("serve")
                .arg("--logfile")
                .arg(logfile_path);
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
            cmd.spawn().map_err(|e| format!("Failed to spawn daemon: {e}"))?;
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
            config.set_watch(path.clone(), WatchConfig::default()).map_err(|e| e.to_string())?;
        } else {
            config.set_unwatch(path.clone()).map_err(|e| e.to_string())?;
        }
        config.save();
    }
    let _ = poller::send_uds_command("reload").await;
    Ok(())
}

#[tauri::command]
pub fn get_snapshots(repo_path: String) -> Result<Vec<SnapshotInfo>, String> {
    let path = Path::new(&repo_path);
    snapshots::list_snapshots(path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_snapshot_diff(repo_path: String, hash: String) -> Result<String, String> {
    let output = std::process::Command::new("git")
        .args(&["show", "--stat", "-p", &hash])
        .current_dir(&repo_path)
        .output()
        .map_err(|e| format!("Failed to execute git show: {e}"))?;
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[tauri::command]
pub fn restore_files(repo_path: String, hash: String, files: Option<Vec<String>>) -> Result<(), String> {
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
    endur::metrics::get_snapshot_metrics(&mut file, &mut output, human_readable)
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
    let tail: Vec<&str> = content.lines().rev().take(lines).collect();
    let tail_joined: Vec<String> = tail.into_iter().rev().map(|s| s.to_string()).collect();
    Ok(tail_joined.join("\n"))
}

#[tauri::command]
pub fn get_snapshot_files(repo_path: String, hash: String) -> Result<Vec<(char, String)>, String> {
    let path = Path::new(&repo_path);
    snapshots::get_snapshot_files(path, &hash).map_err(|e| e.to_string())
}

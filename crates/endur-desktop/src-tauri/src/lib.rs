mod commands;

use tauri::Emitter;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::get_daemon_status,
            commands::control_daemon,
            commands::get_watched_repositories,
            commands::toggle_watch_repo,
            commands::get_snapshots,
            commands::get_snapshot_diff,
            commands::restore_files,
            commands::get_metrics_summary,
            commands::get_log_tail,
            commands::get_snapshot_files
        ])
        .setup(|app| {
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut last_status = None;
                let mut last_log_tail = String::new();

                loop {
                    // Check status and emit event if changed
                    if let Ok(status) = commands::get_daemon_status().await {
                        let status_serialized = serde_json::to_value(&status).unwrap_or(serde_json::Value::Null);
                        if Some(&status_serialized) != last_status.as_ref() {
                            let _ = app_handle.emit("daemon-status", &status);
                            last_status = Some(status_serialized);
                        }
                    }

                    // Check logs and emit event if changed
                    if let Ok(logs) = commands::get_log_tail(45) {
                        if logs != last_log_tail {
                            let _ = app_handle.emit("daemon-logs", &logs);
                            last_log_tail = logs;
                        }
                    }

                    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

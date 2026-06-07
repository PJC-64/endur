mod commands;

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
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

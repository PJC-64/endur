pub mod cache;
pub mod config;
pub mod database;
pub mod log;
pub mod logger;
pub mod metrics;
pub mod poll_guard;
pub mod poller;
pub mod prune;
pub mod repo_status;
pub mod service;
pub mod snapshot_info;
pub mod snapshots;
pub mod tui;
pub mod watcher;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

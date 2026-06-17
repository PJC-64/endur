pub mod cache;
pub mod config;
pub mod database;
pub mod git_repo_iter;
pub mod log;
pub mod logger;
pub mod metrics;
pub mod poll_guard;
pub mod poller;
pub mod repo_status;
pub mod service;
pub mod snapshots;
pub mod prune;
pub mod tui;
pub mod watcher;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

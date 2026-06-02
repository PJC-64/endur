use crate::config::{Config, WatchConfig};
use git2::Repository;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct ChangedFile {
    pub path: String,
    pub status: git2::Status,
}

#[derive(Debug, Clone)]
pub struct RepoStatus {
    pub path: PathBuf,
    pub exists: bool,
    pub is_git_repo: bool,
    pub git_error: Option<String>,
    pub backup_count: usize,
    pub latest_commit_id: Option<String>,
    pub last_backup: Option<SystemTime>,
    pub changed_files: Vec<ChangedFile>,
    pub watch_config: Rc<WatchConfig>,
}

impl RepoStatus {
    pub fn from_path(path: &Path, watch_config: Rc<WatchConfig>, config: &Config) -> Self {
        let path_buf = path.to_path_buf();
        if !path.exists() {
            return Self {
                path: path_buf,
                exists: false,
                is_git_repo: false,
                git_error: None,
                backup_count: 0,
                latest_commit_id: None,
                last_backup: None,
                changed_files: Vec::new(),
                watch_config,
            };
        }

        match Repository::open(path) {
            Ok(repo) => {
                let mut changed_files = Vec::new();
                if let Ok(statuses) = repo.statuses(Some(
                    git2::StatusOptions::new()
                        .include_untracked(true)
                        .include_ignored(false)
                        .include_unmodified(false),
                )) {
                    for entry in statuses.iter() {
                        let status = entry.status();
                        if status.is_wt_new()
                            || status.is_wt_modified()
                            || status.is_wt_deleted()
                            || status.is_index_new()
                            || status.is_index_modified()
                            || status.is_index_deleted()
                        {
                            if let Some(p) = entry.path() {
                                changed_files.push(ChangedFile {
                                    path: p.to_string(),
                                    status,
                                });
                            }
                        }
                    }
                }

                let (backup_count, latest_commit_id, latest_time) = config.count_backups(&repo);
                let last_backup = if latest_time > 0 {
                    Some(
                        SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(latest_time as u64),
                    )
                } else {
                    None
                };

                Self {
                    path: path_buf,
                    exists: true,
                    is_git_repo: true,
                    git_error: None,
                    backup_count,
                    latest_commit_id,
                    last_backup,
                    changed_files,
                    watch_config,
                }
            }
            Err(e) => Self {
                path: path_buf,
                exists: true,
                is_git_repo: false,
                git_error: Some(e.to_string()),
                backup_count: 0,
                latest_commit_id: None,
                last_backup: None,
                changed_files: Vec::new(),
                watch_config,
            },
        }
    }

    pub fn has_uncommitted_changes(&self) -> bool {
        !self.changed_files.is_empty()
    }
}

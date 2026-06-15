use ignore::gitignore::{Gitignore, GitignoreBuilder};
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing::{error, info};

pub struct WatcherManager {
    watcher: RecommendedWatcher,
    gitignores: Arc<Mutex<HashMap<PathBuf, Gitignore>>>,
}

impl WatcherManager {
    pub fn new<F>(event_handler: F) -> Result<Self, notify::Error>
    where
        F: Fn(PathBuf, PathBuf) + Send + Sync + 'static,
    {
        let gitignores = Arc::new(Mutex::new(HashMap::<PathBuf, Gitignore>::new()));
        let gitignores_clone = Arc::clone(&gitignores);

        let watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                if event.kind.is_modify()
                    || event.kind.is_create()
                    || event.kind.is_remove()
                    || event.kind.is_other()
                {
                    // Check if any modified file is a .gitignore and reload the cache
                    for path in &event.paths {
                        if path.file_name().and_then(|f| f.to_str()) == Some(".gitignore") {
                            let canon_path = path.canonicalize().unwrap_or_else(|_| path.clone());
                            let mut map = gitignores_clone.lock().unwrap();
                            let repo_paths: Vec<PathBuf> = map.keys().cloned().collect();
                            for repo_path in repo_paths {
                                if canon_path.starts_with(&repo_path) {
                                    info!(
                                        "Dynamic reload of .gitignore detected for repo: {}",
                                        repo_path.display()
                                    );
                                    let mut builder = GitignoreBuilder::new(&repo_path);
                                    let gitignore_file = repo_path.join(".gitignore");
                                    if gitignore_file.exists() {
                                        builder.add(&gitignore_file);
                                    }
                                    let gitignore =
                                        builder.build().unwrap_or_else(|_| Gitignore::empty());
                                    map.insert(repo_path, gitignore);
                                }
                            }
                        }
                    }

                    let gitignores_map = gitignores_clone.lock().unwrap();
                    for path in event.paths {
                        let canon_path = path.canonicalize().unwrap_or_else(|_| path.clone());
                        for repo_path in gitignores_map.keys() {
                            if canon_path.starts_with(repo_path) {
                                // Skip files under .git
                                if canon_path.components().any(|c| c.as_os_str() == ".git") {
                                    continue;
                                }

                                // Check if ignored by gitignore
                                if let Some(gitignore) = gitignores_map.get(repo_path) {
                                    if let Ok(rel_path) = canon_path.strip_prefix(repo_path) {
                                        let is_dir = canon_path.is_dir();
                                        if gitignore.matched(rel_path, is_dir).is_ignore() {
                                            continue;
                                        }
                                    }
                                }

                                event_handler(repo_path.clone(), canon_path.clone());
                                break;
                            }
                        }
                    }
                }
            }
        })?;

        Ok(Self {
            watcher,
            gitignores,
        })
    }

    pub fn watch_repo(&mut self, repo_path: &Path) {
        let repo_path = repo_path.to_path_buf();
        info!("Starting file watch for repo: {}", repo_path.display());

        let mut builder = GitignoreBuilder::new(&repo_path);
        let gitignore_file = repo_path.join(".gitignore");
        if gitignore_file.exists() {
            builder.add(&gitignore_file);
        }
        let gitignore = builder.build().unwrap_or_else(|_| Gitignore::empty());

        self.gitignores
            .lock()
            .unwrap()
            .insert(repo_path.clone(), gitignore);

        if let Err(e) = self.watcher.watch(&repo_path, RecursiveMode::Recursive) {
            error!("Failed to watch path {}: {e}", repo_path.display());
        }
    }

    pub fn unwatch_repo(&mut self, repo_path: &Path) {
        let repo_path = repo_path.to_path_buf();
        info!("Stopping file watch for repo: {}", repo_path.display());
        self.gitignores.lock().unwrap().remove(&repo_path);
        let _ = self.watcher.unwatch(&repo_path);
    }
}

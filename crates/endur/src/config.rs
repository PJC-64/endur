use chrono::{DateTime, Local};
use git2::Repository;
use std::collections::BTreeMap;
use std::fs::{create_dir_all, File};
use std::io::IsTerminal;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::SystemTime;
use std::{env, fs};

use serde::{Deserialize, Serialize};

use crate::database::RuntimeLock;
use crate::git_repo_iter::GitRepoIter;
use crate::repo_status::RepoStatus;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct WatchConfig {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub max_depth: u8,
}

impl WatchConfig {
    pub fn new() -> Self {
        Self {
            include: vec![],
            exclude: vec![],
            max_depth: 255,
        }
    }
}

impl Default for WatchConfig {
    fn default() -> Self {
        WatchConfig::new()
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    // When commit_exclude_git_config is true,
    // never use any git configuration to sign endur's commits.
    // Defaults to false
    #[serde(default)]
    pub commit_exclude_git_config: bool,
    pub commit_author: Option<String>,
    pub commit_email: Option<String>,
    #[serde(default)]
    pub base_root: Option<String>,
    pub repos: BTreeMap<String, Rc<WatchConfig>>,
}

impl Config {
    const SYMBOLS_FANCY: [&'static str; 8] = ["✓", "📝", "❌", "⚠️", "ℹ️", "🕒", "📊", "📁"];
    const SYMBOLS_PLAIN: [&'static str; 8] = ["[OK]", "[M]", "[X]", "!", "i", "@", "#", "*"];

    fn get_symbols() -> &'static [&'static str; 8] {
        // Check environment variable first (explicit override)
        if std::env::var("ENDUR_PLAIN_TEXT").is_ok() {
            return &Self::SYMBOLS_PLAIN;
        }

        // Check if ENDUR_FANCY is set (explicit override)
        if std::env::var("ENDUR_FANCY").is_ok() {
            return &Self::SYMBOLS_FANCY;
        }

        // Auto-detect terminal capabilities
        if !std::io::stdout().is_terminal() {
            // Not a terminal (e.g., pipe or redirect)
            return &Self::SYMBOLS_PLAIN;
        }

        // Check for NO_COLOR (standard for disabling color/unicode)
        if std::env::var("NO_COLOR").is_ok() {
            return &Self::SYMBOLS_PLAIN;
        }

        // Check TERM environment variable
        if let Ok(term) = std::env::var("TERM") {
            let term = term.to_lowercase();
            if term == "dumb" || term == "vt100" || term.contains("linux") {
                return &Self::SYMBOLS_PLAIN;
            }
        }

        // Default to fancy if we couldn't determine otherwise
        // Most modern terminals support Unicode
        &Self::SYMBOLS_FANCY
    }

    pub fn empty() -> Self {
        Self {
            commit_exclude_git_config: false,
            commit_author: None,
            commit_email: None,
            base_root: None,
            repos: BTreeMap::new(),
        }
    }

    pub fn default_path() -> Result<PathBuf> {
        Ok(Self::get_endur_config_home()?.join("config.toml"))
    }

    /// Location of all config. By default
    ///
    /// Linux   :   $XDG_CONFIG_HOME/endur or $HOME/.config/endur
    /// macOS   :   $HOME/Library/Application Support
    /// Windows :   %AppData%\Roaming\endur
    ///
    /// This can be overridden by setting ENDUR_CONFIG_HOME environment variable.
    fn get_endur_config_home() -> Result<PathBuf> {
        // The environment variable lets us run tests independently, but I'm sure someone will come
        // up with another reason to use it.
        if let Ok(env_var) = env::var("ENDUR_CONFIG_HOME") {
            if !env_var.is_empty() {
                return Ok(env_var.into());
            }
        }

        dirs::config_dir()
            .map(|dir| dir.join("endur"))
            .ok_or_else(|| "Could not find your config directory. The default is ~/.config/endur but it can also be controlled by setting the ENDUR_CONFIG_HOME environment variable.".into())
    }

    /// Load Config from default path
    pub fn load() -> Self {
        match Self::default_path() {
            Ok(path) => Self::load_file(&path).unwrap_or_else(|_| Self::empty()),
            Err(_) => Self::empty(),
        }
    }

    pub fn load_file(path: &Path) -> Result<Self> {
        let mut reader = BufReader::new(File::open(path)?);

        let mut buffer = Vec::new();
        reader.read_to_end(&mut buffer)?;

        let res = toml::from_slice(buffer.as_slice())?;
        Ok(res)
    }

    /// Save config to disk in ~/.config/endur/config.toml
    pub fn save(&self) {
        match Self::default_path() {
            Ok(path) => self.save_to_path(&path),
            Err(e) => eprintln!("Error getting default path: {e}"),
        }
    }

    pub fn create_dir(path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            create_dir_all(dir)
                .map_err(|e| format!("Failed to create directory at `{}`: {}. \
                    Endur stores its configuration in `{}/config.toml`, \
                    where you can instruct endur to watch patterns of Git repositories, among other things. \
                    See https://github.com/PJC-64/endur for more information.", dir.display(), e, path.display()).into())
        } else {
            Ok(())
        }
    }

    /// Attempts to create parent dirs, serialize `self` as TOML and write to disk.
    pub fn save_to_path(&self, path: &Path) {
        if let Err(e) = Self::create_dir(path) {
            eprintln!("{e}");
            return;
        }

        let config_string = match toml::to_string(self) {
            Ok(v) => v,
            Err(e) => {
                println!("Unexpected error when deserializing config: {e}");
                return;
            }
        };

        match fs::write(path, config_string) {
            Ok(_) => (),
            Err(e) => println!("Unable to initialize endur config file: {e}"),
        }
    }

    pub fn set_watch(&mut self, path: String, cfg: WatchConfig) -> Result<()> {
        let abs_path = fs::canonicalize(&path)
            .map_err(|e| format!("The provided path '{path}' is not a directory: {e}"))?;
        let abs_path_str = abs_path
            .to_str()
            .ok_or("The provided path is not valid unicode")?;

        if self.repos.contains_key(abs_path_str) {
            Err(format!("{abs_path_str} is already being watched").into())
        } else {
            self.repos.insert(abs_path_str.to_string(), Rc::new(cfg));
            println!("Started watching {abs_path_str}");
            Ok(())
        }
    }

    pub fn set_unwatch(&mut self, path: String) -> Result<()> {
        let abs_path = fs::canonicalize(&path)
            .map_err(|e| format!("The provided path '{path}' is not a directory: {e}"))?;
        let abs_path_str = abs_path
            .to_str()
            .ok_or("The provided path is not valid unicode")?;

        match self.repos.remove(abs_path_str) {
            Some(_) => {
                println!("Stopped watching {abs_path_str}");
                Ok(())
            }
            None => Err(format!("{abs_path_str} is not being watched").into()),
        }
    }

    pub fn git_repos(&self) -> GitRepoIter<'_> {
        GitRepoIter::new(self)
    }

    pub fn count_backups(&self, repo: &Repository) -> (usize, Option<String>, i64) {
        let mut backup_count = 0;
        let mut latest_commit_id = None;
        let mut latest_time = 0;

        let head = match repo.head().ok().and_then(|h| h.peel_to_commit().ok()) {
            Some(h) => h,
            None => return (0, None, 0),
        };

        let branch_name = format!("endur/{}", head.id());
        let branch = match repo.find_branch(&branch_name, git2::BranchType::Local) {
            Ok(b) => b,
            Err(_) => return (0, None, 0),
        };

        let branch_commit = match branch.get().peel_to_commit() {
            Ok(c) => c,
            Err(_) => return (0, None, 0),
        };

        let mut revwalk = match repo.revwalk() {
            Ok(rw) => rw,
            Err(_) => return (0, None, 0),
        };

        if revwalk.push(branch_commit.id()).is_err() {
            return (0, None, 0);
        }
        if revwalk.hide(head.id()).is_err() {
            return (0, None, 0);
        }

        for oid in revwalk.flatten() {
            if let Ok(commit) = repo.find_commit(oid) {
                if let Some(message) = commit.message() {
                    if message.ends_with("endur auto-backup") {
                        backup_count += 1;
                        let commit_time = commit.time().seconds();
                        if commit_time > latest_time {
                            latest_time = commit_time;
                            latest_commit_id = Some(oid.to_string());
                        }
                    }
                }
            }
        }

        (backup_count, latest_commit_id, latest_time)
    }

    pub fn print_summary(&self) {
        let symbols = Self::get_symbols();
        let [ok, modified, error, _warning, _info, _time, _stats, _folder] = symbols;

        println!("Endur Status Summary");
        println!("-------------------");

        // Add server status at the top
        if RuntimeLock::is_active() {
            let runtime_lock = RuntimeLock::load();
            match runtime_lock.pid {
                Some(pid) => {
                    let uptime = runtime_lock
                        .start_time
                        .and_then(|start| SystemTime::now().duration_since(start).ok())
                        .map(|duration| {
                            let days = duration.as_secs() / 86400;
                            let hours = (duration.as_secs() % 86400) / 3600;
                            let minutes = (duration.as_secs() % 3600) / 60;
                            if days > 0 {
                                format!("{days}d {hours}h")
                            } else if hours > 0 {
                                format!("{hours}h {minutes}m")
                            } else {
                                format!("{minutes}m")
                            }
                        })
                        .unwrap_or_else(|| "unknown time".to_string());
                    println!("Server: Running (PID: {pid}, Uptime: {uptime})");
                }
                None => println!("Server: Running (PID: unknown, Uptime: unknown)"),
            }
        } else {
            println!("Server: Not running");
        }
        println!();

        let total_repos = self.repos.len();
        let mut total_backups = 0;
        let mut repos_with_changes = 0;
        let mut inaccessible_repos = 0;

        for (path, config) in &self.repos {
            let status = RepoStatus::from_path(Path::new(path), Rc::clone(config), self);
            if !status.exists || !status.is_git_repo {
                inaccessible_repos += 1;
                if !status.exists {
                    println!("{error} {}: Not found", status.path.display());
                } else {
                    println!("{error} {}: Not a git repository", status.path.display());
                }
                continue;
            }

            let has_changes = status.has_uncommitted_changes();
            if has_changes {
                repos_with_changes += 1;
            }

            total_backups += status.backup_count;

            let commit_info = status
                .latest_commit_id
                .as_ref()
                .map(|id| format!(" [{}]", &id[..7]))
                .unwrap_or_default();

            let time_info = if let Some(last_backup) = status.last_backup {
                let datetime: DateTime<Local> = last_backup.into();
                format!(" @ {}", datetime.format("%Y%m%d-%H%M%S"))
            } else {
                String::new()
            };

            println!(
                "{}{}: {} backups{}{}{}",
                if has_changes { modified } else { ok },
                status.path.display(),
                status.backup_count,
                commit_info,
                time_info,
                if has_changes {
                    " (uncommitted changes)"
                } else {
                    ""
                }
            );
        }

        println!("\nOverall Status:");
        println!(
            "Watching {total_repos} repositories ({} accessible)",
            total_repos - inaccessible_repos
        );
        println!("Total backups: {total_backups}");
        if repos_with_changes > 0 {
            println!("Repositories with uncommitted changes: {repos_with_changes}");
        }
        if inaccessible_repos > 0 {
            println!("Inaccessible repositories: {inaccessible_repos}");
        }
    }

    pub fn print_detailed_info(&self) {
        let symbols = Self::get_symbols();
        let [ok, modified, error, warning, info, time, stats, folder] = symbols;

        for (path, config) in &self.repos {
            let status = RepoStatus::from_path(Path::new(path), Rc::clone(config), self);
            println!("{folder} {}", status.path.display());

            if !status.exists {
                println!("  {error} Path does not exist");
                continue;
            }

            if !status.is_git_repo {
                let err_msg = status
                    .git_error
                    .as_deref()
                    .unwrap_or("Path is not a git repository");
                println!("  {error} Not a valid git repository: {err_msg}\n");
                continue;
            }

            println!("  {ok} Valid Git repository");

            for file in &status.changed_files {
                println!(
                    "  {modified} Change detected: {} ({:?})",
                    file.path, file.status
                );
            }

            if status.has_uncommitted_changes() {
                println!("  {warning} Has uncommitted changes");
            } else {
                println!("  {ok} No uncommitted changes");
            }

            if status.backup_count > 0 {
                if let Some(id) = &status.latest_commit_id {
                    if let Some(last_backup) = status.last_backup {
                        let datetime: DateTime<Local> = last_backup.into();
                        println!(
                            "  {time} Last backup: {} ({})",
                            datetime.format("%Y-%m-%d %H:%M:%S"),
                            &id[..7]
                        );
                    }
                }
                println!("  {stats} Total backups: {}", status.backup_count);
            } else {
                println!("  {info} No backups found");
            }

            // Print watch configuration
            println!("  Watch Configuration:");
            if status.watch_config.include.is_empty() {
                println!("    Include: All files");
            } else {
                println!("    Include: {:?}", status.watch_config.include);
            }
            println!("    Max depth: {}\n", status.watch_config.max_depth);
        }
    }
}

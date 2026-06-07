use std::{
    collections::HashSet,
    ops, path,
    process::{Command, Stdio},
    thread, time,
};

use crate::util::daemon::Daemon;
use endur::config::Config;
use endur::database::RuntimeLock;

/// Utility to start endur asynchronously (e.g. endur serve) and kill the process when this goes out
/// of scope. This helps us do end-to-end tests where we invoke the executable, possibly multiple
/// different processes.
pub struct Endur {
    pub primary: Option<Daemon>,
    pub secondary: Option<Daemon>,
    config_dir: tempfile::TempDir,
    cache_dir: tempfile::TempDir,
}

impl Endur {
    pub fn new() -> Self {
        Self {
            primary: None,
            secondary: None,
            config_dir: tempfile::tempdir().unwrap(),
            cache_dir: tempfile::tempdir().unwrap(),
        }
    }

    pub fn start_async(&mut self, args: &[&str], is_primary: bool) {
        println!("$ endur {} &", args.join(" "));
        let exe = env!("CARGO_BIN_EXE_endur").to_string();
        let child = Command::new(exe)
            .args(args)
            .env("ENDUR_CONFIG_HOME", self.config_dir.path())
            .env("ENDUR_CACHE_HOME", self.cache_dir.path())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        let log_path = self.cache_dir.path().join("endur.log");

        if is_primary {
            self.primary = Some(Daemon::new(child, log_path));
        } else {
            self.secondary = Some(Daemon::new(child, log_path));
        }
    }

    pub fn run(&self, args: &[&str]) {
        println!("$ endur {}", args.join(" "));
        let exe = env!("CARGO_BIN_EXE_endur").to_string();
        let child_proc = Command::new(exe)
            .args(args)
            .env("ENDUR_CONFIG_HOME", self.config_dir.path())
            .env("ENDUR_CACHE_HOME", self.cache_dir.path())
            .output();

        if let Ok(output) = child_proc {
            if !output.status.success() {
                // This cleans up test development by causing us to fail earlier
                return;
            }
            let text = String::from_utf8(output.stdout).unwrap();
            if !text.is_empty() {
                println!("{text}");
            }
            let err = String::from_utf8(output.stderr).unwrap();
            if !err.is_empty() {
                println!("{err}");
            }
        }
    }

    pub fn run_in_dir(&self, args: &[&str], dir: &path::Path) {
        println!("$ endur {}", args.join(" "));
        let exe = env!("CARGO_BIN_EXE_endur").to_string();
        let child_proc = Command::new(exe)
            .args(args)
            .env("ENDUR_CONFIG_HOME", self.config_dir.path())
            .env("ENDUR_CACHE_HOME", self.cache_dir.path())
            .current_dir(dir)
            .output();

        if let Ok(output) = child_proc {
            if !output.status.success() {
                // This cleans up test development by causing us to fail earlier
                return;
            }
            let text = String::from_utf8(output.stdout).unwrap();
            if !text.is_empty() {
                println!("{text}");
            }
            let err = String::from_utf8(output.stderr).unwrap();
            if !err.is_empty() {
                println!("{err}");
            }
        }
    }

    pub fn pid(&self, is_primary: bool) -> Option<u32> {
        if is_primary {
            self.primary.as_ref().map(|d| d.child.id())
        } else {
            self.secondary.as_ref().map(|d| d.child.id())
        }
    }

    pub fn config_path(&self) -> path::PathBuf {
        self.config_dir.path().join("config.toml")
    }

    pub fn get_config(&self) -> Option<Config> {
        println!("$ cat ~/.config/endur/config.toml");
        let cfg = Config::load_file(self.config_path().as_path()).ok();
        println!("{cfg:?}");
        cfg
    }

    pub fn save_config(&self, cfg: &Config) {
        cfg.save_to_path(self.config_path().as_path());
    }

    pub fn runtime_lock_path(&self) -> path::PathBuf {
        self.cache_dir.path().join("runtime.db")
    }

    pub fn get_runtime_lock(&self) -> Option<RuntimeLock> {
        println!("$ cat ~/.cache/endur/runtime.db");
        let cfg = RuntimeLock::load_file(self.runtime_lock_path().as_path());
        cfg.ok()
    }

    pub fn save_runtime_lock(&self, cfg: &RuntimeLock) {
        cfg.save_to_path(self.runtime_lock_path().as_path());
    }

    pub fn git_repos(&self) -> HashSet<path::PathBuf> {
        match self.get_config() {
            Some(cfg) => cfg.git_repos().collect(),
            None => HashSet::new(),
        }
    }

    pub fn wait(&self) {
        // This hack isn't going to work. Another idea is to read lines
        // from stdout as a signal to proceed.
        thread::sleep(time::Duration::from_secs(6));
    }
}

impl ops::Drop for Endur {
    /// Force Kill. Not the "kind kill" that is `endur kill`
    ///
    /// Ensure the process is stopped. Each test gets a unique config file path, so processes
    /// should stay independent and isolated as long as no one is running `ps aux`
    fn drop(&mut self) {
        // don't handle kill errors.
        let _ = self.primary.as_mut().map(|d| d.child.kill());
        let _ = self.secondary.as_mut().map(|d| d.child.kill());
    }
}

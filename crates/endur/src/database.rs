use fs2::FileExt;
use std::fs::{create_dir_all, File, OpenOptions};
use std::io::{Result, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use std::{env, fs, io};

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeLock {
    pub pid: Option<u32>,
    pub start_time: Option<SystemTime>,
}

impl RuntimeLock {
    pub fn empty() -> Self {
        Self {
            pid: None,
            start_time: None,
        }
    }

    pub fn default_path() -> PathBuf {
        Self::get_endur_cache_home().join("runtime.db")
    }

    pub fn lock_path() -> PathBuf {
        Self::get_endur_cache_home().join("runtime.lock")
    }

    /// Location of all database files. By default
    ///
    /// Linux   :   $XDG_CACHE_HOME/endur or $HOME/.cache/endur
    /// macOS   :   $HOME/Library/Caches
    /// Windows :   %AppData%\Local\endur
    ///
    /// This can be overridden by setting ENDUR_CACHE_HOME environment variable.
    pub fn get_endur_cache_home() -> PathBuf {
        if let Ok(env_var) = env::var("ENDUR_CACHE_HOME") {
            if !env_var.is_empty() {
                return env_var.into();
            }
        }

        dirs::cache_dir()
            .expect("Could not find your cache directory. The default is ~/.cache/endur but it can also \
                be controlled by setting the ENDUR_CACHE_HOME environment variable.")
            .join("endur")
    }

    /// Load Config from default path
    pub fn load() -> Self {
        Self::load_file(Self::default_path().as_path()).unwrap_or_else(|_| Self::empty())
    }

    pub fn load_file(path: &Path) -> Result<Self> {
        let reader = io::BufReader::new(File::open(path)?);
        let res = serde_json::from_reader(reader)?;
        Ok(res)
    }

    /// Tries to acquire an exclusive lock on the runtime lock file.
    /// If successful, returns the locked File handle.
    pub fn acquire_exclusive() -> Result<File> {
        let path = Self::lock_path();
        Self::create_dir(&path);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)?;
        file.try_lock_exclusive()?;
        Ok(file)
    }

    /// Checks if the daemon is currently running by attempting a shared lock.
    pub fn is_active() -> bool {
        let path = Self::lock_path();
        if !path.exists() {
            return false;
        }
        let file = match File::open(&path) {
            Ok(f) => f,
            Err(_) => {
                #[cfg(target_os = "windows")]
                return true;
                #[cfg(not(target_os = "windows"))]
                return false;
            }
        };
        file.try_lock_shared().is_err()
    }

    /// Write lock metadata into the active locked file
    pub fn write_metadata(&self) -> Result<()> {
        let path = Self::default_path();
        Self::create_dir(&path);
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)?;
        let json = serde_json::to_string(self).unwrap();
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
        Ok(())
    }

    pub fn create_dir(path: &Path) {
        if let Some(dir) = path.parent() {
            create_dir_all(dir).unwrap_or_else(|_| {
                panic!(
                    "Failed to create directory at `{}`.\
                    Endur stores its runtime cache in `{}/runtime.db`. \
                    See https://github.com/PJC-64/endur for more information.",
                    dir.display(),
                    path.display()
                )
            })
        }
    }

    /// Save lock state directly to default path
    pub fn save(&self) {
        self.save_to_path(Self::default_path().as_path())
    }

    /// Attempts to create parent dirs, serialize `self` as JSON and write to disk.
    pub fn save_to_path(&self, path: &Path) {
        Self::create_dir(path);
        let json = serde_json::to_string(self).unwrap();
        let _ = fs::write(path, json);
    }
}

use std::io::{BufRead, BufReader};
use std::process::Child;
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Main-thread side of a process watcher. The process that's launched is exposed as messages
/// (per-line) over a mpsc channel. This is intended to simplify, speed up, and generally make the
/// tests more reliable when they dispatch asynchronously to `dura serve`. However, nothing abot
/// this is intended to be specific to dura.
pub struct Daemon {
    mailbox: Receiver<Option<String>>,
    pub child: Child,
    /// Signals to kill daemon thread if this goes <= 0, like a CountDownLatch
    kill_sign: Arc<Mutex<i32>>,
}

impl Daemon {
    pub fn new(child: Child, log_path: std::path::PathBuf) -> Self {
        let kill_sign = Arc::new(Mutex::new(1));
        Self {
            mailbox: Self::attach(log_path, Arc::clone(&kill_sign)),
            child,
            kill_sign,
        }
    }

    /// Spawn another thread to watch the daemon log file. It tails the log file and sends each line
    /// over the channel.
    fn attach(
        log_path: std::path::PathBuf,
        kill_sign: Arc<Mutex<i32>>,
    ) -> Receiver<Option<String>> {
        fn is_ignored(msg: &str) -> bool {
            msg.contains("Started serving with dura")
        }
        let (sender, receiver) = channel();
        thread::spawn(move || {
            let mut file_opt = None;
            for _ in 0..100 {
                // 5 seconds max wait
                if *kill_sign.lock().unwrap() <= 0 {
                    return;
                }
                if log_path.exists() {
                    if let Ok(file) = std::fs::File::open(&log_path) {
                        file_opt = Some(file);
                        break;
                    }
                }
                thread::sleep(Duration::from_millis(50));
            }

            let file = match file_opt {
                Some(f) => f,
                None => {
                    let _ = sender.send(None);
                    return;
                }
            };

            let mut reader = BufReader::new(file);
            loop {
                {
                    if *kill_sign.lock().unwrap() <= 0 {
                        break;
                    }
                }
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) => {
                        thread::sleep(Duration::from_millis(50));
                    }
                    Ok(_) => {
                        if !line.is_empty()
                            && !is_ignored(line.as_str())
                            && sender.send(Some(line)).is_err()
                        {
                            break;
                        }
                    }
                    Err(e) => {
                        eprintln!("Error in daemon log watcher: {e:?}");
                        let _ = sender.send(None);
                        break;
                    }
                }
            }
        });
        receiver
    }

    /// Read a line from the log file channel, waiting at most timeout_secs.
    pub fn read_line(&self, timeout_secs: u64) -> Option<String> {
        self.mailbox
            .recv_timeout(Duration::from_secs(timeout_secs))
            .unwrap()
    }

    pub fn kill(&mut self) {
        let mut kill_sign = self.kill_sign.lock().unwrap();
        *kill_sign -= 1;
        self.child.kill().unwrap();
    }
}

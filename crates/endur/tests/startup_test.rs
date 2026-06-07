mod util;

use endur::config::Config;
use endur::database::RuntimeLock;
use std::fs;

/// How many seconds to wait, at most, for endur to start?
const START_TIMEOUT: u64 = 8;

#[test]
fn start_serve() {
    let mut endur = util::endur::Endur::new();
    assert_eq!(None, endur.pid(true));
    assert_eq!(None, endur.get_runtime_lock());

    endur.start_async(&["serve"], true);
    endur
        .primary
        .as_ref()
        .map(|d| d.read_line(START_TIMEOUT).unwrap());

    assert_ne!(None, endur.pid(true));
    let runtime_lock = endur.get_runtime_lock();
    assert_ne!(None, runtime_lock);
    assert_eq!(endur.pid(true), runtime_lock.unwrap().pid);
}

#[test]
fn start_serve_with_null_pid_in_config() {
    let mut endur = util::endur::Endur::new();
    let mut runtime_lock = RuntimeLock::empty();
    runtime_lock.pid = None;
    endur.save_runtime_lock(&runtime_lock);

    assert_eq!(None, endur.pid(true));
    assert_ne!(None, endur.get_runtime_lock());

    endur.start_async(&["serve"], true);
    endur
        .primary
        .as_ref()
        .map(|d| d.read_line(START_TIMEOUT).unwrap());

    assert_ne!(None, endur.pid(true));
    let runtime_lock = endur.get_runtime_lock();
    assert_ne!(None, runtime_lock);
    assert_eq!(endur.pid(true), runtime_lock.unwrap().pid);
}

#[test]
fn start_serve_with_other_pid_in_config() {
    let mut endur = util::endur::Endur::new();
    let mut runtime_lock = RuntimeLock::empty();
    runtime_lock.pid = Some(12345);
    endur.save_runtime_lock(&runtime_lock);

    println!("db:: {:?}", endur.get_runtime_lock());

    assert_eq!(None, endur.pid(true));
    assert_ne!(None, endur.get_runtime_lock());

    endur.start_async(&["serve"], true);
    endur
        .primary
        .as_ref()
        .map(|d| d.read_line(START_TIMEOUT).unwrap());

    assert_ne!(None, endur.pid(true));
    let runtime_lock = endur.get_runtime_lock();
    assert_ne!(None, runtime_lock);
    assert_eq!(endur.pid(true), runtime_lock.unwrap().pid);
}

#[test]
fn start_serve_with_invalid_json() {
    let mut endur = util::endur::Endur::new();
    let runtime_lock_path = endur.runtime_lock_path();
    let _ = Config::create_dir(runtime_lock_path.as_path());
    fs::write(runtime_lock_path, "{\"pid\":34725").unwrap();

    assert_eq!(None, endur.pid(true));
    assert_eq!(None, endur.get_runtime_lock());

    endur.start_async(&["serve"], true);
    endur
        .primary
        .as_ref()
        .map(|d| d.read_line(START_TIMEOUT).unwrap());

    assert_ne!(None, endur.pid(true));
    let runtime_lock = endur.get_runtime_lock();
    assert_ne!(None, runtime_lock);
    assert_eq!(endur.pid(true), runtime_lock.unwrap().pid);
}

#[test]
fn double_lock_prevention() {
    let mut endur = util::endur::Endur::new();
    // Start primary daemon
    endur.start_async(&["serve"], true);
    endur
        .primary
        .as_ref()
        .map(|d| d.read_line(START_TIMEOUT).unwrap());

    // Check that it's running
    assert_ne!(None, endur.pid(true));

    // Try to start a secondary daemon in the same cache directory
    endur.start_async(&["serve"], false);

    // The secondary daemon should exit immediately because the lock is held
    std::thread::sleep(std::time::Duration::from_millis(1000));

    if let Some(ref mut secondary) = endur.secondary {
        let status = secondary.child.try_wait().unwrap();
        assert!(
            status.is_some(),
            "Secondary daemon did not exit on double lock"
        );
        let exit_code = status.unwrap().code();
        assert_eq!(exit_code, Some(1));
    } else {
        panic!("Secondary daemon failed to spawn");
    }
}

#[cfg(any(unix, windows))]
#[tokio::test]
async fn test_uds_communication() {
    let mut endur = util::endur::Endur::new();
    endur.start_async(&["serve"], true);

    // Wait for the daemon to start and create the socket
    std::thread::sleep(std::time::Duration::from_millis(1500));

    // Override the environment variable so our client finds the test cache directory
    std::env::set_var(
        "ENDUR_CACHE_HOME",
        endur.runtime_lock_path().parent().unwrap(),
    );

    // Try sending a status command
    let res = endur::poller::send_uds_command("status").await;
    assert!(res.is_ok(), "Failed to send UDS command: {:?}", res.err());
    let res_json: serde_json::Value = serde_json::from_str(&res.unwrap()).unwrap();
    assert_eq!(res_json["status"], "ok");

    // Try sending a reload command
    let res = endur::poller::send_uds_command("reload").await;
    assert!(res.is_ok());

    // Clean up env var
    std::env::remove_var("ENDUR_CACHE_HOME");
}

#[cfg(unix)]
#[test]
fn test_graceful_shutdown_on_sigterm() {
    let mut endur = util::endur::Endur::new();
    assert_eq!(None, endur.pid(true));

    endur.start_async(&["serve"], true);
    endur
        .primary
        .as_ref()
        .map(|d| d.read_line(START_TIMEOUT).unwrap());

    let pid = endur.pid(true).expect("Daemon should be running");

    // Check that lock is active and has our PID
    let runtime_lock = endur.get_runtime_lock();
    assert!(runtime_lock.is_some());
    assert_eq!(runtime_lock.unwrap().pid, Some(pid));

    // Send SIGTERM to the process
    let status = std::process::Command::new("kill")
        .args(["-s", "TERM", &pid.to_string()])
        .status()
        .expect("failed to execute kill");
    assert!(status.success());

    // Wait for the process to exit
    let mut exited = false;
    for _ in 0..50 {
        if let Some(ref mut daemon) = endur.primary {
            if let Ok(Some(exit_status)) = daemon.child.try_wait() {
                // Should exit with status success (0)
                assert_eq!(exit_status.code(), Some(0));
                exited = true;
                break;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert!(exited, "Daemon did not exit in time after SIGTERM");

    // Check that the lock file has been updated to clear the PID
    let runtime_lock = endur.get_runtime_lock();
    assert!(runtime_lock.is_none() || runtime_lock.unwrap().pid.is_none());
}

#[cfg(unix)]
#[test]
fn test_graceful_shutdown_on_sigint() {
    let mut endur = util::endur::Endur::new();
    assert_eq!(None, endur.pid(true));

    endur.start_async(&["serve"], true);
    endur
        .primary
        .as_ref()
        .map(|d| d.read_line(START_TIMEOUT).unwrap());

    let pid = endur.pid(true).expect("Daemon should be running");

    // Send SIGINT to the process
    let status = std::process::Command::new("kill")
        .args(["-s", "INT", &pid.to_string()])
        .status()
        .expect("failed to execute kill");
    assert!(status.success());

    // Wait for the process to exit
    let mut exited = false;
    for _ in 0..50 {
        if let Some(ref mut daemon) = endur.primary {
            if let Ok(Some(exit_status)) = daemon.child.try_wait() {
                // Should exit with status success (0)
                assert_eq!(exit_status.code(), Some(0));
                exited = true;
                break;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert!(exited, "Daemon did not exit in time after SIGINT");

    // Check that the lock file has been updated to clear the PID
    let runtime_lock = endur.get_runtime_lock();
    assert!(runtime_lock.is_none() || runtime_lock.unwrap().pid.is_none());
}

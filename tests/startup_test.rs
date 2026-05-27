mod util;

use dura::config::Config;
use dura::database::RuntimeLock;
use std::fs;

/// How many seconds to wait, at most, for dura to start?
const START_TIMEOUT: u64 = 8;

#[test]
fn start_serve() {
    let mut dura = util::dura::Dura::new();
    assert_eq!(None, dura.pid(true));
    assert_eq!(None, dura.get_runtime_lock());

    dura.start_async(&["serve"], true);
    dura.primary
        .as_ref()
        .map(|d| d.read_line(START_TIMEOUT).unwrap());

    assert_ne!(None, dura.pid(true));
    let runtime_lock = dura.get_runtime_lock();
    assert_ne!(None, runtime_lock);
    assert_eq!(dura.pid(true), runtime_lock.unwrap().pid);
}

#[test]
fn start_serve_with_null_pid_in_config() {
    let mut dura = util::dura::Dura::new();
    let mut runtime_lock = RuntimeLock::empty();
    runtime_lock.pid = None;
    dura.save_runtime_lock(&runtime_lock);

    assert_eq!(None, dura.pid(true));
    assert_ne!(None, dura.get_runtime_lock());

    dura.start_async(&["serve"], true);
    dura.primary
        .as_ref()
        .map(|d| d.read_line(START_TIMEOUT).unwrap());

    assert_ne!(None, dura.pid(true));
    let runtime_lock = dura.get_runtime_lock();
    assert_ne!(None, runtime_lock);
    assert_eq!(dura.pid(true), runtime_lock.unwrap().pid);
}

#[test]
fn start_serve_with_other_pid_in_config() {
    let mut dura = util::dura::Dura::new();
    let mut runtime_lock = RuntimeLock::empty();
    runtime_lock.pid = Some(12345);
    dura.save_runtime_lock(&runtime_lock);

    println!("db:: {:?}", dura.get_runtime_lock());

    assert_eq!(None, dura.pid(true));
    assert_ne!(None, dura.get_runtime_lock());

    dura.start_async(&["serve"], true);
    dura.primary
        .as_ref()
        .map(|d| d.read_line(START_TIMEOUT).unwrap());

    assert_ne!(None, dura.pid(true));
    let runtime_lock = dura.get_runtime_lock();
    assert_ne!(None, runtime_lock);
    assert_eq!(dura.pid(true), runtime_lock.unwrap().pid);
}

#[test]
fn start_serve_with_invalid_json() {
    let mut dura = util::dura::Dura::new();
    let runtime_lock_path = dura.runtime_lock_path();
    let _ = Config::create_dir(runtime_lock_path.as_path());
    fs::write(runtime_lock_path, "{\"pid\":34725").unwrap();

    assert_eq!(None, dura.pid(true));
    assert_eq!(None, dura.get_runtime_lock());

    dura.start_async(&["serve"], true);
    dura.primary
        .as_ref()
        .map(|d| d.read_line(START_TIMEOUT).unwrap());

    assert_ne!(None, dura.pid(true));
    let runtime_lock = dura.get_runtime_lock();
    assert_ne!(None, runtime_lock);
    assert_eq!(dura.pid(true), runtime_lock.unwrap().pid);
}

#[test]
fn double_lock_prevention() {
    let mut dura = util::dura::Dura::new();
    // Start primary daemon
    dura.start_async(&["serve"], true);
    dura.primary
        .as_ref()
        .map(|d| d.read_line(START_TIMEOUT).unwrap());

    // Check that it's running
    assert_ne!(None, dura.pid(true));

    // Try to start a secondary daemon in the same cache directory
    dura.start_async(&["serve"], false);

    // The secondary daemon should exit immediately because the lock is held
    std::thread::sleep(std::time::Duration::from_millis(1000));
    
    if let Some(ref mut secondary) = dura.secondary {
        let status = secondary.child.try_wait().unwrap();
        assert!(status.is_some(), "Secondary daemon did not exit on double lock");
        let exit_code = status.unwrap().code();
        assert_eq!(exit_code, Some(1));
    } else {
        panic!("Secondary daemon failed to spawn");
    }
}

#[tokio::test]
async fn test_uds_communication() {
    let mut dura = util::dura::Dura::new();
    dura.start_async(&["serve"], true);
    
    // Wait for the daemon to start and create the socket
    std::thread::sleep(std::time::Duration::from_millis(1500));

    // Override the environment variable so our client finds the test cache directory
    std::env::set_var("DURA_CACHE_HOME", dura.runtime_lock_path().parent().unwrap());

    // Try sending a status command
    let res = dura::poller::send_uds_command("status").await;
    assert!(res.is_ok(), "Failed to send UDS command: {:?}", res.err());
    let res_json: serde_json::Value = serde_json::from_str(&res.unwrap()).unwrap();
    assert_eq!(res_json["status"], "ok");

    // Try sending a reload command
    let res = dura::poller::send_uds_command("reload").await;
    assert!(res.is_ok());

    // Clean up env var
    std::env::remove_var("DURA_CACHE_HOME");
}


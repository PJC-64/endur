use anyhow::{anyhow, Result};

#[cfg(any(target_os = "macos", target_os = "linux"))]
use anyhow::Context;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::env;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::fs::{self, create_dir_all};
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::process::Command;

#[cfg(target_os = "macos")]
fn get_uid() -> Result<String> {
    let output = Command::new("id")
        .arg("-u")
        .output()
        .context("Failed to run 'id -u' command")?;
    if !output.status.success() {
        return Err(anyhow!(
            "'id -u' command failed with status: {:?}",
            output.status
        ));
    }
    let uid = String::from_utf8(output.stdout)?.trim().to_string();
    Ok(uid)
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn get_endur_cli_path() -> std::path::PathBuf {
    if let Ok(exe_path) = env::current_exe() {
        if let Some(file_name) = exe_path.file_name() {
            let name_str = file_name.to_string_lossy().to_lowercase();
            if name_str == "endur" || name_str == "endur.exe" {
                return exe_path;
            }
        }
        if let Some(home) = dirs::home_dir() {
            let cargo_bin = home.join(".cargo").join("bin").join("endur");
            if cargo_bin.exists() {
                return cargo_bin;
            }
        }
        if let Some(parent) = exe_path.parent() {
            let local_bin = parent.join("endur");
            if local_bin.exists() {
                return local_bin;
            }
        }
    }
    std::path::PathBuf::from("endur")
}

#[cfg(target_os = "macos")]
pub fn install() -> Result<()> {
    let exe_path = get_endur_cli_path();
    let home = dirs::home_dir().ok_or_else(|| anyhow!("Could not determine home directory"))?;
    let launch_agents_dir = home.join("Library").join("LaunchAgents");
    let plist_path = launch_agents_dir.join("com.endur.daemon.plist");

    println!(
        "Creating startup service config at: {}",
        plist_path.display()
    );
    create_dir_all(&launch_agents_dir)
        .context("Failed to create Library/LaunchAgents directory")?;

    let plist_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.endur.daemon</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>serve</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>ProcessType</key>
    <string>Background</string>
</dict>
</plist>
"#,
        exe_path.to_string_lossy()
    );

    fs::write(&plist_path, plist_content).context("Failed to write plist file")?;

    let uid = get_uid()?;
    println!("Registering service with launchctl...");

    // Attempt bootstrap (modern macOS launchctl)
    let status = Command::new("launchctl")
        .args([
            "bootstrap",
            &format!("gui/{uid}"),
            plist_path.to_str().unwrap(),
        ])
        .status();

    match status {
        Ok(s) if s.success() => {
            println!("Endur startup service successfully installed and started.");
            Ok(())
        }
        _ => {
            // Fallback to load
            println!("bootstrap failed or launchctl modern GUI not available. Falling back to 'launchctl load'...");
            let load_status = Command::new("launchctl")
                .args(["load", plist_path.to_str().unwrap()])
                .status()
                .context("Failed to execute launchctl load")?;
            if load_status.success() {
                println!("Endur startup service successfully loaded.");
                Ok(())
            } else {
                Err(anyhow!("Failed to register service with launchctl"))
            }
        }
    }
}

#[cfg(target_os = "macos")]
pub fn uninstall() -> Result<()> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("Could not determine home directory"))?;
    let plist_path = home
        .join("Library")
        .join("LaunchAgents")
        .join("com.endur.daemon.plist");

    if !plist_path.exists() {
        println!("Service configuration file does not exist. Service is not installed.");
        return Ok(());
    }

    let uid = get_uid()?;
    println!("Stopping service and unregistering from launchctl...");

    // Attempt bootout
    let status = Command::new("launchctl")
        .args([
            "bootout",
            &format!("gui/{uid}"),
            plist_path.to_str().unwrap(),
        ])
        .status();

    let stopped = match status {
        Ok(s) if s.success() => true,
        _ => {
            // Fallback to unload
            let unload_status = Command::new("launchctl")
                .args(["unload", plist_path.to_str().unwrap()])
                .status();
            match unload_status {
                Ok(s) => s.success(),
                _ => false,
            }
        }
    };

    if stopped {
        println!("Service stopped and unregistered successfully.");
    } else {
        println!("Warning: Service could not be stopped/unregistered (it might not be running).");
    }

    println!("Removing configuration file: {}", plist_path.display());
    fs::remove_file(&plist_path).context("Failed to remove plist file")?;
    println!("Endur startup service successfully uninstalled.");
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn is_installed() -> bool {
    if let Some(home) = dirs::home_dir() {
        home.join("Library")
            .join("LaunchAgents")
            .join("com.endur.daemon.plist")
            .exists()
    } else {
        false
    }
}

#[cfg(target_os = "macos")]
pub fn is_running() -> Result<bool> {
    let output = Command::new("launchctl")
        .args(["list"])
        .output()
        .context("Failed to run launchctl list")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 && parts[2] == "com.endur.daemon" {
            return Ok(parts[0] != "-");
        }
    }
    Ok(false)
}

#[cfg(target_os = "macos")]
pub fn start() -> Result<()> {
    let uid = get_uid()?;
    let status = Command::new("launchctl")
        .args(["kickstart", "-k", &format!("gui/{uid}/com.endur.daemon")])
        .status();
    match status {
        Ok(s) if s.success() => Ok(()),
        _ => {
            let status = Command::new("launchctl")
                .args(["start", "com.endur.daemon"])
                .status()
                .context("Failed to run launchctl start")?;
            if status.success() {
                Ok(())
            } else {
                Err(anyhow!("Failed to start launchctl service"))
            }
        }
    }
}

#[cfg(target_os = "macos")]
pub fn stop() -> Result<()> {
    let status = Command::new("launchctl")
        .args(["stop", "com.endur.daemon"])
        .status()
        .context("Failed to run launchctl stop")?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("Failed to stop launchctl service"))
    }
}

#[cfg(target_os = "linux")]
pub fn install() -> Result<()> {
    let exe_path = get_endur_cli_path();
    let home = dirs::home_dir().ok_or_else(|| anyhow!("Could not determine home directory"))?;
    let systemd_dir = home.join(".config").join("systemd").join("user");
    let service_path = systemd_dir.join("endur.service");

    println!(
        "Creating systemd service config at: {}",
        service_path.display()
    );
    create_dir_all(&systemd_dir).context("Failed to create systemd user directory")?;

    let service_content = format!(
        r#"[Unit]
Description=Endur Git Auto-Backup Daemon
After=default.target

[Service]
ExecStart={} serve
Restart=always
RestartSec=5

[Install]
WantedBy=default.target
"#,
        exe_path.to_string_lossy()
    );

    fs::write(&service_path, service_content).context("Failed to write systemd service file")?;

    println!("Reloading systemd manager configuration...");
    let status = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status()
        .context("Failed to run 'systemctl --user daemon-reload'")?;
    if !status.success() {
        return Err(anyhow!("systemctl daemon-reload failed"));
    }

    println!("Enabling endur service...");
    let status = Command::new("systemctl")
        .args(["--user", "enable", "endur"])
        .status()
        .context("Failed to run 'systemctl --user enable'")?;
    if !status.success() {
        return Err(anyhow!("systemctl enable failed"));
    }

    println!("Starting endur service...");
    let status = Command::new("systemctl")
        .args(["--user", "start", "endur"])
        .status()
        .context("Failed to run 'systemctl --user start'")?;
    if !status.success() {
        return Err(anyhow!("systemctl start failed"));
    }

    println!("Endur startup service successfully installed and started.");
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn uninstall() -> Result<()> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("Could not determine home directory"))?;
    let service_path = home
        .join(".config")
        .join("systemd")
        .join("user")
        .join("endur.service");

    if !service_path.exists() {
        println!("Service configuration file does not exist. Service is not installed.");
        return Ok(());
    }

    println!("Stopping endur service...");
    let _ = Command::new("systemctl")
        .args(["--user", "stop", "endur"])
        .status();

    println!("Disabling endur service...");
    let _ = Command::new("systemctl")
        .args(["--user", "disable", "endur"])
        .status();

    println!("Removing configuration file: {}", service_path.display());
    fs::remove_file(&service_path).context("Failed to remove systemd service file")?;

    println!("Reloading systemd manager configuration...");
    let _ = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status();

    println!("Endur startup service successfully uninstalled.");
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn is_installed() -> bool {
    if let Some(home) = dirs::home_dir() {
        home.join(".config")
            .join("systemd")
            .join("user")
            .join("endur.service")
            .exists()
    } else {
        false
    }
}

#[cfg(target_os = "linux")]
pub fn is_running() -> Result<bool> {
    let output = Command::new("systemctl")
        .args(["--user", "is-active", "endur"])
        .output()
        .context("Failed to run systemctl is-active")?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(stdout == "active")
}

#[cfg(target_os = "linux")]
pub fn start() -> Result<()> {
    let status = Command::new("systemctl")
        .args(["--user", "start", "endur"])
        .status()
        .context("Failed to run systemctl start")?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("Failed to start systemd service"))
    }
}

#[cfg(target_os = "linux")]
pub fn stop() -> Result<()> {
    let status = Command::new("systemctl")
        .args(["--user", "stop", "endur"])
        .status()
        .context("Failed to run systemctl stop")?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("Failed to stop systemd service"))
    }
}

#[cfg(target_os = "windows")]
pub fn install() -> Result<()> {
    let exe_path = get_endur_cli_path();
    let cmd_str = format!("\"{}\" serve", exe_path.to_string_lossy());
    
    println!("Adding Endur to Windows CurrentVersion\\Run registry...");
    let status = Command::new("reg")
        .args([
            "add",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
            "/v",
            "EndurDaemon",
            "/t",
            "REG_SZ",
            "/d",
            &cmd_str,
            "/f",
        ])
        .status()
        .context("Failed to run reg add command")?;

    if !status.success() {
        return Err(anyhow!("Failed to add registry entry for Endur"));
    }

    start()?;
    println!("Endur startup service successfully installed and started.");
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn uninstall() -> Result<()> {
    let _ = stop();

    println!("Removing Endur from Windows CurrentVersion\\Run registry...");
    let status = Command::new("reg")
        .args([
            "delete",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
            "/v",
            "EndurDaemon",
            "/f",
        ])
        .status()
        .context("Failed to run reg delete command")?;

    if status.success() {
        println!("Endur startup service successfully uninstalled.");
    } else {
        println!("Warning: Registry entry could not be removed (it might not have existed).");
    }
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn is_installed() -> bool {
    let output = Command::new("reg")
        .args([
            "query",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
            "/v",
            "EndurDaemon",
        ])
        .output();
    
    match output {
        Ok(out) => out.status.success(),
        Err(_) => false,
    }
}

#[cfg(target_os = "windows")]
pub fn is_running() -> Result<bool> {
    Ok(crate::database::RuntimeLock::is_active())
}

#[cfg(target_os = "windows")]
pub fn start() -> Result<()> {
    let exe_path = get_endur_cli_path();
    let logfile_path = crate::database::RuntimeLock::get_endur_cache_home().join("endur.log");
    
    let mut cmd = Command::new(exe_path);
    cmd.arg("serve").arg("--logfile").arg(logfile_path);
    
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(0x00000008 | 0x00000200);
    
    cmd.spawn()
        .context("Failed to spawn endur serve daemon process")?;
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn stop() -> Result<()> {
    let exe_path = get_endur_cli_path();
    let status = Command::new(exe_path)
        .arg("kill")
        .status()
        .context("Failed to run endur kill command")?;
    
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("Failed to stop endur daemon"))
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
pub fn install() -> Result<()> {
    Err(anyhow!(
        "Automatic service installation is not supported on this platform."
    ))
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
pub fn uninstall() -> Result<()> {
    Err(anyhow!(
        "Automatic service uninstallation is not supported on this platform."
    ))
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
pub fn is_installed() -> bool {
    false
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
pub fn is_running() -> Result<bool> {
    Ok(false)
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
pub fn start() -> Result<()> {
    Err(anyhow!(
        "Service management is not supported on this platform."
    ))
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
pub fn stop() -> Result<()> {
    Err(anyhow!(
        "Service management is not supported on this platform."
    ))
}

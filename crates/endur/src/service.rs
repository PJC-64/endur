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

#[cfg(target_os = "macos")]
pub fn install() -> Result<()> {
    let exe_path = env::current_exe().context("Failed to get current executable path")?;
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

#[cfg(target_os = "linux")]
pub fn install() -> Result<()> {
    let exe_path = env::current_exe().context("Failed to get current executable path")?;
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

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn install() -> Result<()> {
    Err(anyhow!(
        "Automatic service installation is not supported on this platform."
    ))
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn uninstall() -> Result<()> {
    Err(anyhow!(
        "Automatic service uninstallation is not supported on this platform."
    ))
}

use std::fs::{File, OpenOptions};
use std::io::{stdin, stdout, BufReader, BufWriter, Read, Write};
use std::path::Path;
use std::process;

use chrono::TimeZone;
use clap::builder::IntoResettable;
use clap::{
    arg, crate_authors, crate_description, crate_name, crate_version, value_parser, Arg, Command,
};
use endur::config::{Config, WatchConfig};
use endur::database::RuntimeLock;
use endur::logger::NestedJsonLayer;
use endur::metrics;
use endur::poller;
use endur::service;
use endur::snapshots;
use tracing::info;
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Registry};

#[tokio::main]
async fn main() {
    if !check_if_user() {
        eprintln!("Endur cannot be run as root, to avoid data corruption");
        process::exit(1);
    }

    let cwd = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("Failed to get current directory: {e}");
            process::exit(1);
        }
    };

    let arg_directory = Arg::new("directory")
        .default_value(cwd.into_os_string().into_resettable())
        .help("The directory to watch. Defaults to current directory");

    let matches = Command::new(crate_name!())
        .about(crate_description!())
        .disable_version_flag(true)
        .arg_required_else_help(true)
        .author(crate_authors!())
        .arg(
            Arg::new("version")
                .short('v')
                .long("version")
                .action(clap::builder::ArgAction::SetTrue)
                .help("Print version information")
        )
        .subcommand(
            Command::new("capture")
                .short_flag('C')
                .long_flag("capture")
                .about("Run a single backup of an entire repository. This is the one single iteration of the `serve` control loop.")
                .arg(arg_directory.clone())
        )
        .subcommand(
            Command::new("info")
                .visible_alias("status")
                .short_flag('I')
                .long_flag("info")
                .about("Prints summary information about the current configuration and repository status.")
                .arg(
                    arg!(-d --detail "Show detailed output")
                        .required(false)
                        .action(clap::builder::ArgAction::SetTrue)
                )
        )
        .subcommand(
            Command::new("serve")
                .short_flag('S')
                .long_flag("serve")
                .about("Starts the worker that listens for file changes. If another process is already running, this will do it's best to terminate the other process.")
                .arg(
                    arg!(--logfile <FILE>)
                    .required(false)
                    .help("Sets custom logfile. Default is logging to stdout")
        ))
        .subcommand(
            Command::new("watch")
                .short_flag('W')
                .long_flag("watch")
                .about("Add the current working directory as a repository to watch.")
                .arg(arg_directory.clone())
                .arg(arg!(-i --include)
                    .required(false)
                    .action(clap::builder::ArgAction::Set)
                    .num_args(0..)
                    .value_parser(value_parser!(String))
                    .value_delimiter(',')
                    .help("Overrides excludes by re-including specific directories relative to the watch directory.")
                )
                .arg(arg!(-e --exclude)
                    .required(false)
                    .action(clap::builder::ArgAction::Set)
                    .num_args(0..)
                    .value_parser(value_parser!(String))
                    .value_delimiter(',')
                    .help("Excludes specific directories relative to the watch directory")
                )
                .arg(arg!(-d --maxdepth)
                    .required(false)
                    .action(clap::builder::ArgAction::Set)
                    .value_parser(value_parser!(String))
                    .default_value("255".to_string())
                    .num_args(0..=1)
                    .help("Determines the depth to recurse into when scanning directories")
                )
        )
        .subcommand(
            Command::new("unwatch")
                .short_flag('U')
                .long_flag("unwatch")
                .about("Remove the current working directory as a repository to watch.")
                .arg(arg_directory.clone())
        )
        .subcommand(
            Command::new("kill")
                .short_flag('K')
                .long_flag("kill")
                .about("Stop the running worker (should only be a single worker).")
        )
        .subcommand(
            Command::new("metrics")
                .short_flag('M')
                .long_flag("metrics")
                .about("Convert logs into richer metrics about snapshots.")
                .disable_help_flag(true)
                .arg(arg!(--help "Print help")
                     .action(clap::builder::ArgAction::Help)
                )
                .arg(arg!(-i --input <FILE>)
                     .required(false)
                     .help("The log file to read. Defaults to stdin.")
                 )
                .arg(arg!(-o --output <FILE>)
                     .required(false)
                     .help("The json file to write. Defaults to stdout.")
                 )
                .arg(Arg::new("human-readable")
                     .short('h')
                     .long("human-readable")
                     .action(clap::builder::ArgAction::SetTrue)
                     .help("Print metrics in a human-readable table and summary format")
                 )
        )
        .subcommand(
            Command::new("list-snapshots")
                .about("List all local endur backup snapshots.")
                .arg(arg_directory.clone())
                .arg(
                    arg!(--all "Show all historical snapshots, not just those after the latest commit")
                        .action(clap::builder::ArgAction::SetTrue)
                )
        )
        .subcommand(
            Command::new("restore")
                .about("Restore files from a specific endur backup snapshot.")
                .arg(
                    Arg::new("hash")
                        .required_unless_present("interactive")
                        .help("The commit hash of the snapshot to restore")
                )
                .arg(
                    arg!(-i --interactive "Interactive mode using TUI")
                        .action(clap::builder::ArgAction::SetTrue)
                )
                .arg(
                    Arg::new("files")
                        .short('f')
                        .long("files")
                        .num_args(1..)
                        .value_name("FILE")
                        .help("Specific files or directories to restore")
                )
                .arg(arg_directory.clone())
        )
        .subcommand(
            Command::new("prune")
                .about("Prune historical backup snapshots.")
                .arg(arg_directory.clone())
                .arg(
                    Arg::new("commit")
                        .help("Target formal commit hash. All snapshots prior to this commit will be pruned.")
                        .required_unless_present_any(["keep", "before", "interactive"])
                )
                .arg(
                    arg!(-k --keep <N> "Keep snapshots for the last N formal commits and prune older")
                        .value_parser(value_parser!(usize))
                        .conflicts_with_all(["commit", "before", "interactive"])
                )
                .arg(
                    arg!(-b --before <DURATION> "Prune snapshots older than a duration (e.g., 30d, 12h, 5m)")
                        .conflicts_with_all(["commit", "keep", "interactive"])
                )
                .arg(
                    arg!(-i --interactive "Interactive mode: select the cutoff commit using TUI")
                        .action(clap::builder::ArgAction::SetTrue)
                        .conflicts_with_all(["commit", "keep", "before"])
                )
                .arg(
                    arg!(-y --yes "Skip the confirmation prompt before executing deletion")
                        .action(clap::builder::ArgAction::SetTrue)
                )
                .arg(
                    arg!(--gc "Run git gc --prune=now immediately after pruning")
                        .action(clap::builder::ArgAction::SetTrue)
                )
                .arg(
                    arg!(--"dry-run" "List what would be deleted without actually deleting")
                        .action(clap::builder::ArgAction::SetTrue)
                )
        )
        .subcommand(
            Command::new("cleanup")
                .about("Remove any inaccessible or invalid repositories from the watch list.")
        )
        .subcommand(
            Command::new("service")
                .about("Manage endur background service")
                .subcommand(Command::new("install").about("Install endur as a system startup service (launchd on macOS, systemd on Linux)"))
                .subcommand(Command::new("uninstall").about("Uninstall endur startup service"))
        )
        .subcommand(
            Command::new("tui")
                .about("Interactive control center for daemon monitoring and snapshot management.")
        )
        .get_matches();

    if matches.get_flag("version") {
        print_version_info();
        return;
    }

    match matches.subcommand() {
        Some(("capture", arg_matches)) => {
            let dir = Path::new(arg_matches.get_one::<String>("directory").unwrap());
            match snapshots::capture(dir) {
                Ok(oid_opt) => {
                    if let Some(oid) = oid_opt {
                        println!("{oid}");
                    }
                }
                Err(e) => {
                    println!("Endur capture failed: {e}");
                    process::exit(1);
                }
            }
        }
        Some(("info", arg_matches)) => {
            let config = Config::load();
            if arg_matches.get_flag("detail") {
                config.print_detailed_info();
            } else {
                config.print_summary();
            }
        }
        Some(("serve", arg_matches)) => {
            let env_filter =
                EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

            let logfile_path = match arg_matches.get_one::<String>("logfile") {
                Some(logfile) => std::path::PathBuf::from(logfile),
                None => RuntimeLock::get_endur_cache_home().join("endur.log"),
            };

            if let Some(parent) = logfile_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }

            let file_str = logfile_path.to_string_lossy().to_string();
            Registry::default()
                .with(env_filter)
                .with(NestedJsonLayer::new(move || {
                    let result_open_file =
                        OpenOptions::new().append(true).create(true).open(&file_str);
                    match result_open_file {
                        Ok(f) => f,
                        Err(e) => {
                            eprintln!("Unable to open file {file_str} for logging due to {e}");
                            std::process::exit(1);
                        }
                    }
                }))
                .init();

            info!("Started serving with endur v{}", crate_version!());
            poller::start().await;
        }
        Some(("watch", arg_matches)) => {
            let dir = Path::new(arg_matches.get_one::<String>("directory").unwrap());

            let include = arg_matches
                .get_many::<String>("include")
                .unwrap_or_default()
                .map(|s| s.to_string())
                .collect::<Vec<String>>();
            let exclude = arg_matches
                .get_many::<String>("exclude")
                .unwrap_or_default()
                .map(|s| s.to_string())
                .collect::<Vec<String>>();
            let max_depth = match arg_matches
                .get_one::<String>("maxdepth")
                .unwrap_or(&"255".to_string())
                .parse::<u8>()
            {
                Ok(depth) => depth,
                Err(_) => {
                    eprintln!("Max depth must be a number between 0 and 255");
                    process::exit(1);
                }
            };

            let watch_config = WatchConfig {
                include,
                exclude,
                max_depth,
            };

            watch_dir(dir, watch_config).await;
        }
        Some(("unwatch", arg_matches)) => {
            let dir = Path::new(arg_matches.get_one::<String>("directory").unwrap());
            unwatch_dir(dir).await;
        }
        Some(("kill", _)) => {
            kill().await;
        }
        Some(("metrics", arg_matches)) => {
            let mut input: Box<dyn Read> = match arg_matches.get_one::<String>("input") {
                Some(input) => match File::open(input) {
                    Ok(file) => Box::new(file),
                    Err(e) => {
                        eprintln!("Couldn't open '{input}': {e}");
                        process::exit(1);
                    }
                },
                None => {
                    use std::io::IsTerminal;
                    if std::io::stdin().is_terminal() {
                        let log_path = RuntimeLock::get_endur_cache_home().join("endur.log");
                        match File::open(&log_path) {
                            Ok(file) => Box::new(BufReader::new(file)),
                            Err(e) => {
                                eprintln!(
                                    "Couldn't open default log file '{}': {e}",
                                    log_path.display()
                                );
                                process::exit(1);
                            }
                        }
                    } else {
                        Box::new(BufReader::new(stdin()))
                    }
                }
            };
            let mut output: Box<dyn Write> = match arg_matches.get_one::<String>("output") {
                Some(output) => match File::create(output) {
                    Ok(file) => Box::new(file),
                    Err(e) => {
                        eprintln!("Couldn't create '{output}': {e}");
                        process::exit(1);
                    }
                },
                None => Box::new(BufWriter::new(stdout())),
            };
            let human_readable = arg_matches.get_flag("human-readable");
            if let Err(e) =
                metrics::get_snapshot_metrics(&mut input, &mut output, human_readable, false)
            {
                eprintln!("Failed: {e}");
                process::exit(1);
            }
        }
        Some(("list-snapshots", arg_matches)) => {
            let dir = Path::new(arg_matches.get_one::<String>("directory").unwrap());
            let show_all = arg_matches.get_flag("all");
            match snapshots::list_snapshots(dir, show_all) {
                Ok(snapshots) => {
                    if snapshots.is_empty() {
                        println!("No snapshots found in repository: {}", dir.display());
                    } else {
                        println!("Endur Snapshots for repository: {}", dir.display());
                        println!(
                            "{:<40} {:<25} {:<10}",
                            "Commit Hash", "Date/Time", "Changes"
                        );
                        println!("{}", "-".repeat(80));
                        for snap in snapshots {
                            let date_time = chrono::Local
                                .timestamp_opt(snap.timestamp, 0)
                                .single()
                                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                                .unwrap_or_else(|| "Unknown".to_string());
                            println!(
                                "{:<40} {:<25} {:<10} files",
                                snap.commit_hash, date_time, snap.files_changed
                            );
                        }
                    }
                }
                Err(e) => {
                    println!("Failed to list snapshots: {e}");
                    process::exit(1);
                }
            }
        }
        Some(("tui", _)) => {
            if let Err(e) = endur::tui::run_control_center().await {
                eprintln!("Failed to run interactive TUI: {e}");
                process::exit(1);
            }
        }
        Some(("restore", arg_matches)) => {
            let (dir, hash, files_to_restore) = if arg_matches.get_flag("interactive") {
                match endur::tui::run_interactive() {
                    Ok(Some((repo, hash, files))) => (repo, hash, files),
                    Ok(None) => {
                        println!("Interactive restore cancelled.");
                        return;
                    }
                    Err(e) => {
                        println!("Failed to run interactive TUI: {e}");
                        process::exit(1);
                    }
                }
            } else {
                let dir =
                    Path::new(arg_matches.get_one::<String>("directory").unwrap()).to_path_buf();
                let hash = arg_matches.get_one::<String>("hash").unwrap().to_string();
                let files = arg_matches
                    .get_many::<String>("files")
                    .map(|vals| vals.map(|s| s.to_string()).collect::<Vec<String>>());
                (dir, hash, files)
            };

            match snapshots::restore(&dir, &hash, files_to_restore.as_deref()) {
                Ok(changes) => {
                    if changes.is_empty() {
                        println!("No files needed to be restored or changed for commit {hash}");
                    } else {
                        println!(
                            "Restored working directory and index of {} to snapshot {hash}:",
                            dir.display()
                        );
                        for (status, path) in changes {
                            println!("  {status} {path}");
                        }
                    }
                }
                Err(e) => {
                    println!(
                        "Failed to restore snapshot {hash} in {}: {e}",
                        dir.display()
                    );
                    process::exit(1);
                }
            }
        }
        Some(("prune", arg_matches)) => {
            let dir = Path::new(arg_matches.get_one::<String>("directory").unwrap());
            let target_commit = if arg_matches.get_flag("interactive") {
                let repo = match git2::Repository::open(dir) {
                    Ok(r) => r,
                    Err(e) => {
                        println!("Failed to open repository: {e}");
                        process::exit(1);
                    }
                };
                let mut commits = Vec::new();
                if let Ok(head_ref) = repo.head() {
                    if let Ok(head_commit) = head_ref.peel_to_commit() {
                        if let Ok(mut revwalk) = repo.revwalk() {
                            let _ = revwalk.push(head_commit.id());
                            for oid in revwalk.flatten() {
                                if let Ok(commit) = repo.find_commit(oid) {
                                    commits.push(commit);
                                }
                            }
                        }
                    }
                }

                if commits.is_empty() {
                    println!("No formal commits found in repository history.");
                    return;
                }

                println!("Select the cutoff formal commit (snapshots prior to it will be pruned):");
                for (i, commit) in commits.iter().take(10).enumerate() {
                    let summary = commit.summary().unwrap_or("");
                    let date_time = chrono::Local
                        .timestamp_opt(commit.time().seconds(), 0)
                        .single()
                        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_else(|| "Unknown".to_string());
                    println!("[{}] {} ({}) - {}", i, commit.id(), date_time, summary);
                }
                if commits.len() > 10 {
                    println!("... and {} more commits", commits.len() - 10);
                }

                print!("Enter index or commit hash: ");
                let _ = stdout().flush();
                let mut input = String::new();
                if stdin().read_line(&mut input).is_err() {
                    println!("Failed to read input.");
                    return;
                }
                let input = input.trim();
                if let Ok(idx) = input.parse::<usize>() {
                    if idx < commits.len() {
                        Some(commits[idx].id().to_string())
                    } else {
                        println!("Invalid index.");
                        return;
                    }
                } else if !input.is_empty() {
                    Some(input.to_string())
                } else {
                    println!("Cancelled.");
                    return;
                }
            } else {
                arg_matches
                    .get_one::<String>("commit")
                    .map(|s| s.to_string())
            };

            let report = match endur::prune::prune(
                dir,
                &endur::prune::PruneOptions {
                    target_commit: target_commit.clone(),
                    keep_last_n: arg_matches.get_one::<usize>("keep").copied(),
                    before_duration: arg_matches.get_one::<String>("before").cloned(),
                    dry_run: true,
                    run_gc: false,
                },
            ) {
                Ok(r) => r,
                Err(e) => {
                    println!("Prune planning failed: {e}");
                    process::exit(1);
                }
            };

            if report.pruned.is_empty() {
                println!("No snapshots match the criteria for pruning.");
                return;
            }

            println!(
                "The following {} snapshot branches will be pruned:",
                report.pruned.len()
            );
            for item in &report.pruned {
                println!(
                    "  {} (latest snapshot: {})",
                    item.branch_name, item.latest_snapshot_hash
                );
            }

            if !arg_matches.get_flag("dry-run") {
                let proceed = if arg_matches.get_flag("yes") {
                    true
                } else {
                    print!("Are you sure you want to prune these snapshots? [y/N]: ");
                    let _ = stdout().flush();
                    let mut input = String::new();
                    if stdin().read_line(&mut input).is_err() {
                        false
                    } else {
                        input.trim().eq_ignore_ascii_case("y")
                    }
                };

                if proceed {
                    match endur::prune::prune(
                        dir,
                        &endur::prune::PruneOptions {
                            target_commit,
                            keep_last_n: arg_matches.get_one::<usize>("keep").copied(),
                            before_duration: arg_matches.get_one::<String>("before").cloned(),
                            dry_run: false,
                            run_gc: arg_matches.get_flag("gc"),
                        },
                    ) {
                        Ok(real_report) => {
                            println!(
                                "Successfully pruned {} snapshot branches.",
                                real_report.pruned.len()
                            );
                            if real_report.gc_run {
                                println!("Garbage collection ran successfully.");
                            }
                        }
                        Err(e) => {
                            println!("Failed to prune snapshots: {e}");
                            process::exit(1);
                        }
                    }
                } else {
                    println!("Pruning cancelled.");
                }
            }
        }
        Some(("cleanup", _)) => {
            let mut config = Config::load();
            let mut to_remove = Vec::new();

            for repo_path_str in config.repos.keys() {
                let path = Path::new(repo_path_str);
                if git2::Repository::open(path).is_err() {
                    to_remove.push(repo_path_str.clone());
                }
            }

            if to_remove.is_empty() {
                println!("No inaccessible repositories found in watch list.");
            } else {
                println!(
                    "Found {} inaccessible repository/repositories:",
                    to_remove.len()
                );
                for repo in &to_remove {
                    println!("  Removing: {repo}");
                    config.repos.remove(repo);
                }
                config.save();
                println!("Cleaned up configuration successfully.");

                // Notify daemon
                let _ = endur::poller::send_uds_command("reload").await;
            }
        }
        Some(("service", arg_matches)) => match arg_matches.subcommand() {
            Some(("install", _)) => {
                if let Err(e) = service::install() {
                    eprintln!("Error installing service: {e}");
                    process::exit(1);
                }
            }
            Some(("uninstall", _)) => {
                if let Err(e) = service::uninstall() {
                    eprintln!("Error uninstalling service: {e}");
                    process::exit(1);
                }
            }
            _ => {
                eprintln!("Invalid service command. Use 'install' or 'uninstall'.");
                process::exit(1);
            }
        },
        _ => unreachable!(),
    }
}

async fn watch_dir(path: &std::path::Path, watch_config: WatchConfig) {
    let mut config = Config::load();
    let path_str = match path.to_str() {
        Some(s) => s.to_string(),
        None => {
            eprintln!("The provided path is not valid unicode");
            process::exit(1);
        }
    };

    if let Err(e) = config.set_watch(path_str, watch_config) {
        eprintln!("{e}");
        process::exit(1);
    }
    config.save();

    // Notify daemon
    let _ = endur::poller::send_uds_command("reload").await;
}

async fn unwatch_dir(path: &std::path::Path) {
    let mut config = Config::load();

    let path_str = match path.to_str() {
        Some(s) => s.to_string(),
        None => {
            eprintln!("The provided path is not valid unicode");
            process::exit(1);
        }
    };

    if let Err(e) = config.set_unwatch(path_str) {
        eprintln!("{e}");
        process::exit(1);
    }
    config.save();

    // Notify daemon
    let _ = endur::poller::send_uds_command("reload").await;
}

#[cfg(unix)]
fn check_if_user() -> bool {
    sudo::check() != sudo::RunningAs::Root
}

#[cfg(target_os = "windows")]
fn check_if_user() -> bool {
    true
}

/// Stops the running endur poller.
async fn kill() {
    match endur::poller::send_uds_command("kill").await {
        Ok(res) => {
            println!("Sent kill command to daemon: {res}");
        }
        Err(_) => {
            if RuntimeLock::is_active() {
                let mut runtime_lock = RuntimeLock::load();
                runtime_lock.pid = None;
                runtime_lock.save();
                println!("Endur server terminated via lock file fallback.");
            } else {
                println!("Endur server is not running.");
            }
        }
    }
}

fn print_version_info() {
    let suffix = option_env!("ENDUR_VERSION_SUFFIX")
        .map(|v| format!(" @ {v}"))
        .unwrap_or_else(|| String::from(""));
    let version = format!("{}{}", crate_version!(), suffix);

    println!("endur {version}");
    println!("  TUI: Enabled (ratatui 0.30)");
    println!("  IPC: Unix Domain Sockets (UDS)");
    println!("  Locking: Advisory File Locks (fs2)");

    if let Ok(config_path) = Config::default_path() {
        if let Some(parent) = config_path.parent() {
            println!("  Config Home: {}", parent.display());
        }
    }

    let cache_path = RuntimeLock::default_path();
    if let Some(parent) = cache_path.parent() {
        println!("  Cache Home:  {}", parent.display());
    }
}

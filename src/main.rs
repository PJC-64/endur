use std::fs::{File, OpenOptions};
use std::io::{stdin, stdout, BufReader, BufWriter, Read, Write};
use std::path::Path;
use std::process;

use clap::builder::IntoResettable;
use clap::{
    arg, crate_authors, crate_description, crate_name, crate_version, value_parser, Arg, Command,
};
use dura::config::{Config, WatchConfig};
use dura::database::RuntimeLock;
use dura::logger::NestedJsonLayer;
use dura::metrics;
use dura::poller;
use dura::snapshots;
use tracing::info;
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Registry};

#[tokio::main]
async fn main() {
    if !check_if_user() {
        eprintln!("Dura cannot be run as root, to avoid data corruption");
        process::exit(1);
    }

    let cwd = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("Failed to get current directory: {e}");
            process::exit(1);
        }
    };

    let suffix = option_env!("DURA_VERSION_SUFFIX")
        .map(|v| format!(" @ {v}"))
        .unwrap_or_else(|| String::from(""));

    let version = format!("{}{}", crate_version!(), suffix);

    let arg_directory = Arg::new("directory")
        .default_value(cwd.into_os_string().into_resettable())
        .help("The directory to watch. Defaults to current directory");

    let matches = Command::new(crate_name!())
        .about(crate_description!())
        .version(version.into_resettable())
        .subcommand_required(true)
        .arg_required_else_help(true)
        .author(crate_authors!())
        .subcommand(
            Command::new("capture")
                .short_flag('C')
                .long_flag("capture")
                .about("Run a single backup of an entire repository. This is the one single iteration of the `serve` control loop.")
                .arg(arg_directory.clone())
        )
        .subcommand(
            Command::new("info")
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
                .arg(arg_directory)
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
                .arg(arg!(-i --input)
                     .required(false)
                     .num_args(1)
                     .help("The log file to read. Defaults to stdin.")
                 )
                .arg(arg!(-o --output)
                     .required(false)
                     .num_args(1)
                     .help("The json file to write. Defaults to stdout.")
                 )
        )
        .get_matches();

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
                    println!("Dura capture failed: {e}");
                    process::exit(1);
                }
            }
        }
        Some(("info", arg_matches)) => {
            let config = Config::load();
            if arg_matches.get_flag("detail"){
                config.print_detailed_info();
            } else {
                config.print_summary();
            }
        }
        Some(("serve", arg_matches)) => {
            let env_filter =
                EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

            match arg_matches.get_one::<String>("logfile") {
                Some(logfile) => {
                    let file = logfile.to_string();
                    Registry::default()
                        .with(env_filter)
                        .with(NestedJsonLayer::new(move || {
                            let result_open_file =
                                OpenOptions::new().append(true).create(true).open(&file);
                            match result_open_file {
                                Ok(f) => f,
                                Err(e) => {
                                    eprintln!("Unable to open file {file} for logging due to {e}");
                                    std::process::exit(1);
                                }
                            }
                        }))
                        .init();
                }
                None => {
                    Registry::default()
                        .with(env_filter)
                        .with(NestedJsonLayer::new(std::io::stdout))
                        .init();
                }
            }

            info!("Started serving with dura v{}", crate_version!());
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
                .parse::<u8>() {
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

            watch_dir(dir, watch_config);
        }
        Some(("unwatch", arg_matches)) => {
            let dir = Path::new(arg_matches.get_one::<String>("directory").unwrap());
            unwatch_dir(dir)
        }
        Some(("kill", _)) => {
            kill();
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
                None => Box::new(BufReader::new(stdin())),
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
            if let Err(e) = metrics::get_snapshot_metrics(&mut input, &mut output) {
                eprintln!("Failed: {e}");
                process::exit(1);
            }
        }
        _ => unreachable!(),
    }
}

fn watch_dir(path: &std::path::Path, watch_config: WatchConfig) {
    let mut config = Config::load();
    let path = match path.to_str() {
        Some(s) => s.to_string(),
        None => {
            eprintln!("The provided path is not valid unicode");
            process::exit(1);
        }
    };

    if let Err(e) = config.set_watch(path, watch_config) {
        eprintln!("{e}");
        process::exit(1);
    }
    config.save();
}

fn unwatch_dir(path: &std::path::Path) {
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
}

#[cfg(unix)]
fn check_if_user() -> bool {
    sudo::check() != sudo::RunningAs::Root
}

#[cfg(target_os = "windows")]
fn check_if_user() -> bool {
    true
}

/// Stops the running dura poller.
///
/// The poller checks to make sure that its pid is the same as the pid
/// found in the runtime lock file. If they are not the same, it will exit.
/// This function does not actually kill the poller process, but rather
/// clears the pid from the lock file, which will cause the poller to
/// exit on its next check.
fn kill() {
    let mut runtime_lock = RuntimeLock::load();
    runtime_lock.pid = None;
    runtime_lock.save();
}

use crate::log::Operation;
use git2::{Oid, Repository};
use serde_json::map::Map;
use serde_json::value::from_value;
use serde_json::{json, Number, Value};
use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::rc::Rc;

type FlexResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn format_repo_path(path: &str) -> String {
    let p = std::path::Path::new(path);
    let components: Vec<_> = p
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    if components.is_empty() {
        return path.to_string();
    }

    if path.len() <= 30 {
        return path.to_string();
    }

    let mut result = String::new();
    for comp in components.iter().rev() {
        if comp.is_empty() || comp == "/" {
            continue;
        }
        let separator = if result.is_empty() { "" } else { "/" };
        let candidate = format!("{comp}{separator}{result}");
        if candidate.len() + 4 > 30 {
            if result.is_empty() {
                return format!(".../{}", &comp[comp.len().saturating_sub(26)..]);
            }
            return format!(".../{result}");
        }
        result = candidate;
    }
    format!(".../{result}")
}

/// Reads an input stream that contains endur logs and enriches them with more analytics-ready info
/// like number of insertions & deletions. The result is written back out to an output stream.
pub fn get_snapshot_metrics(
    input: &mut dyn io::Read,
    output: &mut dyn io::Write,
    human_readable: bool,
) -> FlexResult<()> {
    let mut reader = io::BufReader::new(input);
    let mut writer = io::BufWriter::new(output);
    let mut line: u64 = 0; // for printing better error messages
    let mut repo_cache: HashMap<String, Rc<Repository>> = HashMap::new();

    if human_readable {
        let mut snapshots = Vec::new();
        loop {
            line += 1;
            let mut input_line = String::new();
            if reader.read_line(&mut input_line)? == 0 {
                break;
            }
            match scrape_log(input_line) {
                Ok(Some(mut output)) => {
                    if let Err(e) = scrape_git(&mut output, &mut repo_cache) {
                        eprintln!("line {line}: git scrape error: {e}");
                        continue;
                    }
                    snapshots.push(output);
                }
                Ok(None) => {}
                Err(e) => eprintln!("line {line}: {e}"),
            }
        }

        if snapshots.is_empty() {
            writeln!(&mut writer, "No snapshot metrics found in the log.")?;
            writer.flush()?;
            return Ok(());
        }

        let header = format!(
            "{:<19}  {:<30}  {:>5}  {:>10}  {:>9}  {:>8}  {:<7}",
            "Date/Time", "Repository", "Files", "Insertions", "Deletions", "Latency", "Commit"
        );
        writeln!(&mut writer, "{header}")?;
        writeln!(&mut writer, "{}", "-".repeat(header.len()))?;

        for s in &snapshots {
            let time_str = s.get("time").and_then(|v| v.as_str()).unwrap_or("");
            let formatted_time = if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(time_str) {
                dt.format("%Y-%m-%d %H:%M:%S").to_string()
            } else {
                time_str
                    .replace('T', " ")
                    .chars()
                    .take(19)
                    .collect::<String>()
            };

            let repo = s.get("repo").and_then(|v| v.as_str()).unwrap_or("");
            let formatted_repo = format_repo_path(repo);

            let files = s
                .get("num_files_changed")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let insertions = s.get("insertions").and_then(|v| v.as_u64()).unwrap_or(0);
            let deletions = s.get("deletions").and_then(|v| v.as_u64()).unwrap_or(0);

            let latency = s.get("latency").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let formatted_latency = if latency < 1.0 {
                format!("{:.1}ms", latency * 1000.0)
            } else {
                format!("{:.2}s", latency)
            };

            let commit_hash = s.get("commit_hash").and_then(|v| v.as_str()).unwrap_or("");
            let short_commit = &commit_hash[..commit_hash.len().min(7)];

            writeln!(
                &mut writer,
                "{:<19}  {:<30}  {:>5}  {:>10}  {:>9}  {:>8}  {:<7}",
                formatted_time,
                formatted_repo,
                files,
                insertions,
                deletions,
                formatted_latency,
                short_commit
            )?;
        }

        let total_snapshots = snapshots.len();
        let mut unique_repos = std::collections::HashSet::new();
        let mut total_files = 0;
        let mut total_insertions = 0;
        let mut total_deletions = 0;
        let mut latencies = Vec::new();

        for s in &snapshots {
            if let Some(repo) = s.get("repo").and_then(|v| v.as_str()) {
                unique_repos.insert(repo.to_string());
            }
            total_files += s
                .get("num_files_changed")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            total_insertions += s.get("insertions").and_then(|v| v.as_u64()).unwrap_or(0);
            total_deletions += s.get("deletions").and_then(|v| v.as_u64()).unwrap_or(0);
            if let Some(latency) = s.get("latency").and_then(|v| v.as_f64()) {
                latencies.push(latency);
            }
        }

        let avg_latency = if latencies.is_empty() {
            0.0
        } else {
            latencies.iter().sum::<f64>() / latencies.len() as f64
        };
        let max_latency = latencies.iter().copied().fold(0.0, f64::max);

        let formatted_avg = if avg_latency < 1.0 {
            format!("{:.1}ms", avg_latency * 1000.0)
        } else {
            format!("{:.2}s", avg_latency)
        };
        let formatted_max = if max_latency < 1.0 {
            format!("{:.1}ms", max_latency * 1000.0)
        } else {
            format!("{:.2}s", max_latency)
        };

        writeln!(&mut writer, "{}", "-".repeat(header.len()))?;
        writeln!(&mut writer, "Summary:")?;
        writeln!(&mut writer, "  Total Snapshots     : {}", total_snapshots)?;
        writeln!(
            &mut writer,
            "  Watched Repositories: {}",
            unique_repos.len()
        )?;
        writeln!(&mut writer, "  Total Files Changed : {}", total_files)?;
        writeln!(
            &mut writer,
            "  Total Insertions    : {} (+)",
            total_insertions
        )?;
        writeln!(
            &mut writer,
            "  Total Deletions     : {} (-)",
            total_deletions
        )?;
        writeln!(&mut writer, "  Average Latency     : {}", formatted_avg)?;
        writeln!(&mut writer, "  Maximum Latency     : {}", formatted_max)?;
    } else {
        loop {
            line += 1;
            let mut input_line = String::new();
            if reader.read_line(&mut input_line)? == 0 {
                break;
            }
            match scrape_log(input_line) {
                Ok(Some(mut output)) => {
                    scrape_git(&mut output, &mut repo_cache)?;
                    writeln!(&mut writer, "{output}")?;
                }
                Ok(None) => {}
                Err(e) => eprintln!("line {line}: {e}"),
            }
        }
    }
    writer.flush()?;
    Ok(())
}

/// Scrape information out of the snapshot log.
fn scrape_log(line: String) -> serde_json::Result<Option<Value>> {
    let input_val: Value = serde_json::from_str(line.as_str())?;
    let mut output_val = Value::Object(Map::new());

    if let Some(t) = input_val.get("time") {
        output_val["time"] = t.clone();
    }

    if let Some(op_value) = input_val.get("fields").and_then(|f| f.get("operation")) {
        match from_value(op_value.clone())? {
            Operation::Snapshot {
                repo,
                op: Some(op),
                error: _,
                latency,
            } => {
                output_val["repo"] = Value::String(repo);
                if let Some(latency) = Number::from_f64(latency as f64) {
                    output_val["latency"] = Value::Number(latency);
                }
                output_val["endur_branch"] = Value::String(op.endur_branch);
                output_val["commit_hash"] = Value::String(op.commit_hash);
                output_val["base_hash"] = Value::String(op.base_hash);
            }
            _ => return Ok(None),
        }
    } else {
        return Ok(None);
    }

    Ok(Some(output_val))
}

/// Use the info captured from scrape_log to open a repo and capture information about the commit
///
/// The repo_cache is retained between calls. This cache seems to cut runtime by 50% in a
/// completely non-scientific measure. It still seems to take unexpectedly long, probably because
/// it still has to open lots of files (for each commit & tree object) behind the scenes, and this
/// is inherently not cache-able.
fn scrape_git(
    value: &mut Value,
    repo_cache: &mut HashMap<String, Rc<Repository>>,
) -> Result<(), git2::Error> {
    if let Some(repo_path_value) = value.get("repo") {
        let repo_path = match repo_path_value.as_str() {
            Some(x) => Ok(x),
            None => Err(git2::Error::from_str("Couldn't find 'repo' in JSON")),
        }?;
        let repo = match repo_cache.get(repo_path) {
            Some(repo) => Rc::clone(repo),
            None => {
                let repo = Rc::new(Repository::open(repo_path)?);
                repo_cache.insert(repo_path.to_string(), Rc::clone(&repo));
                repo
            }
        };
        let commit_opt = value
            .get("commit_hash")
            .and_then(|c| c.as_str())
            .and_then(|c| Oid::from_str(c).ok())
            .and_then(|c| repo.find_commit(c).ok());
        let parent_commit = commit_opt.as_ref().and_then(|c| c.parents().next_back());
        if let (Some(commit), Some(parent)) = (commit_opt, parent_commit) {
            let diff =
                repo.diff_tree_to_tree(Some(&parent.tree()?), Some(&commit.tree()?), None)?;
            let stats = diff.stats()?;
            value["num_files_changed"] = json!(stats.files_changed());
            value["insertions"] = json!(stats.insertions());
            value["deletions"] = json!(stats.deletions());

            let files: Vec<_> = diff
                .deltas()
                .flat_map(|d| d.new_file().path())
                .map(|p| p.to_str())
                .collect();
            value["files_changed"] = json!(files);
        };
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::metrics::scrape_log;

    #[test]
    fn scrape_log_happy_path() {
        // broken up into multiple lines to satisfy style checker, but serde_json will handle it
        // fine
        let line = r#"{"target":"endur::poller","file":"src/poller.rs",
            "name":"event src/poller.rs:70","level":"Level(Info)",
            "fields":{
                "message":"info_operation","operation":{"Snapshot":{
                    "error":null,"latency":0.00988253,"op":{
                        "base_hash":"3e8e8c99b5434e726b13f56ba00d139bab57d5eb",
                        "commit_hash":"3423d21a2937d95119982395bc1281d3d8ebe3b6",
                        "endur_branch":"endur/3e8e8c99b5434e726b13f56ba00d139bab57d5eb"
                    },
                    "repo":"/Users/timkellogg/code/endur"}
                }
            },"time":"2022-01-14T01:49:51.638031+00:00"
        }"#;

        let output = scrape_log(line.to_string()).unwrap().unwrap();

        assert_eq!(
            output["time"].as_str(),
            Some("2022-01-14T01:49:51.638031+00:00")
        );
        assert_eq!(
            output["repo"].as_str(),
            Some("/Users/timkellogg/code/endur")
        );
        assert_eq!(
            output["endur_branch"].as_str(),
            Some("endur/3e8e8c99b5434e726b13f56ba00d139bab57d5eb")
        );
        assert_eq!(
            output["commit_hash"].as_str(),
            Some("3423d21a2937d95119982395bc1281d3d8ebe3b6")
        );
        assert_eq!(
            output["base_hash"].as_str(),
            Some("3e8e8c99b5434e726b13f56ba00d139bab57d5eb")
        );
        let latency = output["latency"].as_f64().unwrap();
        assert!(latency < (0.00988253 + f32::EPSILON).into());
        assert!(latency > (0.00988253 - f32::EPSILON).into());
    }

    #[test]
    fn scrape_log_no_snapshot() {
        // broken up into multiple lines to satisfy style checker, but serde_json will handle it
        // fine
        let line = r#"{"target":"endur","file":"src/main.rs","name":"event src/main.rs:96",
            "level":"Level(Info)","fields":{"pid":5416},
            "time":"2022-01-14T01:45:37.469819+00:00"}"#;

        let output = scrape_log(line.to_string()).unwrap();

        assert_eq!(output, None);
    }

    #[test]
    fn test_format_repo_path() {
        use crate::metrics::format_repo_path;
        assert_eq!(format_repo_path("/short/path"), "/short/path");
        assert_eq!(
            format_repo_path("/an/extremely/long/path/to/some/project/directory/endur"),
            ".../project/directory/endur"
        );
    }

    #[test]
    fn test_human_readable_output() {
        use crate::metrics::get_snapshot_metrics;
        let line = r#"{"target":"endur::poller","file":"src/poller.rs","name":"event src/poller.rs:70","level":"Level(Info)","fields":{"message":"info_operation","operation":{"Snapshot":{"error":null,"latency":0.00988253,"op":{"base_hash":"3e8e8c99b5434e726b13f56ba00d139bab57d5eb","commit_hash":"3423d21a2937d95119982395bc1281d3d8ebe3b6","endur_branch":"endur/3e8e8c99b5434e726b13f56ba00d139bab57d5eb"},"repo":"../.."}}},"time":"2022-01-14T01:49:51.638031+00:00"}"#;

        let mut input = line.as_bytes();
        let mut output = Vec::new();
        get_snapshot_metrics(&mut input, &mut output, true).unwrap();
        let output_str = String::from_utf8(output).unwrap();

        assert!(output_str.contains("Date/Time"));
        assert!(output_str.contains("Repository"));
        assert!(output_str.contains("Commit"));
        assert!(output_str.contains("2022-01-14 01:49:51"));
        assert!(output_str.contains("../.."));
        assert!(output_str.contains("3423d21"));
        assert!(output_str.contains("Summary:"));
        assert!(output_str.contains("Total Snapshots     : 1"));
    }
}

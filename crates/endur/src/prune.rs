use git2::{BranchType, Error, Oid, Repository};
use std::collections::HashSet;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::cache;

#[derive(Debug, Default, Clone)]
pub struct PruneOptions {
    pub target_commit: Option<String>,
    pub keep_last_n: Option<usize>,
    pub before_duration: Option<String>,
    pub dry_run: bool,
    pub run_gc: bool,
}

#[derive(Debug, Clone)]
pub struct PrunedSnapshot {
    pub branch_name: String,
    pub base_hash: String,
    pub latest_snapshot_hash: String,
    pub timestamp: i64,
}

#[derive(Debug, Clone)]
pub struct PruneReport {
    pub pruned: Vec<PrunedSnapshot>,
    pub dry_run: bool,
    pub gc_run: bool,
}

pub fn prune(path: &Path, options: &PruneOptions) -> Result<PruneReport, Error> {
    let repo = Repository::open(path)?;

    // 1. Determine which base commits (P) are to be pruned.
    // We will collect candidate branches `endur/P`.
    let mut candidates = Vec::new();
    if let Ok(branches) = repo.branches(Some(BranchType::Local)) {
        for (branch, _) in branches.flatten() {
            if let Ok(Some(name_str)) = branch.name() {
                if name_str.starts_with("endur/") {
                    let base_hash = name_str["endur/".len()..].to_string();
                    if let Some(target_oid) = branch.get().target() {
                        if let Ok(commit) = repo.find_commit(target_oid) {
                            candidates.push(PrunedSnapshot {
                                branch_name: name_str.to_string(),
                                base_hash,
                                latest_snapshot_hash: target_oid.to_string(),
                                timestamp: commit.time().seconds(),
                            });
                        }
                    }
                }
            }
        }
    }

    // 2. Filter candidates based on the pruning options.
    let to_prune = if let Some(ref target_hash) = options.target_commit {
        // Find the target OID.
        let target_oid = repo.revparse_single(target_hash)?.id();
        
        // Collect all ancestors of the target OID (excluding target_oid itself).
        let mut ancestors = HashSet::new();
        let mut revwalk = repo.revwalk()?;
        revwalk.push(target_oid)?;
        for oid in revwalk.flatten() {
            if oid != target_oid {
                ancestors.insert(oid);
            }
        }

        candidates
            .into_iter()
            .filter(|c| {
                if let Ok(oid) = Oid::from_str(&c.base_hash) {
                    ancestors.contains(&oid)
                } else {
                    // If base_hash is not a valid OID, we prune it because it's invalid.
                    true
                }
            })
            .collect::<Vec<_>>()
    } else if let Some(keep_n) = options.keep_last_n {
        // Collect the last N formal commits reachable from HEAD.
        let mut keep_commits = HashSet::new();
        if let Ok(head_ref) = repo.head() {
            if let Ok(head_commit) = head_ref.peel_to_commit() {
                let mut revwalk = repo.revwalk()?;
                revwalk.push(head_commit.id())?;
                let mut count = 0;
                for oid in revwalk.flatten() {
                    if count < keep_n {
                        keep_commits.insert(oid);
                        count += 1;
                    } else {
                        break;
                    }
                }
            }
        }

        candidates
            .into_iter()
            .filter(|c| {
                if let Ok(oid) = Oid::from_str(&c.base_hash) {
                    !keep_commits.contains(&oid)
                } else {
                    true
                }
            })
            .collect::<Vec<_>>()
    } else if let Some(ref dur_str) = options.before_duration {
        let duration = parse_duration(dur_str)
            .map_err(|e| Error::from_str(&e))?;
        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let cutoff_time = now_secs - duration.as_secs() as i64;

        candidates
            .into_iter()
            .filter(|c| c.timestamp < cutoff_time)
            .collect::<Vec<_>>()
    } else {
        // If no criteria specified, prune nothing.
        Vec::new()
    };

    // 3. Perform the deletion (if not dry_run).
    if !options.dry_run {
        let conn = cache::open();
        for item in &to_prune {
            // Delete branch
            if let Ok(mut branch) = repo.find_branch(&item.branch_name, BranchType::Local) {
                let _ = branch.delete();
            }
            // Delete from cache
            if let Some(ref db) = conn {
                cache::delete_snapshots_for_base(db, path, &item.base_hash);
            }
        }
    }

    let pruned = to_prune;

    // 4. Optionally run git gc.
    let mut gc_run = false;
    if options.run_gc && !options.dry_run {
        // Run git gc --prune=now
        let status = std::process::Command::new("git")
            .arg("gc")
            .arg("--prune=now")
            .current_dir(path)
            .status();
        if let Ok(st) = status {
            gc_run = st.success();
        }
    }

    Ok(PruneReport {
        pruned,
        dry_run: options.dry_run,
        gc_run,
    })
}

fn parse_duration(s: &str) -> Result<std::time::Duration, String> {
    let unit = s.chars().last().ok_or("Empty duration")?;
    let val_str = &s[..s.len() - 1];
    let val: u64 = val_str.parse().map_err(|_| format!("Invalid duration value: {}", val_str))?;
    match unit {
        's' => Ok(std::time::Duration::from_secs(val)),
        'm' => Ok(std::time::Duration::from_secs(val * 60)),
        'h' => Ok(std::time::Duration::from_secs(val * 3600)),
        'd' => Ok(std::time::Duration::from_secs(val * 86400)),
        _ => Err(format!("Unknown duration unit: {}", unit)),
    }
}

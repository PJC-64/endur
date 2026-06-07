use git2::{BranchType, DiffOptions, Error, IndexAddOption, Repository, Signature};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;

use crate::config::Config;

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct CaptureStatus {
    pub endur_branch: String,
    pub commit_hash: String,
    pub base_hash: String,
}

impl fmt::Display for CaptureStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "endur: {}, commit_hash: {}, base: {}",
            self.endur_branch, self.commit_hash, self.base_hash
        )
    }
}

pub fn is_repo(path: &Path) -> bool {
    Repository::open(path).is_ok()
}

pub fn capture(path: &Path) -> Result<Option<CaptureStatus>, Error> {
    let repo = Repository::open(path)?;
    let head = repo.head()?.peel_to_commit()?;
    let message = "endur auto-backup";

    // status check
    if repo.statuses(None)?.is_empty() {
        return Ok(None);
    }

    let branch_name = format!("endur/{}", head.id());
    let branch_commit = match repo.find_branch(&branch_name, BranchType::Local) {
        Ok(mut branch) => {
            match branch.get().peel_to_commit() {
                Ok(commit) if commit.id() != head.id() => Some(commit),
                _ => {
                    // Endur branch exists but no commit is made by endur
                    // So we clean this branch
                    branch.delete()?;
                    None
                }
            }
        }
        Err(_) => None,
    };
    let parent_commit = branch_commit.as_ref().unwrap_or(&head);

    // tree
    let index_path = repo.path().join("endur_index");
    let mut index = git2::Index::open(&index_path)?;
    index.read_tree(&parent_commit.tree()?)?;
    repo.set_index(&mut index)?;
    index.add_all(["*"].iter(), IndexAddOption::DEFAULT, None)?;
    index.write()?;

    let dirty_diff = repo.diff_tree_to_index(
        Some(&parent_commit.tree()?),
        Some(&index),
        Some(DiffOptions::new().include_untracked(true)),
    )?;
    if dirty_diff.deltas().len() == 0 {
        return Ok(None);
    }

    let tree_oid = index.write_tree()?;
    let tree = repo.find_tree(tree_oid)?;
    if repo.find_branch(&branch_name, BranchType::Local).is_err() {
        repo.branch(branch_name.as_str(), &head, false)?;
    }

    let committer = Signature::now(&get_git_author(&repo), &get_git_email(&repo))?;
    let oid = repo.commit(
        Some(&format!("refs/heads/{}", &branch_name)),
        &committer,
        &committer,
        message,
        &tree,
        &[parent_commit],
    )?;

    Ok(Some(CaptureStatus {
        endur_branch: branch_name,
        commit_hash: oid.to_string(),
        base_hash: head.id().to_string(),
    }))
}

fn get_git_author(repo: &Repository) -> String {
    let endur_cfg = Config::load();
    if let Some(value) = endur_cfg.commit_author {
        return value;
    }

    if !endur_cfg.commit_exclude_git_config {
        if let Ok(git_cfg) = repo.config() {
            if let Ok(value) = git_cfg.get_string("user.name") {
                return value;
            }
        }
    }

    "endur".to_string()
}

fn get_git_email(repo: &Repository) -> String {
    let endur_cfg = Config::load();
    if let Some(value) = endur_cfg.commit_email {
        return value;
    }

    if !endur_cfg.commit_exclude_git_config {
        if let Ok(git_cfg) = repo.config() {
            if let Ok(value) = git_cfg.get_string("user.email") {
                return value;
            }
        }
    }

    "endur@github.io".to_string()
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub struct SnapshotInfo {
    pub commit_hash: String,
    pub base_hash: String,
    pub timestamp: i64,
    pub message: String,
    pub files_changed: usize,
}

pub fn list_snapshots(path: &Path) -> Result<Vec<SnapshotInfo>, Error> {
    let repo = Repository::open(path)?;
    let mut snapshots = Vec::new();

    if let Ok(branches) = repo.branches(Some(BranchType::Local)) {
        for (branch, _) in branches.flatten() {
            if let Ok(Some(name_str)) = branch.name() {
                if name_str.starts_with("endur/") {
                    if let Some(target) = branch.get().target() {
                        if let Ok(mut revwalk) = repo.revwalk() {
                            if revwalk.push(target).is_ok() {
                                for oid in revwalk.flatten() {
                                    if let Ok(commit) = repo.find_commit(oid) {
                                        if commit.summary() == Some("endur auto-backup") {
                                            let parent_hash = if commit.parent_count() > 0 {
                                                commit
                                                    .parent_id(0)
                                                    .map(|id| id.to_string())
                                                    .unwrap_or_default()
                                            } else {
                                                "".to_string()
                                            };

                                            let mut files_changed = 0;
                                            if commit.parent_count() > 0 {
                                                if let Ok(parent) = commit.parent(0) {
                                                    if let Ok(diff) = repo.diff_tree_to_tree(
                                                        Some(&parent.tree()?),
                                                        Some(&commit.tree()?),
                                                        None,
                                                    ) {
                                                        files_changed = diff.deltas().len();
                                                    }
                                                }
                                            }

                                            snapshots.push(SnapshotInfo {
                                                commit_hash: oid.to_string(),
                                                base_hash: parent_hash,
                                                timestamp: commit.time().seconds(),
                                                message: commit.summary().unwrap_or("").to_string(),
                                                files_changed,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Sort by timestamp descending
    snapshots.sort_by_key(|s| std::cmp::Reverse(s.timestamp));
    Ok(snapshots)
}

pub fn restore(
    path: &Path,
    commit_hash: &str,
    files: Option<&[String]>,
) -> Result<Vec<(char, String)>, Error> {
    let repo = Repository::open(path)?;
    let oid = git2::Oid::from_str(commit_hash)?;
    let commit = repo.find_commit(oid)?;

    // Compute changes that will be restored relative to current HEAD
    let mut changes = Vec::new();
    if let Ok(head_ref) = repo.head() {
        if let Ok(head_commit) = head_ref.peel_to_commit() {
            if let Ok(diff) =
                repo.diff_tree_to_tree(Some(&head_commit.tree()?), Some(&commit.tree()?), None)
            {
                for delta in diff.deltas() {
                    let status_char = match delta.status() {
                        git2::Delta::Added => 'A',
                        git2::Delta::Deleted => 'D',
                        git2::Delta::Modified => 'M',
                        git2::Delta::Renamed => 'R',
                        git2::Delta::Copied => 'C',
                        _ => 'M',
                    };
                    let path_str = delta
                        .new_file()
                        .path()
                        .and_then(|p| p.to_str())
                        .unwrap_or("")
                        .to_string();

                    let should_include = if let Some(paths) = files {
                        paths.iter().any(|p| {
                            let path_p = Path::new(p);
                            let delta_path = Path::new(&path_str);
                            delta_path.starts_with(path_p)
                        })
                    } else {
                        true
                    };

                    if should_include {
                        changes.push((status_char, path_str));
                    }
                }
            }
        }
    }

    let mut checkout_opts = git2::build::CheckoutBuilder::new();
    checkout_opts.force();
    if let Some(paths) = files {
        for p in paths {
            checkout_opts.path(Path::new(p));
        }
    }

    let obj = repo.find_object(oid, None)?;
    repo.checkout_tree(&obj, Some(&mut checkout_opts))?;

    Ok(changes)
}

pub fn get_snapshot_files(path: &Path, commit_hash: &str) -> Result<Vec<(char, String)>, Error> {
    let repo = Repository::open(path)?;
    let oid = git2::Oid::from_str(commit_hash)?;
    let commit = repo.find_commit(oid)?;
    let mut files = Vec::new();

    if commit.parent_count() > 0 {
        let parent = commit.parent(0)?;
        let diff = repo.diff_tree_to_tree(Some(&parent.tree()?), Some(&commit.tree()?), None)?;
        for delta in diff.deltas() {
            let status_char = match delta.status() {
                git2::Delta::Added => 'A',
                git2::Delta::Deleted => 'D',
                git2::Delta::Modified => 'M',
                git2::Delta::Renamed => 'R',
                git2::Delta::Copied => 'C',
                _ => 'M',
            };
            let path_str = delta
                .new_file()
                .path()
                .and_then(|p| p.to_str())
                .unwrap_or("")
                .to_string();
            files.push((status_char, path_str));
        }
    }
    Ok(files)
}

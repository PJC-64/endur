mod util;

use crate::util::durable::Durable;
use crate::util::git_repo::GitRepo;
use durable::config::Config;
use std::collections::HashSet;

#[macro_use]
extern crate serial_test;

#[test]
fn watch_repo() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = GitRepo::new(tmp.path().to_path_buf());
    repo.init();

    let durable = Durable::new();
    durable.run_in_dir(&["watch"], tmp.path());

    let mut tmp_set = HashSet::new();
    tmp_set.insert(tmp.path().canonicalize().unwrap());

    assert_eq!(durable.git_repos(), tmp_set);
}

#[test]
fn watch_1_dir_with_2_repos() {
    let tmp = tempfile::tempdir().unwrap();
    let repo1 = GitRepo::new(tmp.path().join("repo1"));
    repo1.init();
    let repo2 = GitRepo::new(tmp.path().join("repo2"));
    repo2.init();

    let durable = Durable::new();
    durable.run_in_dir(&["watch"], tmp.path());

    let mut tmp_set = HashSet::new();
    tmp_set.insert(repo1.dir.canonicalize().unwrap());
    tmp_set.insert(repo2.dir.canonicalize().unwrap());

    assert_eq!(durable.git_repos(), tmp_set);
}

#[test]
fn watch_dir_with_repo_nested_3_folders_deep() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = GitRepo::new(tmp.path().join("a/b/c"));
    repo.init();

    let durable = Durable::new();
    durable.run_in_dir(&["watch"], tmp.path());

    let mut tmp_set = HashSet::new();
    tmp_set.insert(repo.dir.canonicalize().unwrap());

    assert_eq!(durable.git_repos(), tmp_set);
}

#[test]
fn test_event_driven_backup() {
    let tmp = tempfile::tempdir().unwrap();
    let mut repo = GitRepo::new(tmp.path().to_path_buf());
    repo.init();
    repo.write_file("foo.txt");
    repo.commit_all();

    let mut durable = Durable::new();
    durable.run_in_dir(&["watch"], tmp.path());

    durable.start_async(&["serve"], true);

    // Read the startup line once to ensure serve process is running
    durable.primary.as_ref().map(|d| d.read_line(8).unwrap());

    std::thread::sleep(std::time::Duration::from_millis(1500));

    std::env::set_var(
        "DURABLE_CACHE_HOME",
        durable.runtime_lock_path().parent().unwrap(),
    );
    std::env::set_var(
        "DURABLE_CONFIG_HOME",
        durable.config_path().parent().unwrap(),
    );

    repo.change_file("foo.txt");

    std::thread::sleep(std::time::Duration::from_millis(2500));

    let head_hash_raw = repo.git(&["rev-parse", "HEAD"]).unwrap();
    let head_hash = head_hash_raw.trim();
    let durable_branch = format!("durable/{head_hash}");

    let has_durable_commit = repo.git(&["rev-parse", &durable_branch]).is_some();

    std::env::remove_var("DURABLE_CACHE_HOME");
    std::env::remove_var("DURABLE_CONFIG_HOME");

    assert!(
        has_durable_commit,
        "Durable branch was not created or snapshot not captured"
    );

    let durable_hash_raw = repo.git(&["rev-parse", &durable_branch]).unwrap();
    let durable_hash = durable_hash_raw.trim();
    assert_ne!(
        durable_hash, head_hash,
        "Durable commit should not have the same hash as HEAD"
    );
}

#[test]
#[serial]
fn test_cleanup_inaccessible_repos() {
    let tmp = tempfile::tempdir().unwrap();
    let repo1 = GitRepo::new(tmp.path().join("repo1"));
    repo1.init();

    let repo2 = GitRepo::new(tmp.path().join("repo2"));
    repo2.init();

    let durable = Durable::new();

    // Watch repo1 and repo2
    durable.run_in_dir(&["watch"], &repo1.dir);
    durable.run_in_dir(&["watch"], &repo2.dir);

    // Watch a non-git directory that is invalid
    let invalid_dir = tmp.path().join("invalid_dir");
    std::fs::create_dir(&invalid_dir).unwrap();
    durable.run_in_dir(&["watch"], &invalid_dir);

    // Verify all three are watched
    std::env::set_var(
        "DURABLE_CONFIG_HOME",
        durable.config_path().parent().unwrap(),
    );
    let config = Config::load();
    assert_eq!(config.repos.len(), 3);

    // Delete repo2 directory entirely (making it inaccessible)
    std::fs::remove_dir_all(&repo2.dir).unwrap();

    // Now run cleanup command
    durable.run(&["cleanup"]);

    // Load config again and verify:
    // - repo1 is still watched (accessible)
    // - repo2 is removed (deleted directory)
    // - invalid_dir is removed (not a git repository)
    let config_after = Config::load();
    std::env::remove_var("DURABLE_CONFIG_HOME");

    assert_eq!(config_after.repos.len(), 1);
    assert!(config_after
        .repos
        .contains_key(repo1.dir.canonicalize().unwrap().to_str().unwrap()));
}

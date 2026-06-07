use endur::{config::Config, snapshots};

use std::env;

mod util;

#[macro_use]
extern crate serial_test;

#[test]
fn change_single_file() {
    let tmp = tempfile::tempdir().unwrap();
    let mut repo = repo_and_file!(tmp, "foo.txt");
    repo.change_file("foo.txt");
    let status = snapshots::capture(repo.dir.as_path()).unwrap().unwrap();

    assert_ne!(status.commit_hash, status.base_hash);
    assert_eq!(status.endur_branch, format!("endur/{}", status.base_hash));
}

#[test]
fn no_changes() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = repo_and_file!(tmp, "foo.txt");
    let status = snapshots::capture(repo.dir.as_path()).unwrap();

    assert_eq!(status, None);
}

/// It keeps capturing commits during a merge conflict
#[test]
fn during_merge_conflicts() {
    let tmp = tempfile::tempdir().unwrap();
    let mut repo = repo_and_file!(tmp, "foo.txt");

    // branch1
    repo.change_file("foo.txt");
    repo.commit_all();
    repo.git(&["checkout", "-b", "branch1"]).unwrap();

    // branch2
    repo.git(&["checkout", "-b", "branch2"]).unwrap();
    repo.git(&["reset", "HEAD^", "--hard"]).unwrap();
    repo.change_file("foo.txt");
    repo.commit_all();

    // MERGE FAIL
    let merge_result = repo.git(&["merge", "branch1"]);
    assert_eq!(merge_result, None);
    repo.git(&["status"]).unwrap(); // debug info

    // change a file anyway
    repo.change_file("foo.txt");
    let status = snapshots::capture(repo.dir.as_path()).unwrap().unwrap();

    // Regular endur commit
    assert_ne!(status.commit_hash, status.base_hash);
    assert_eq!(status.endur_branch, format!("endur/{}", status.base_hash));
}

#[test]
#[serial]
fn test_commit_signature_using_endur_config() {
    let tmp = tempfile::tempdir().unwrap();
    let mut repo = util::git_repo::GitRepo::new(tmp.path().to_path_buf());
    repo.init();
    repo.set_config("user.name", "git-author");
    repo.set_config("user.email", "git@someemail.com");

    env::set_var("ENDUR_CONFIG_HOME", tmp.path());
    let mut endur_config = Config::empty();
    endur_config.commit_author = Some("endur-config".to_string());
    endur_config.commit_email = Some("endur-config@email.com".to_string());
    endur_config.save();

    repo.write_file("foo.txt");
    repo.commit_all();

    repo.change_file("foo.txt");
    let status = snapshots::capture(repo.dir.as_path()).unwrap().unwrap();

    let commit_author = repo.git(&["show", "-s", "--format=format:%an", &status.commit_hash]);
    assert_eq!(commit_author, endur_config.commit_author);

    let commit_email = repo.git(&["show", "-s", "--format=format:%ae", &status.commit_hash]);
    assert_eq!(commit_email, endur_config.commit_email);
}

#[test]
#[serial]
fn test_commit_signature_using_git_config() {
    let tmp = tempfile::tempdir().unwrap();
    let mut repo = util::git_repo::GitRepo::new(tmp.path().to_path_buf());
    repo.init();
    repo.set_config("user.name", "git-author");
    repo.set_config("user.email", "git@someemail.com");

    env::set_var("ENDUR_CONFIG_HOME", tmp.path());
    let endur_config = Config::empty();
    endur_config.save();

    repo.write_file("foo.txt");
    repo.commit_all();

    repo.change_file("foo.txt");
    let status = snapshots::capture(repo.dir.as_path()).unwrap().unwrap();

    let commit_author = repo
        .git(&["show", "-s", "--format=format:%an", &status.commit_hash])
        .unwrap();
    assert_eq!(commit_author, "git-author");

    let commit_email = repo
        .git(&["show", "-s", "--format=format:%ae", &status.commit_hash])
        .unwrap();
    assert_eq!(commit_email, "git@someemail.com");
}

#[test]
#[serial]
fn test_commit_signature_exclude_git_config() {
    let tmp = tempfile::tempdir().unwrap();
    let mut repo = util::git_repo::GitRepo::new(tmp.path().to_path_buf());
    repo.init();
    repo.set_config("user.name", "git-author");
    repo.set_config("user.email", "git@someemail.com");

    env::set_var("ENDUR_CONFIG_HOME", tmp.path());
    let mut endur_config = Config::empty();
    endur_config.commit_exclude_git_config = true;
    endur_config.save();

    repo.write_file("foo.txt");
    repo.commit_all();
    repo.change_file("foo.txt");
    let status = snapshots::capture(repo.dir.as_path()).unwrap().unwrap();

    let commit_author = repo
        .git(&["show", "-s", "--format=format:%an", &status.commit_hash])
        .unwrap();
    assert_eq!(commit_author, "endur");

    let commit_email = repo
        .git(&["show", "-s", "--format=format:%ae", &status.commit_hash])
        .unwrap();
    assert_eq!(commit_email, "endur@github.io");
}

#[test]
fn test_index_isolation() {
    let tmp = tempfile::tempdir().unwrap();
    let mut repo = repo_and_file!(tmp, "foo.txt");

    // Stage a change in the standard index
    repo.change_file("foo.txt");
    repo.git(&["add", "foo.txt"]).unwrap();

    // Now make another unstaged modification in the working tree
    repo.write_file("bar.txt");

    // Get the status of standard index before endur capture
    let git_status_before = repo.git(&["status", "--porcelain"]).unwrap();

    // Run endur capture
    let status = snapshots::capture(repo.dir.as_path()).unwrap().unwrap();
    assert_ne!(status.commit_hash, status.base_hash);

    // Get the status of standard index after endur capture
    let git_status_after = repo.git(&["status", "--porcelain"]).unwrap();

    // Assert that the standard git index status is completely unchanged
    assert_eq!(git_status_before, git_status_after);
}

#[test]
fn test_list_snapshots() {
    let tmp = tempfile::tempdir().unwrap();
    // Isolate the SQLite cache in a separate subdir so it doesn't pollute the git repo.
    let cache_dir = tmp.path().join("endur_cache");
    std::fs::create_dir_all(&cache_dir).unwrap();
    env::set_var("ENDUR_CACHE_HOME", &cache_dir);

    let repo_dir = tmp.path().join("repo");
    std::fs::create_dir_all(&repo_dir).unwrap();
    let mut repo = util::git_repo::GitRepo::new(repo_dir);
    repo.init();
    repo.write_file("foo.txt");
    repo.commit_all();

    repo.change_file("foo.txt");
    let status1 = snapshots::capture(repo.dir.as_path()).unwrap().unwrap();

    repo.change_file("foo.txt");
    let status2 = snapshots::capture(repo.dir.as_path()).unwrap().unwrap();

    let list = snapshots::list_snapshots(repo.dir.as_path(), true).unwrap();
    assert_eq!(list.len(), 2);

    // Newest first
    assert_eq!(list[0].commit_hash, status2.commit_hash);
    assert_eq!(list[1].commit_hash, status1.commit_hash);

    assert_eq!(list[0].files_changed, 1);
    assert_eq!(list[1].files_changed, 1);

    assert_eq!(list[0].message, "endur auto-backup");
    assert_eq!(list[1].message, "endur auto-backup");

    env::remove_var("ENDUR_CACHE_HOME");
}

#[test]
fn test_restore() {
    let tmp = tempfile::tempdir().unwrap();
    let mut repo = util::git_repo::GitRepo::new(tmp.path().to_path_buf());
    repo.init();
    repo.write_file("foo.txt");
    repo.commit_all();

    // First snapshot
    repo.change_file("foo.txt"); // content will be "change 1"
    let status1 = snapshots::capture(repo.dir.as_path()).unwrap().unwrap();

    // Second snapshot
    repo.change_file("foo.txt"); // content will be "change 2"
    let status2 = snapshots::capture(repo.dir.as_path()).unwrap().unwrap();

    // Dirty working directory modification
    let foo_path = repo.dir.join("foo.txt");
    std::fs::write(&foo_path, "dirty working tree").unwrap();

    // Restore to snapshot 1
    let changes = snapshots::restore(repo.dir.as_path(), &status1.commit_hash, None).unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].0, 'M');
    assert_eq!(changes[0].1, "foo.txt");

    // Verify content of foo.txt is "change 1"
    let content = std::fs::read_to_string(&foo_path).unwrap();
    assert_eq!(content, "change 1");

    // Restore to snapshot 2
    let changes2 = snapshots::restore(repo.dir.as_path(), &status2.commit_hash, None).unwrap();
    assert_eq!(changes2.len(), 1);
    assert_eq!(changes2[0].0, 'M');
    assert_eq!(changes2[0].1, "foo.txt");

    // Verify content of foo.txt is "change 2"
    let content2 = std::fs::read_to_string(&foo_path).unwrap();
    assert_eq!(content2, "change 2");
}

#[test]
fn test_discrete_restore() {
    let tmp = tempfile::tempdir().unwrap();
    let mut repo = util::git_repo::GitRepo::new(tmp.path().to_path_buf());
    repo.init();

    // Create two files
    repo.write_file("foo.txt");
    repo.write_file("bar.txt");
    repo.commit_all();

    // Modify both files and capture snapshot
    repo.change_file("foo.txt"); // content will be "change 1"
    repo.change_file("bar.txt"); // content will be "change 1"
    let status = snapshots::capture(repo.dir.as_path()).unwrap().unwrap();

    // Make both dirty in working tree
    let foo_path = repo.dir.join("foo.txt");
    let bar_path = repo.dir.join("bar.txt");
    std::fs::write(&foo_path, "dirty foo").unwrap();
    std::fs::write(&bar_path, "dirty bar").unwrap();

    // Restore only foo.txt
    let changes = snapshots::restore(
        repo.dir.as_path(),
        &status.commit_hash,
        Some(&["foo.txt".to_string()]),
    )
    .unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].1, "foo.txt");

    // Verify foo.txt is restored
    let content_foo = std::fs::read_to_string(&foo_path).unwrap();
    assert_eq!(content_foo, "change 1");

    // Verify bar.txt remains dirty (not restored)
    let content_bar = std::fs::read_to_string(&bar_path).unwrap();
    assert_eq!(content_bar, "dirty bar");
}

#[test]
fn test_get_snapshot_files() {
    let tmp = tempfile::tempdir().unwrap();
    let mut repo = repo_and_file!(tmp, "foo.txt");
    repo.write_file("bar.txt");
    repo.commit_all();

    repo.change_file("foo.txt");
    repo.write_file("new_file.txt");
    let status = snapshots::capture(repo.dir.as_path()).unwrap().unwrap();

    let files = snapshots::get_snapshot_files(repo.dir.as_path(), &status.commit_hash).unwrap();
    assert_eq!(files.len(), 2);
    let paths: Vec<String> = files.iter().map(|(_, p)| p.clone()).collect();
    assert!(paths.contains(&"foo.txt".to_string()));
    assert!(paths.contains(&"new_file.txt".to_string()));
}

use durable::{config::Config, snapshots};

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
    assert_eq!(status.durable_branch, format!("durable/{}", status.base_hash));
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

    // Regular durable commit
    assert_ne!(status.commit_hash, status.base_hash);
    assert_eq!(status.durable_branch, format!("durable/{}", status.base_hash));
}

#[test]
#[serial]
fn test_commit_signature_using_durable_config() {
    let tmp = tempfile::tempdir().unwrap();
    let mut repo = util::git_repo::GitRepo::new(tmp.path().to_path_buf());
    repo.init();
    repo.set_config("user.name", "git-author");
    repo.set_config("user.email", "git@someemail.com");

    env::set_var("DURABLE_CONFIG_HOME", tmp.path());
    let mut durable_config = Config::empty();
    durable_config.commit_author = Some("durable-config".to_string());
    durable_config.commit_email = Some("durable-config@email.com".to_string());
    durable_config.save();

    repo.write_file("foo.txt");
    repo.commit_all();

    repo.change_file("foo.txt");
    let status = snapshots::capture(repo.dir.as_path()).unwrap().unwrap();

    let commit_author = repo.git(&["show", "-s", "--format=format:%an", &status.commit_hash]);
    assert_eq!(commit_author, durable_config.commit_author);

    let commit_email = repo.git(&["show", "-s", "--format=format:%ae", &status.commit_hash]);
    assert_eq!(commit_email, durable_config.commit_email);
}

#[test]
#[serial]
fn test_commit_signature_using_git_config() {
    let tmp = tempfile::tempdir().unwrap();
    let mut repo = util::git_repo::GitRepo::new(tmp.path().to_path_buf());
    repo.init();
    repo.set_config("user.name", "git-author");
    repo.set_config("user.email", "git@someemail.com");

    env::set_var("DURABLE_CONFIG_HOME", tmp.path());
    let durable_config = Config::empty();
    durable_config.save();

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

    env::set_var("DURABLE_CONFIG_HOME", tmp.path());
    let mut durable_config = Config::empty();
    durable_config.commit_exclude_git_config = true;
    durable_config.save();

    repo.write_file("foo.txt");
    repo.commit_all();
    repo.change_file("foo.txt");
    let status = snapshots::capture(repo.dir.as_path()).unwrap().unwrap();

    let commit_author = repo
        .git(&["show", "-s", "--format=format:%an", &status.commit_hash])
        .unwrap();
    assert_eq!(commit_author, "durable");

    let commit_email = repo
        .git(&["show", "-s", "--format=format:%ae", &status.commit_hash])
        .unwrap();
    assert_eq!(commit_email, "durable@github.io");
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

    // Get the status of standard index before durable capture
    let git_status_before = repo.git(&["status", "--porcelain"]).unwrap();
    
    // Run durable capture
    let status = snapshots::capture(repo.dir.as_path()).unwrap().unwrap();
    assert_ne!(status.commit_hash, status.base_hash);

    // Get the status of standard index after durable capture
    let git_status_after = repo.git(&["status", "--porcelain"]).unwrap();
    
    // Assert that the standard git index status is completely unchanged
    assert_eq!(git_status_before, git_status_after);
}

#[test]
fn test_list_snapshots() {
    let tmp = tempfile::tempdir().unwrap();
    let mut repo = util::git_repo::GitRepo::new(tmp.path().to_path_buf());
    repo.init();
    repo.write_file("foo.txt");
    repo.commit_all();

    repo.change_file("foo.txt");
    let status1 = snapshots::capture(repo.dir.as_path()).unwrap().unwrap();

    repo.change_file("foo.txt");
    let status2 = snapshots::capture(repo.dir.as_path()).unwrap().unwrap();

    let list = snapshots::list_snapshots(repo.dir.as_path()).unwrap();
    assert_eq!(list.len(), 2);

    // Newest first
    assert_eq!(list[0].commit_hash, status2.commit_hash);
    assert_eq!(list[1].commit_hash, status1.commit_hash);

    assert_eq!(list[0].files_changed, 1);
    assert_eq!(list[1].files_changed, 1);

    assert_eq!(list[0].message, "durable auto-backup");
    assert_eq!(list[1].message, "durable auto-backup");
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
    let changes = snapshots::restore(repo.dir.as_path(), &status1.commit_hash).unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].0, 'M');
    assert_eq!(changes[0].1, "foo.txt");

    // Verify content of foo.txt is "change 1"
    let content = std::fs::read_to_string(&foo_path).unwrap();
    assert_eq!(content, "change 1");

    // Restore to snapshot 2
    let changes2 = snapshots::restore(repo.dir.as_path(), &status2.commit_hash).unwrap();
    assert_eq!(changes2.len(), 1);
    assert_eq!(changes2[0].0, 'M');
    assert_eq!(changes2[0].1, "foo.txt");

    // Verify content of foo.txt is "change 2"
    let content2 = std::fs::read_to_string(&foo_path).unwrap();
    assert_eq!(content2, "change 2");
}

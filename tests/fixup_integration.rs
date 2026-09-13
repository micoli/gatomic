//! Integration tests for file<->commit matching and the bulk triage flow,
//! against a real repo history (not hand-built commit graphs).

mod common;

use gatomic::fixup::{build_files_by_commit, match_files_to_commits};
use gatomic::git::{
    FileEntry, FileStatusKind, commit_fixup, commits_of_current_branch, stage_path,
};

use common::{Sandbox, sandbox_lock};

fn modified_entry(path: &str) -> FileEntry {
    FileEntry {
        path: path.to_string(),
        old_path: None,
        kind: FileStatusKind::Modified,
        staged: false,
        has_unstaged_changes: true,
    }
}

#[test]
fn file_touched_by_a_single_commit_is_evident_others_are_ambiguous() {
    let _guard = sandbox_lock();
    let sandbox = Sandbox::enter();

    sandbox.commit("a.txt", "a\n", "init a");
    sandbox.commit("b.txt", "b\n", "init b");
    sandbox.commit("shared.txt", "shared v1\n", "shared v1");
    sandbox.commit("shared.txt", "shared v2\n", "shared v2");

    let commits = commits_of_current_branch().unwrap();
    assert!(commits.len() >= 4);

    let files = vec![
        modified_entry("a.txt"),
        modified_entry("b.txt"),
        modified_entry("shared.txt"),
    ];

    let files_by_commit = build_files_by_commit(&commits).unwrap();
    let matches = match_files_to_commits(&files, &commits, &files_by_commit);

    let a_match = matches.iter().find(|m| m.file == "a.txt").unwrap();
    assert!(a_match.is_evident());

    let b_match = matches.iter().find(|m| m.file == "b.txt").unwrap();
    assert!(b_match.is_evident());
    assert_ne!(
        a_match.matching_commits[0].short_sha,
        b_match.matching_commits[0].short_sha
    );

    let shared_match = matches.iter().find(|m| m.file == "shared.txt").unwrap();
    assert!(
        !shared_match.is_evident(),
        "shared.txt was touched by two commits in range, should be ambiguous"
    );
}

#[test]
fn bulk_accepting_evident_matches_creates_one_fixup_commit_per_target() {
    let _guard = sandbox_lock();
    let sandbox = Sandbox::enter();

    sandbox.commit("a.txt", "a\n", "init a");
    sandbox.commit("b.txt", "b\n", "init b");

    // Simulate the working-tree modifications a user would then fix up.
    sandbox.write("a.txt", "a\nmodified\n");
    sandbox.write("b.txt", "b\nmodified\n");

    let commits = commits_of_current_branch().unwrap();
    let files = vec![modified_entry("a.txt"), modified_entry("b.txt")];
    let files_by_commit = build_files_by_commit(&commits).unwrap();
    let matches = match_files_to_commits(&files, &commits, &files_by_commit);
    assert_eq!(matches.len(), 2);
    assert!(matches.iter().all(|m| m.is_evident()));

    for m in &matches {
        stage_path(&m.file).unwrap();
        commit_fixup(&m.matching_commits[0].short_sha).unwrap();
    }

    let log = sandbox.git(&["log", "--oneline"]);
    let log_text = String::from_utf8_lossy(&log.stdout);
    assert_eq!(log_text.matches("fixup!").count(), 2);

    let status = sandbox.git(&["status", "--porcelain"]);
    assert!(
        String::from_utf8_lossy(&status.stdout).is_empty(),
        "both files should be fully staged and committed"
    );
}

#[test]
fn build_files_by_commit_maps_each_sha_to_its_changed_paths() {
    let _guard = sandbox_lock();
    let sandbox = Sandbox::enter();

    sandbox.commit("a.txt", "a\n", "init a");
    sandbox.commit("b.txt", "b\n", "init b");

    let commits = commits_of_current_branch().unwrap();
    let map = build_files_by_commit(&commits).unwrap();

    let init_a = commits.iter().find(|c| c.message == "init a").unwrap();
    let init_b = commits.iter().find(|c| c.message == "init b").unwrap();

    assert!(map[&init_a.short_sha].contains("a.txt"));
    assert!(!map[&init_a.short_sha].contains("b.txt"));
    assert!(map[&init_b.short_sha].contains("b.txt"));
}

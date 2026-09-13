//! End-to-end test against a real, temporary git repository with a large
//! number of files and commits — the same git-backed functions the TUI
//! calls (per CLAUDE.md: the TUI itself needs a real terminal, so it isn't
//! driven directly here), but at a scale meant to catch anything that only
//! shows up once the repo stops being a handful of files: O(n^2) diff-tree
//! calls, HashMap collisions, off-by-one batching, etc.

mod common;

use std::collections::HashMap;

use gatomic::diff::{apply_selection, count_hunks_per_file, load_file_diff};
use gatomic::fixup::{build_files_by_commit, match_files_to_commits};
use gatomic::git::{
    FileStatusKind, commit_fixup, commits_of_current_branch, list_file_entries,
    numstat_against_head, stage_path,
};
use gatomic::selection::FileSelection;

use common::{Sandbox, sandbox_lock};

const FILE_COUNT: usize = 150;
const SHARED_FILE_COUNT: usize = 5;

#[test]
fn large_number_of_files_are_matched_staged_and_fixed_up_independently() {
    let _guard = sandbox_lock();
    let sandbox = Sandbox::enter();

    // One commit per file: each of these is later an "evident" fixup
    // target since exactly one commit in the branch touched it.
    for i in 0..FILE_COUNT {
        let path = format!("src/module_{i:04}.rs");
        sandbox.commit(
            &path,
            &format!("fn f_{i}() {{}}\n"),
            &format!("add module {i}"),
        );
    }

    // A handful of files touched by two commits each: ambiguous, must never
    // be picked up as evident no matter how many unrelated commits surround
    // them in the branch.
    for i in 0..SHARED_FILE_COUNT {
        let path = format!("src/shared_{i:02}.rs");
        sandbox.commit(&path, "v1\n", &format!("shared {i} v1"));
        sandbox.commit(&path, "v1\nv2\n", &format!("shared {i} v2"));
    }

    let commits = commits_of_current_branch().unwrap();
    assert_eq!(commits.len(), FILE_COUNT + 2 * SHARED_FILE_COUNT);

    // Modify every tracked file in the working tree, as a user would before
    // triaging a large pile of changes back into the right commits.
    for i in 0..FILE_COUNT {
        let path = format!("src/module_{i:04}.rs");
        sandbox.write(&path, &format!("fn f_{i}() {{}}\nfn f_{i}_extra() {{}}\n"));
    }
    for i in 0..SHARED_FILE_COUNT {
        let path = format!("src/shared_{i:02}.rs");
        sandbox.write(&path, "v1\nv2\nv3\n");
    }

    // --- status/listing at scale ---------------------------------------

    let files = list_file_entries().unwrap();
    assert_eq!(
        files.len(),
        FILE_COUNT + SHARED_FILE_COUNT,
        "every modified file must be listed, none dropped or duplicated"
    );
    assert!(
        files.iter().all(|f| f.kind == FileStatusKind::Modified),
        "no file was newly added, all should report as Modified"
    );

    let numstat = numstat_against_head().unwrap();
    assert_eq!(numstat.len(), FILE_COUNT + SHARED_FILE_COUNT);
    let module_0_stat = numstat["src/module_0000.rs"];
    assert_eq!(module_0_stat, (1, 0), "one line appended, none removed");

    let hunk_counts = count_hunks_per_file(3).unwrap();
    assert_eq!(hunk_counts.len(), FILE_COUNT + SHARED_FILE_COUNT);
    assert!(
        hunk_counts.values().all(|&n| n == 1),
        "every file has a single contiguous appended-line change"
    );

    // --- evident matching at scale --------------------------------------

    let files_by_commit = build_files_by_commit(&commits).unwrap();
    let matches = match_files_to_commits(&files, &commits, &files_by_commit);
    assert_eq!(matches.len(), FILE_COUNT + SHARED_FILE_COUNT);

    let evident: Vec<_> = matches.iter().filter(|m| m.is_evident()).collect();
    let ambiguous: Vec<_> = matches.iter().filter(|m| !m.is_evident()).collect();
    assert_eq!(evident.len(), FILE_COUNT);
    assert_eq!(ambiguous.len(), SHARED_FILE_COUNT);

    // Every evident match must point at the one commit that actually
    // introduced that file, not some unrelated commit picked up by a
    // sloppy path comparison across hundreds of candidates.
    for m in &evident {
        let expected_message = format!(
            "add module {}",
            m.file
                .strip_prefix("src/module_")
                .unwrap()
                .strip_suffix(".rs")
                .unwrap()
                .parse::<usize>()
                .unwrap()
        );
        assert_eq!(m.matching_commits[0].message, expected_message);
    }
    for m in &ambiguous {
        assert_eq!(m.matching_commits.len(), 2);
    }

    // --- bulk fixup over every evident match -----------------------------

    let mut fixup_targets: HashMap<String, ()> = HashMap::new();
    for m in &evident {
        stage_path(&m.file).unwrap();
        commit_fixup(&m.matching_commits[0].short_sha).unwrap();
        fixup_targets.insert(m.matching_commits[0].short_sha.clone(), ());
    }
    assert_eq!(
        fixup_targets.len(),
        FILE_COUNT,
        "each module has a distinct target commit, so no two fixups collapse onto the same sha"
    );

    let log = sandbox.git(&["log", "--oneline"]);
    let log_text = String::from_utf8_lossy(&log.stdout);
    assert_eq!(log_text.matches("fixup!").count(), FILE_COUNT);

    let status = sandbox.git(&["status", "--porcelain"]);
    let status_text = String::from_utf8_lossy(&status.stdout);
    let remaining: Vec<&str> = status_text.lines().collect();
    assert_eq!(
        remaining.len(),
        SHARED_FILE_COUNT,
        "only the ambiguous shared files should still show as dirty"
    );
    assert!(remaining.iter().all(|line| line.contains("shared_")));

    // --- hunk-level staging still works once the ambiguous files are the
    // only thing left in a repo with hundreds of commits in its history ---

    let first_shared = "src/shared_00.rs";
    let diff = load_file_diff(first_shared, &FileStatusKind::Modified, 3).unwrap();
    assert_eq!(diff.hunks.len(), 1);
    let mut selection = FileSelection::all_undecided(first_shared, &diff);
    selection.accept_hunk(0);
    apply_selection(first_shared, &diff, &selection).unwrap();

    let staged = sandbox.git(&["diff", "--cached", "--", first_shared]);
    assert!(
        String::from_utf8_lossy(&staged.stdout).contains("v3"),
        "hunk selection must still stage correctly against a repo with {} commits",
        commits.len()
    );
}

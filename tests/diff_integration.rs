//! Integration tests exercising the diff-load / patch-apply pipeline against
//! real git repositories (not hand-written diff fixtures), so the parser and
//! patch reconstruction are validated against git's actual output format.

mod common;

use gatomic::diff::{apply_selection, load_file_diff};
use gatomic::git::FileStatusKind;
use gatomic::selection::FileSelection;

use common::{Sandbox, sandbox_lock};

#[test]
fn staging_one_of_two_separated_hunks_leaves_the_other_unstaged() {
    let _guard = sandbox_lock();
    let sandbox = Sandbox::enter();

    let original: String = (1..=20).map(|n| format!("{n}\n")).collect();
    sandbox.commit("f.txt", &original, "init");

    let mut lines: Vec<String> = (1..=20).map(|n| n.to_string()).collect();
    lines[1] = "CHANGED2".to_string();
    lines[17] = "CHANGED18".to_string();
    sandbox.write("f.txt", &(lines.join("\n") + "\n"));

    let diff = load_file_diff("f.txt", &FileStatusKind::Modified, 1).unwrap();
    assert_eq!(diff.hunks.len(), 2);

    let mut selection = FileSelection::all_selected("f.txt", &diff);
    selection.toggle_hunk(0); // keep only the second hunk

    apply_selection("f.txt", &diff, &selection).unwrap();

    let staged = sandbox.git(&["diff", "--cached"]);
    let staged_text = String::from_utf8_lossy(&staged.stdout);
    assert!(staged_text.contains("CHANGED18"));
    assert!(!staged_text.contains("CHANGED2\n"));

    let worktree = sandbox.git(&["diff"]);
    let worktree_text = String::from_utf8_lossy(&worktree.stdout);
    assert!(worktree_text.contains("CHANGED2"));
    assert!(!worktree_text.contains("CHANGED18"));
}

#[test]
fn line_level_selection_stages_only_the_chosen_change_in_a_hunk() {
    let _guard = sandbox_lock();
    let sandbox = Sandbox::enter();

    sandbox.commit("f.txt", "a\nb\nc\nd\ne\n", "init");
    sandbox.write("f.txt", "A\nb\nc\nd\nE\n");

    let diff = load_file_diff("f.txt", &FileStatusKind::Modified, 3).unwrap();
    assert_eq!(diff.hunks.len(), 1);

    let mut selection = FileSelection::all_selected("f.txt", &diff);
    // Lines: 0 "-a", 1 "+A", 2 " b", 3 " c", 4 " d", 5 "-e", 6 "+E".
    selection.toggle_line(0, 5);
    selection.toggle_line(0, 6);

    apply_selection("f.txt", &diff, &selection).unwrap();

    let staged = sandbox.git(&["diff", "--cached"]);
    let staged_text = String::from_utf8_lossy(&staged.stdout);
    assert!(staged_text.contains("+A"));
    assert!(!staged_text.contains("+E"));

    let worktree = sandbox.git(&["diff"]);
    let worktree_text = String::from_utf8_lossy(&worktree.stdout);
    assert!(worktree_text.contains("+E"));
    assert!(!worktree_text.contains("+A"));
}

#[test]
fn untracked_file_is_parsed_and_partially_staged_like_a_new_file() {
    let _guard = sandbox_lock();
    let sandbox = Sandbox::enter();

    sandbox.git(&["commit", "--allow-empty", "-q", "-m", "init"]);
    sandbox.write("new.txt", "x\ny\nz\n");

    let diff = load_file_diff("new.txt", &FileStatusKind::Untracked, 3).unwrap();
    assert!(diff.is_new_file);
    assert_eq!(diff.hunks.len(), 1);

    let mut selection = FileSelection::all_selected("new.txt", &diff);
    selection.toggle_line(0, 2); // drop "z"

    apply_selection("new.txt", &diff, &selection).unwrap();

    let staged = sandbox.git(&["diff", "--cached"]);
    let staged_text = String::from_utf8_lossy(&staged.stdout);
    assert!(staged_text.contains("+x"));
    assert!(staged_text.contains("+y"));
    assert!(!staged_text.contains("+z"));
}

#[test]
fn toggling_back_and_forth_is_idempotent() {
    let _guard = sandbox_lock();
    let sandbox = Sandbox::enter();

    sandbox.commit("f.txt", "one\ntwo\nthree\n", "init");
    sandbox.write("f.txt", "one\nTWO\nthree\n");

    let diff = load_file_diff("f.txt", &FileStatusKind::Modified, 3).unwrap();
    let mut selection = FileSelection::all_selected("f.txt", &diff);

    selection.toggle_hunk(0);
    apply_selection("f.txt", &diff, &selection).unwrap();
    let empty = sandbox.git(&["diff", "--cached"]);
    assert!(
        String::from_utf8_lossy(&empty.stdout).is_empty(),
        "nothing should be staged after deselecting the only hunk"
    );

    selection.toggle_hunk(0);
    apply_selection("f.txt", &diff, &selection).unwrap();
    let staged = sandbox.git(&["diff", "--cached"]);
    let staged_text = String::from_utf8_lossy(&staged.stdout);
    assert!(staged_text.contains("TWO"));
}

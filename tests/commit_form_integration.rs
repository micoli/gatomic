//! Integration tests for the "new commit" form's git-backed helpers:
//! reading the configured commit template and previewing what's staged.

mod common;

use gatomic::git::{commit_template, commit_with_message, stage_path, staged_diff};

use common::{Sandbox, sandbox_lock};

#[test]
fn commit_with_message_creates_a_plain_commit_from_staged_content() {
    let _guard = sandbox_lock();
    let sandbox = Sandbox::enter();

    sandbox.commit("a.txt", "a\n", "init a");
    sandbox.write("a.txt", "a\nmodified\n");
    stage_path("a.txt").unwrap();

    commit_with_message("manual commit message").unwrap();

    let log = sandbox.git(&["log", "-1", "--format=%s"]);
    assert_eq!(
        String::from_utf8_lossy(&log.stdout).trim(),
        "manual commit message"
    );

    let status = sandbox.git(&["status", "--porcelain"]);
    assert!(
        String::from_utf8_lossy(&status.stdout).is_empty(),
        "the staged change should now be committed"
    );
}

#[test]
fn commit_with_message_strips_comment_lines_like_the_editor_flow_would() {
    let _guard = sandbox_lock();
    let sandbox = Sandbox::enter();

    sandbox.commit("a.txt", "a\n", "init a");
    sandbox.write("a.txt", "a\nmodified\n");
    stage_path("a.txt").unwrap();

    let message = "Subject line\n\
        \n\
        Body line\n\
        # Please enter the commit message for your changes. Lines starting\n\
        # with '#' will be ignored, and an empty message aborts the commit.\n\
        #\n\
        # vi:ft=gitcommit:tw=72:sw=2:ts=2:expandtab:spell\n";
    commit_with_message(message).unwrap();

    let log = sandbox.git(&["log", "-1", "--format=%B"]);
    let committed = String::from_utf8_lossy(&log.stdout);
    assert_eq!(committed.trim_end(), "Subject line\n\nBody line");
    assert!(
        !committed.contains('#'),
        "template comment lines must not end up in the commit message: {committed:?}"
    );
}

#[test]
fn commit_template_is_none_when_unconfigured() {
    let _guard = sandbox_lock();
    let _sandbox = Sandbox::enter();

    assert!(commit_template().unwrap().is_none());
}

#[test]
fn commit_template_reads_the_configured_file() {
    let _guard = sandbox_lock();
    let sandbox = Sandbox::enter();

    let template_path = sandbox.dir.join(".gitmessage");
    std::fs::write(&template_path, "Subject\n\nBody line\n").unwrap();
    sandbox.git(&["config", "commit.template", template_path.to_str().unwrap()]);

    let template = commit_template().unwrap().unwrap();
    assert_eq!(template, "Subject\n\nBody line\n");
}

#[test]
fn commit_template_is_none_when_configured_file_is_missing() {
    let _guard = sandbox_lock();
    let sandbox = Sandbox::enter();

    sandbox.git(&["config", "commit.template", "does-not-exist.txt"]);

    assert!(commit_template().unwrap().is_none());
}

#[test]
fn commit_template_expands_a_home_relative_path() {
    let _guard = sandbox_lock();
    let sandbox = Sandbox::enter();

    let previous_home = std::env::var("HOME").ok();
    // SAFETY: guarded by sandbox_lock like Sandbox's own env var changes,
    // restored before the test returns.
    unsafe {
        std::env::set_var("HOME", &sandbox.dir);
    }
    std::fs::write(sandbox.dir.join(".gitmessage"), "Templated subject\n").unwrap();
    sandbox.git(&["config", "commit.template", "~/.gitmessage"]);

    let result = commit_template();

    unsafe {
        match &previous_home {
            Some(home) => std::env::set_var("HOME", home),
            None => std::env::remove_var("HOME"),
        }
    }

    assert_eq!(result.unwrap().unwrap(), "Templated subject\n");
}

#[test]
fn staged_diff_is_empty_when_nothing_is_staged() {
    let _guard = sandbox_lock();
    let sandbox = Sandbox::enter();
    sandbox.commit("a.txt", "a\n", "init a");

    assert!(staged_diff().unwrap().is_empty());
}

#[test]
fn staged_diff_shows_staged_content_but_not_unstaged_changes() {
    let _guard = sandbox_lock();
    let sandbox = Sandbox::enter();
    sandbox.commit("a.txt", "a\n", "init a");

    sandbox.write("a.txt", "a\nstaged\n");
    sandbox.git(&["add", "a.txt"]);
    sandbox.write("a.txt", "a\nstaged\nunstaged\n");

    let diff = staged_diff().unwrap();
    assert!(diff.contains("+staged"));
    assert!(!diff.contains("+unstaged"));
}

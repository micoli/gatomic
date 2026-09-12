//! CLI-level tests spawning the compiled binary. The interactive TUI itself
//! is exercised through the library-level integration tests instead
//! (see `diff_integration.rs`), since it needs a real terminal.

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_gatomic");

#[test]
fn help_documents_last_commits_flag() {
    let output = Command::new(BIN).arg("--help").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--last-commits"));
    assert!(stdout.contains("-n"));
}

#[test]
fn rejects_unknown_flags() {
    let output = Command::new(BIN).arg("--not-a-real-flag").output().unwrap();
    assert!(!output.status.success());
}

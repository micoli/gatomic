pub mod commit;
pub mod repo;
pub mod status;

pub use commit::{
    CommitInfo, commit_fixup, commit_template, commit_with_message, commits_by_shas,
    commits_of_current_branch, commits_since, files_changed_in_commits, filter_fixup_targets,
    last_n_commits,
};
pub use repo::{
    NULL_DEVICE, cd_to_repo_root, run_git, run_git_allowing, run_git_with_stdin, stage_path,
    unstage_path,
};
pub use status::{FileEntry, FileStatusKind, list_file_entries, numstat_against_head, staged_diff};

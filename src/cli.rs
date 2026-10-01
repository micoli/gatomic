use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "gatomic",
    about = "TUI to build atomic fixup commits with fine-grained hunk/line selection"
)]
pub struct Cli {
    /// Number of most recent commits to show as fixup targets.
    /// If omitted, only commits unique to the current branch are shown.
    /// `fixup! ` commits are excluded; the list is resolved once at startup.
    #[arg(short = 'n', long = "last-commits")]
    pub last_commits: Option<usize>,

    /// Show commits in `<REF>..HEAD` (sha, tag, branch or remote ref) as
    /// fixup targets. Resolved once at startup. Conflicts with `-n`.
    #[arg(
        long = "rebase-origin",
        value_name = "REF",
        conflicts_with = "last_commits"
    )]
    pub rebase_origin: Option<String>,

    /// Number of context lines requested from `git diff`
    #[arg(long, default_value_t = 3)]
    pub context_lines: u32,
}

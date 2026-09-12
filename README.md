# gatomic

`gatomic` is a terminal UI to build atomic `git commit --fixup` commits: pick
exactly the hunks — or individual lines — to stage from your working tree,
then attach them to the right past commit, without leaving the TUI.

It is the spiritual successor of `git-helper` (`gitHelper`), which already
helped find the right commit to fix up but delegated hunk selection to a
shell-out to the real `git add -p`. `gatomic` integrates that selection
natively.

## Layout

```
┌───────────────────────┬───────────────┐
│ Files (modified/new)  │               │
├───────────────────────┤   Commits     │
│ Hunks / lines          │  (last N or  │
│ (space to toggle,      │   branch-only│
│  staged immediately)   │   by default)│
└───────────────────────┴───────────────┘
```

- **Files** (top-left): modified, staged, and untracked files.
- **Hunks** (bottom-left): unified-diff hunks and lines for the selected
  file. `Space`/`Enter` toggles a hunk header or an individual added/removed
  line; the selection is staged immediately via `git apply --cached`.
- **Commits** (right): the last `-n` commits, or — if `-n` is omitted — only
  the commits unique to the current branch (relative to its upstream, or
  `origin/HEAD`/`origin/main`/`origin/master` as a fallback).

## Usage

```sh
gatomic              # commits unique to the current branch
gatomic -n 30        # 30 most recent commits on HEAD
```

Keys: `Tab`/`Shift+Tab` cycle panes, arrow keys (or `j`/`k`) navigate,
`Space`/`Enter` toggles a hunk/line, `f` (in the Commits pane) runs
`git commit --fixup <sha>` on the currently staged content, `q` quits.

## Development

```sh
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt
bash tests/fixtures/init-test-repository.sh basic t   # demo repo in tmp/t
```

`gatomic` shells out to the `git` binary found on `PATH` (no `git2`
dependency), so it works the same way with system git on Linux, Homebrew
git on macOS, and Git for Windows.

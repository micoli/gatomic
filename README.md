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
┌────────────────────────┬────────────────┐
│ Files (modified/new)   │    Commits     │
├────────────────────────┤ (last N, or    │
│ Hunks / lines          │ branch-only    │
│ (space to toggle,      │ by default)    │
│  staged immediately)   ├────────────────┤
│                         │   git show     │
│                         │ of selected    │
│                         │    commit      │
└────────────────────────┴────────────────┘
│               status / help bar          │
└───────────────────────────────────────────┘
```

- **Files** (top-left): modified, staged, and untracked files.
- **Hunks** (bottom-left): unified-diff hunks and lines for the selected
  file, with a `git add -p`-style guided review (see Keys below). The
  selection is staged immediately via `git apply --cached` on every
  decision.
- **Commits** (top-right): the last `-n` commits, or — if `-n` is omitted —
  only the commits unique to the current branch (relative to its upstream,
  or `origin/HEAD`/`origin/main`/`origin/master` as a fallback).
- **git show** (bottom-right): the full `git show` of whichever commit is
  currently selected in the Commits pane, scrollable.

## Triage screen

At startup, if any modified file was touched by exactly one commit among
the ones currently listed in the Commits pane, gatomic shows a triage
screen first: a checklist of those "evident" file→commit associations,
pre-checked, next to a diff of the selected file and a `git show` of its
target commit. `Tab` cycles focus between the three panes, `Space` toggles
the checked file (or every file under a commit), `a` stages each checked
file in full and creates one `git commit --fixup <sha>` per target commit
(grouping files that target the same commit), then drops into the normal
review layout for whatever is left. `Enter`/`d`/`Esc` skips the triage
entirely for the session. If no file has an evident single-commit match,
gatomic starts directly in the review layout.

## Usage

```sh
gatomic              # commits unique to the current branch
gatomic -n 30        # 30 most recent commits on HEAD
```

### Keys

Global: `Tab`/`Shift+Tab` cycle panes, arrow keys navigate, `c` opens a form
to create a plain (non-fixup) commit from whatever is currently staged,
`l` cycles the UI language (English/French), `q`/`Esc`/`Ctrl+C` quit, `?`
shows a contextual help popup (any key closes it).

### Mouse

Clicking a pane focuses it and selects the row under the cursor; clicking a
hunk or a triage checkbox toggles it directly. The scroll wheel navigates
whichever pane is under the pointer (or scrolls the triage preview panes).

Files pane: `t` reopens the triage screen on demand, recomputing evident
matches for the files still around.

Hunks pane — free navigation (works on any row): `Space`/`Enter` toggles
the hunk or line under the cursor.

Hunks pane — guided review, `git add -p`-style (acts on the current hunk,
cursor jumps to a hunk header):
- `y` / `n`: accept / reject this hunk and move to the next one.
- `a` / `d`: accept / reject this hunk and every remaining hunk in the
  file, then jump to the next file.
- `j` / `k`: next / previous hunk, without deciding.
- `J` / `K`: next / previous **undecided** hunk (wraps around).
- `s`: split the current hunk into smaller ones, when it contains more
  than one change block with enough context between them.

Commits pane: `x` runs `git commit --fixup <sha>` on the currently staged
content; `PageUp`/`PageDown` scroll the git show pane below it.

## Language

The UI defaults to English. Press `l` at any time (review or triage screen)
to cycle to another embedded language — currently English and French.
Translations live one file per language under `src/i18n/` (`en.rs`,
`fr.rs`); adding a language means adding a new file there.

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

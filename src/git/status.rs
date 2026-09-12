use anyhow::Result;

use super::repo::run_git;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileStatusKind {
    Modified,
    Added,
    Deleted,
    Renamed,
    Untracked,
    Conflicted,
}

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: String,
    pub old_path: Option<String>,
    pub kind: FileStatusKind,
    pub staged: bool,
    pub has_unstaged_changes: bool,
}

pub fn list_file_entries() -> Result<Vec<FileEntry>> {
    let raw = run_git(&["status", "--porcelain=v1", "-z"])?;
    Ok(parse_porcelain_status(&raw))
}

fn parse_porcelain_status(raw: &str) -> Vec<FileEntry> {
    let mut fields = raw.split('\0').filter(|f| !f.is_empty());
    let mut entries = Vec::new();

    while let Some(record) = fields.next() {
        let Some((code, path)) = record.split_at_checked(2).map(|(c, p)| (c, p.trim_start()))
        else {
            continue;
        };
        let index_status = code.as_bytes()[0] as char;
        let worktree_status = code.as_bytes()[1] as char;

        let old_path = if index_status == 'R' || index_status == 'C' {
            fields.next().map(str::to_string)
        } else {
            None
        };

        entries.push(FileEntry {
            path: path.to_string(),
            old_path,
            kind: classify(index_status, worktree_status),
            staged: index_status != ' ' && index_status != '?',
            has_unstaged_changes: worktree_status != ' ' && worktree_status != '?',
        });
    }

    entries
}

fn classify(index_status: char, worktree_status: char) -> FileStatusKind {
    if index_status == '?' && worktree_status == '?' {
        return FileStatusKind::Untracked;
    }
    if index_status == 'U' || worktree_status == 'U' {
        return FileStatusKind::Conflicted;
    }
    if index_status == 'R' || index_status == 'C' {
        return FileStatusKind::Renamed;
    }
    if index_status == 'A' {
        return FileStatusKind::Added;
    }
    if index_status == 'D' || worktree_status == 'D' {
        return FileStatusKind::Deleted;
    }
    FileStatusKind::Modified
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_untracked_files() {
        let raw = "?? a.txt\0?? c.txt\0";
        let entries = parse_porcelain_status(raw);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].path, "a.txt");
        assert_eq!(entries[0].kind, FileStatusKind::Untracked);
        assert!(!entries[0].staged);
    }

    #[test]
    fn parses_rename_with_two_paths() {
        let raw = "RM b.txt\0a.txt\0";
        let entries = parse_porcelain_status(raw);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, "b.txt");
        assert_eq!(entries[0].old_path.as_deref(), Some("a.txt"));
        assert_eq!(entries[0].kind, FileStatusKind::Renamed);
        assert!(entries[0].staged);
        assert!(entries[0].has_unstaged_changes);
    }

    #[test]
    fn parses_modified_and_added() {
        let raw = " M modified.txt\0A  added.txt\0";
        let entries = parse_porcelain_status(raw);
        assert_eq!(entries[0].kind, FileStatusKind::Modified);
        assert!(!entries[0].staged);
        assert!(entries[0].has_unstaged_changes);
        assert_eq!(entries[1].kind, FileStatusKind::Added);
        assert!(entries[1].staged);
    }
}

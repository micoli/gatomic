#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Context,
    Added,
    Removed,
}

#[derive(Debug, Clone)]
pub struct DiffLine {
    pub kind: LineKind,
    pub content: String,
    pub old_lineno: Option<u32>,
    pub new_lineno: Option<u32>,
    /// True when this line is immediately followed by a
    /// `\ No newline at end of file` marker in the source diff.
    pub no_newline_at_eof: bool,
}

#[derive(Debug, Clone)]
pub struct Hunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone)]
pub struct FileDiff {
    pub old_path: String,
    pub new_path: String,
    pub is_new_file: bool,
    pub is_deleted_file: bool,
    pub is_binary: bool,
    pub hunks: Vec<Hunk>,
    pub raw_header_lines: Vec<String>,
}

impl FileDiff {
    pub fn display_path(&self) -> &str {
        if self.is_deleted_file {
            &self.old_path
        } else {
            &self.new_path
        }
    }
}

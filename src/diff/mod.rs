pub mod load;
pub mod model;
pub mod parser;
pub mod patch;

pub use load::load_file_diff;
pub use model::{DiffLine, FileDiff, Hunk, LineKind};
pub use parser::parse_file_diff;
pub use patch::{apply_selection, build_patch};

//! Bounded raw content reads and IDEA-style two-sided, zero-based half-open diffs.
mod compare;
mod content;

pub use compare::{
    compare, Alignment, ChangeBlock, ChangeKind, CompareOptions, DiffResult, FoldRange,
    InlineRange, LineRange, Whitespace,
};
pub use content::{
    classify, read_content, Content, ContentSource, DiffError, ReadLimits, DEFAULT_MAX_BYTES,
};

mod patch;
pub use patch::{selected_text, unified_file_patch, unified_patch, PatchFile};

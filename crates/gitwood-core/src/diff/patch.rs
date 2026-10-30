//! HEAD-relative patches from the exact comparison shown to the user.
use super::{ChangeBlock, DiffError};
/// Keep unselected blocks from the left and selected blocks from the right.
pub fn selected_text(
    left: &str,
    right: &str,
    blocks: &[ChangeBlock],
    selected: &[usize],
) -> Result<String, DiffError> {
    if selected.iter().any(|i| *i >= blocks.len()) {
        return Err(DiffError::InvalidSource("stale block selection".into()));
    }
    let l: Vec<_> = left.split_inclusive('\n').collect();
    let r: Vec<_> = right.split_inclusive('\n').collect();
    let mut text = String::new();
    let mut end = 0;
    for (i, b) in blocks.iter().enumerate() {
        if b.left.start < end || b.left.end > l.len() || b.right.end > r.len() {
            return Err(DiffError::InvalidSource("invalid comparison ranges".into()));
        }
        text.extend(l[end..b.left.start].iter().copied());
        text.extend(
            if selected.contains(&i) {
                &r[b.right.start..b.right.end]
            } else {
                &l[b.left.start..b.left.end]
            }
            .iter()
            .copied(),
        );
        end = b.left.end;
    }
    text.extend(l[end..].iter().copied());
    Ok(text)
}
/// Quote every path byte using Git's C-style pathname syntax (including non UTF-8).
fn quoted(prefix: &[u8], path: &[u8]) -> String {
    let mut s = String::from("\"");
    for b in prefix.iter().chain(path) {
        s.push_str(&format!("\\{b:03o}"));
    }
    s.push('"');
    s
}
pub fn unified_patch(
    path: &[u8],
    left: &str,
    right: &str,
    blocks: &[ChangeBlock],
    selected: &[usize],
) -> Result<Vec<u8>, DiffError> {
    unified_file_patch(
        PatchFile {
            old_path: path,
            new_path: path,
            old_mode: "100644",
            new_mode: "100644",
        },
        left,
        right,
        blocks,
        selected,
    )
}
/// File metadata is resolved from Git status in Rust, never supplied by the UI.
pub struct PatchFile<'a> {
    pub old_path: &'a [u8],
    pub new_path: &'a [u8],
    pub old_mode: &'a str,
    pub new_mode: &'a str,
}
pub fn unified_file_patch(
    file: PatchFile<'_>,
    left: &str,
    right: &str,
    blocks: &[ChangeBlock],
    selected: &[usize],
) -> Result<Vec<u8>, DiffError> {
    if !["100644", "100755"].contains(&file.old_mode)
        || !["100644", "100755"].contains(&file.new_mode)
    {
        return Err(DiffError::InvalidSource(
            "partial patches require regular files".into(),
        ));
    }
    let selected = selected_text(left, right, blocks, selected)?;
    let renamed = file.old_path != file.new_path;
    let mode_changed = file.old_mode != file.new_mode;
    if selected == left && !renamed && !mode_changed {
        return Ok(vec![]);
    }
    let a = quoted(b"a/", file.old_path);
    let b = quoted(b"b/", file.new_path);
    let mut patch = format!("diff --git {a} {b}\n");
    if mode_changed {
        patch.push_str(&format!(
            "old mode {}\nnew mode {}\n",
            file.old_mode, file.new_mode
        ));
    }
    if renamed {
        patch.push_str(&format!(
            "rename from {}\nrename to {}\n",
            quoted(b"", file.old_path),
            quoted(b"", file.new_path)
        ));
    }
    // similar emits canonical context hunks, CRLF and the no-final-newline marker.
    patch.push_str(
        &similar::TextDiff::from_lines(left, &selected)
            .unified_diff()
            .context_radius(3)
            .header(&a, &b)
            .to_string(),
    );
    Ok(patch.into_bytes())
}

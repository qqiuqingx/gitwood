use serde::{Deserialize, Serialize};
use similar::{ChangeTag, DiffTag, TextDiff};

/// All line intervals are zero-based, half-open; an empty range is an insertion anchor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineRange {
    pub start: usize,
    pub end: usize,
}
impl From<std::ops::Range<usize>> for LineRange {
    fn from(r: std::ops::Range<usize>) -> Self {
        Self {
            start: r.start,
            end: r.end,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChangeKind {
    Insert,
    Delete,
    Modify,
}
/// UTF-16 columns within a logical line, excluding CR/LF. Compatible with CodeMirror.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InlineRange {
    pub line: usize,
    pub start: usize,
    pub end: usize,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeBlock {
    pub kind: ChangeKind,
    pub left: LineRange,
    pub right: LineRange,
    pub left_inline: Vec<InlineRange>,
    pub right_inline: Vec<InlineRange>,
}
/// Piecewise line-boundary mapping. Equal segments map 1:1; changed boundaries support offset/clamp mapping.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Alignment {
    pub left: LineRange,
    pub right: LineRange,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FoldRange {
    pub left: LineRange,
    pub right: LineRange,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffResult {
    pub blocks: Vec<ChangeBlock>,
    pub alignment: Vec<Alignment>,
    pub folds: Vec<FoldRange>,
    pub left_lines: usize,
    pub right_lines: usize,
}
/// Reserved policy boundary. Unsupported ignore modes are not silently accepted.
#[derive(Clone, Copy, Debug, Default)]
pub enum Whitespace {
    #[default]
    Exact,
}
#[derive(Clone, Copy, Debug)]
pub struct CompareOptions {
    pub context_lines: usize,
    pub whitespace: Whitespace,
}
impl Default for CompareOptions {
    // IDEA TextDiffSettingsHolder.kt PlaceSettings.CONTEXT_RANGE = 4 (0698a5fe).
    fn default() -> Self {
        Self {
            context_lines: 4,
            whitespace: Whitespace::Exact,
        }
    }
}

pub fn compare(left: &str, right: &str, options: CompareOptions) -> DiffResult {
    let left_lines: Vec<_> = left.split_inclusive('\n').collect();
    let right_lines: Vec<_> = right.split_inclusive('\n').collect();
    // Shared comparison deadline prevents adversarial all-different files from
    // monopolizing the worker. similar approximates with valid, coarser blocks.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
    let diff = TextDiff::configure()
        .deadline(deadline)
        .diff_slices(&left_lines, &right_lines);
    let mut result = DiffResult {
        blocks: vec![],
        alignment: vec![],
        folds: vec![],
        left_lines: left_lines.len(),
        right_lines: right_lines.len(),
    };
    for op in diff.ops() {
        let l = op.old_range();
        let r = op.new_range();
        result.alignment.push(Alignment {
            left: l.clone().into(),
            right: r.clone().into(),
        });
        if op.tag() == DiffTag::Equal {
            let before = if l.start == 0 {
                0
            } else {
                options.context_lines
            };
            let after = if l.end == left_lines.len() && r.end == right_lines.len() {
                0
            } else {
                options.context_lines
            };
            if l.len() > before.saturating_add(after).saturating_add(1) {
                result.folds.push(FoldRange {
                    left: (l.start + before..l.end - after).into(),
                    right: (r.start + before..r.end - after).into(),
                });
            }
            continue;
        }
        let mut block = ChangeBlock {
            kind: match op.tag() {
                DiffTag::Insert => ChangeKind::Insert,
                DiffTag::Delete => ChangeKind::Delete,
                _ => ChangeKind::Modify,
            },
            left: l.clone().into(),
            right: r.clone().into(),
            left_inline: vec![],
            right_inline: vec![],
        };
        let old = left_lines[l.clone()].concat().replace("\r\n", "\n");
        let new = right_lines[r.clone()].concat().replace("\r\n", "\n");
        let words = TextDiff::configure()
            .deadline(deadline)
            .diff_unicode_words(old.as_str(), new.as_str());
        let mut lp = (l.start, 0);
        let mut rp = (r.start, 0);
        for change in words.iter_all_changes() {
            match change.tag() {
                ChangeTag::Equal => {
                    advance(change.value(), &mut lp, false, &mut block.left_inline);
                    advance(change.value(), &mut rp, false, &mut block.right_inline);
                }
                ChangeTag::Delete => advance(change.value(), &mut lp, true, &mut block.left_inline),
                ChangeTag::Insert => {
                    advance(change.value(), &mut rp, true, &mut block.right_inline)
                }
            }
        }
        result.blocks.push(block);
    }
    result
}
fn advance(
    value: &str,
    position: &mut (usize, usize),
    highlight: bool,
    output: &mut Vec<InlineRange>,
) {
    for ch in value.chars() {
        if ch == '\n' {
            position.0 += 1;
            position.1 = 0;
        } else {
            let end = position.1 + ch.len_utf16();
            if highlight {
                if let Some(last) = output
                    .last_mut()
                    .filter(|last| last.line == position.0 && last.end == position.1)
                {
                    last.end = end;
                } else {
                    output.push(InlineRange {
                        line: position.0,
                        start: position.1,
                        end,
                    });
                }
            }
            position.1 = end;
        }
    }
}

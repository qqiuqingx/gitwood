//! Line-based three-way merge. Offsets crossing IPC are UTF-16 (CodeMirror positions).
use serde::Serialize;
use similar::{DiffTag, TextDiff};
use std::ops::Range;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeBlock {
    pub from: usize,
    pub to: usize,
    pub left: String,
    pub right: String,
    pub base: String,
}
#[derive(Debug)]
struct Edit {
    range: Range<usize>,
    text: String,
    left: bool,
}

pub fn merge(base: &str, left: &str, right: &str) -> (String, Vec<MergeBlock>) {
    let lines: Vec<_> = base.split_inclusive('\n').collect();
    let mut edits = edits(&lines, left, true);
    edits.extend(self::edits(&lines, right, false));
    edits.sort_by_key(|e| (e.range.start, e.range.end));
    let mut result = String::new();
    let mut blocks = Vec::new();
    let mut cursor = 0;
    let mut index = 0;
    while index < edits.len() {
        let start = edits[index].range.start;
        let mut end = edits[index].range.end;
        let first = index;
        index += 1;
        // Include boundary insertions; treating adjacent changes conservatively avoids
        // silently selecting an ordering for insert/delete at the same boundary.
        while index < edits.len()
            && (edits[index].range.start < end
                || (edits[index].range.start == end
                    && (edits[index].range.is_empty() || start == end)))
        {
            end = end.max(edits[index].range.end);
            index += 1;
        }
        result.extend(lines[cursor..start].iter().copied());
        let group = &edits[first..index];
        let original = lines[start..end].concat();
        let l = apply(&lines, start..end, group, true);
        let r = apply(&lines, start..end, group, false);
        if l == r || r == original {
            result.push_str(&l);
        } else if l == original {
            result.push_str(&r);
        } else {
            let from = result.encode_utf16().count();
            result.push_str(&original);
            blocks.push(MergeBlock {
                from,
                to: result.encode_utf16().count(),
                left: l,
                right: r,
                base: original,
            });
        }
        cursor = end;
    }
    result.extend(lines[cursor..].iter().copied());
    (result, blocks)
}
fn edits(base: &[&str], side: &str, left: bool) -> Vec<Edit> {
    let side: Vec<_> = side.split_inclusive('\n').collect();
    TextDiff::configure()
        .timeout(std::time::Duration::from_millis(500))
        .diff_slices(base, &side)
        .ops()
        .iter()
        .filter(|op| op.tag() != DiffTag::Equal)
        .map(|op| Edit {
            range: op.old_range(),
            text: side[op.new_range()].concat(),
            left,
        })
        .collect()
}
fn apply(lines: &[&str], range: Range<usize>, edits: &[Edit], left: bool) -> String {
    let mut text = String::new();
    let mut cursor = range.start;
    for edit in edits.iter().filter(|e| e.left == left) {
        text.extend(lines[cursor..edit.range.start].iter().copied());
        text.push_str(&edit.text);
        cursor = edit.range.end;
    }
    text.extend(lines[cursor..range.end].iter().copied());
    text
}

/// Unknown encodings are never decoded lossily and cannot be saved by the text editor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    Utf8,
    Bom,
    Le,
    Be,
}
pub fn decode(bytes: &[u8]) -> Option<(String, Encoding)> {
    if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
        let le = bytes[0] == 0xff;
        if !bytes.len().is_multiple_of(2) {
            return None;
        }
        let units: Vec<_> = bytes[2..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| {
                if le {
                    u16::from_le_bytes([b[0], b[1]])
                } else {
                    u16::from_be_bytes([b[0], b[1]])
                }
            })
            .collect();
        return String::from_utf16(&units)
            .ok()
            .filter(|s| !s.contains('\0'))
            .map(|s| (s, if le { Encoding::Le } else { Encoding::Be }));
    }
    let (bytes, encoding) = bytes
        .strip_prefix(&[0xef, 0xbb, 0xbf])
        .map_or((bytes, Encoding::Utf8), |b| (b, Encoding::Bom));
    std::str::from_utf8(bytes)
        .ok()
        .filter(|s| !s.contains('\0'))
        .map(|s| (s.into(), encoding))
}
pub fn encode(text: &str, encoding: Encoding) -> Vec<u8> {
    match encoding {
        Encoding::Utf8 => text.as_bytes().to_vec(),
        Encoding::Bom => [b"\xef\xbb\xbf", text.as_bytes()].concat(),
        Encoding::Le | Encoding::Be => {
            let mut bytes = if encoding == Encoding::Le {
                vec![0xff, 0xfe]
            } else {
                vec![0xfe, 0xff]
            };
            for unit in text.encode_utf16() {
                bytes.extend(if encoding == Encoding::Le {
                    unit.to_le_bytes()
                } else {
                    unit.to_be_bytes()
                });
            }
            bytes
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disjoint_and_adjacent_edits_apply_without_conflicts() {
        assert_eq!(
            merge("a\nb\nc\n", "left\nb\nc\n", "a\nright\nc\n").0,
            "left\nright\nc\n"
        );
        assert!(merge("a\nb\nc\n", "left\nb\nc\n", "a\nright\nc\n")
            .1
            .is_empty());
    }
    #[test]
    fn repeated_lines_deletions_and_utf16_positions() {
        let (text, blocks) = merge("😀\nbase\nend", "😀\nleft\nend", "😀\nright\nend");
        assert_eq!(text, "😀\nbase\nend");
        assert_eq!((blocks[0].from, blocks[0].to), (3, 8));
        assert_eq!(merge("a\nb\nc", "a\nc", "a\nb\nright").0, "a\nright");
        assert_eq!(merge("x\nx\nx\n", "l\nx\nx\n", "x\nx\nr\n").0, "l\nx\nr\n");
    }
    #[test]
    fn utf16_and_bom_roundtrip_without_loss() {
        for encoding in [Encoding::Utf8, Encoding::Bom, Encoding::Le, Encoding::Be] {
            let bytes = encode("😀\r\n中文", encoding);
            let (text, actual) = decode(&bytes).unwrap();
            assert_eq!(actual, encoding);
            assert_eq!(encode(&text, actual), bytes);
        }
        assert!(decode(&[0xff, 0x00]).is_none());
    }
}

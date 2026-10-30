use super::{invalid, text, RepoPath};
use crate::git::{GitError, GitRunner, ReadCommand, ReadOperation, RunOptions};
use serde::Serialize;
#[derive(Debug, Default, Serialize)]
pub struct Branch {
    pub oid: Option<String>,
    pub head: Option<String>,
    pub upstream: Option<String>,
    pub ahead: u64,
    pub behind: u64,
}
#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    pub kind: char,
    pub xy: String,
    pub submodule: String,
    pub path: RepoPath,
    pub original: Option<RepoPath>,
    pub similarity: Option<String>,
    /// Modes and object IDs, including all three stages for unmerged entries.
    pub metadata: Vec<String>,
}
impl Entry {
    pub fn is_rename(&self) -> bool {
        self.kind == '2' && self.similarity.as_ref().is_some_and(|s| s.starts_with('R'))
    }
}
#[derive(Debug, Default, Serialize)]
pub struct Status {
    pub branch: Branch,
    pub staged: Vec<Entry>,
    pub unstaged: Vec<Entry>,
    pub untracked: Vec<Entry>,
    pub ignored: Vec<Entry>,
    pub conflicts: Vec<Entry>,
}
pub async fn read(r: &GitRunner, o: &RunOptions) -> Result<Status, GitError> {
    parse(
        &r.run(
            ReadCommand::new(ReadOperation::Status).args([
                "--porcelain=v2",
                "-z",
                "--branch",
                "--untracked-files=all",
                "--ignored=matching",
            ]),
            o,
        )
        .await?
        .stdout,
    )
}
pub fn parse(bytes: &[u8]) -> Result<Status, GitError> {
    let mut result = Status::default();
    let mut records = bytes.split(|b| *b == 0);
    while let Some(record) = records.next() {
        if record.is_empty() {
            continue;
        }
        if record.starts_with(b"# ") {
            let (key, value) = record[2..]
                .split_once_byte(b' ')
                .ok_or_else(|| invalid("invalid branch header"))?;
            let value = text(value);
            match key {
                b"branch.oid" => result.branch.oid = Some(value),
                b"branch.head" => result.branch.head = Some(value),
                b"branch.upstream" => result.branch.upstream = Some(value),
                b"branch.ab" => {
                    let mut v = value.split_whitespace();
                    result.branch.ahead = v
                        .next()
                        .and_then(|s| s.strip_prefix('+'))
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| invalid("invalid ahead"))?;
                    result.branch.behind = v
                        .next()
                        .and_then(|s| s.strip_prefix('-'))
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| invalid("invalid behind"))?;
                }
                _ => {}
            }
            continue;
        }
        let count = match record[0] {
            b'1' => 9,
            b'2' => 10,
            b'u' => 11,
            b'?' | b'!' => 2,
            _ => return Err(invalid("unknown status record")),
        };
        let fields: Vec<_> = record.splitn(count, |b| *b == b' ').collect();
        if fields.len() != count {
            return Err(invalid("truncated status record"));
        }
        let mut entry = Entry {
            kind: record[0] as char,
            xy: String::new(),
            submodule: String::new(),
            path: RepoPath::new(fields[count - 1]),
            original: None,
            similarity: None,
            metadata: vec![],
        };
        if count > 2 {
            entry.xy = text(fields[1]);
            entry.submodule = text(fields[2]);
            entry.metadata = fields[3..count - 1].iter().map(|b| text(b)).collect();
            if entry.xy.len() != 2 {
                return Err(invalid("invalid XY"));
            }
        }
        match record[0] {
            b'?' => result.untracked.push(entry),
            b'!' => result.ignored.push(entry),
            b'u' => result.conflicts.push(entry),
            _ => {
                if record[0] == b'2' {
                    entry.original = Some(RepoPath::new(
                        records
                            .next()
                            .ok_or_else(|| invalid("missing rename source"))?,
                    ));
                    entry.similarity = Some(text(fields[8]));
                }
                let xy = entry.xy.as_bytes();
                if xy[0] != b'.' {
                    result.staged.push(entry.clone());
                }
                if xy[1] != b'.' {
                    result.unstaged.push(entry);
                }
            }
        }
    }
    Ok(result)
}
trait SplitByte {
    fn split_once_byte(&self, b: u8) -> Option<(&[u8], &[u8])>;
}
impl SplitByte for [u8] {
    fn split_once_byte(&self, b: u8) -> Option<(&[u8], &[u8])> {
        let i = self.iter().position(|v| *v == b)?;
        Some((&self[..i], &self[i + 1..]))
    }
}

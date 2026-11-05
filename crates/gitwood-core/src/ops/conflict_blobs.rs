//! Conflict-list metadata in two batch commands, deduplicated by immutable blob ID.
use super::merge_text;
use crate::{
    git::{GitError, GitRunner, ReadCommand, ReadOperation, RunOptions},
    repo::{invalid, status::Entry},
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub(super) struct Info {
    pub large: bool,
    pub binary: bool,
}

pub(super) fn stages(e: &Entry) -> impl Iterator<Item = &str> {
    (0..3).filter_map(|i| {
        e.metadata
            .get(i)
            .filter(|m| *m != "000000" && *m != "160000")
            .and_then(|_| e.metadata.get(i + 4))
            .map(String::as_str)
    })
}
pub(super) async fn read(
    r: &GitRunner,
    o: &RunOptions,
    entries: &[Entry],
    limit: usize,
) -> Result<BTreeMap<String, Info>, GitError> {
    let ids: BTreeSet<_> = entries.iter().flat_map(stages).collect();
    if ids.is_empty() {
        return Ok(BTreeMap::new());
    }
    let sizes = batch(
        r,
        o,
        "--batch-check",
        &ids.iter().copied().collect::<Vec<_>>(),
    )
    .await?;
    let sizes = std::str::from_utf8(&sizes).map_err(|_| invalid("conflict.invalidBatch"))?;
    let mut lines = sizes.lines();
    let mut info = BTreeMap::new();
    let mut small = Vec::new();
    for id in ids {
        let size = header(
            lines
                .next()
                .ok_or_else(|| invalid("conflict.invalidBatch"))?,
            id,
        )?;
        info.insert(
            id.to_owned(),
            Info {
                large: size > limit,
                binary: false,
            },
        );
        if size <= limit {
            small.push((id, size));
        }
    }
    if lines.next().is_some() {
        return Err(invalid("conflict.invalidBatch"));
    }
    if !small.is_empty() {
        let output = batch(
            r,
            o,
            "--batch",
            &small.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
        )
        .await?;
        classify(&output, &small, &mut info)?;
    }
    Ok(info)
}
async fn batch(
    r: &GitRunner,
    o: &RunOptions,
    flag: &str,
    ids: &[&str],
) -> Result<Vec<u8>, GitError> {
    let mut options = o.clone();
    options.stdin.clear();
    for id in ids {
        if ![40, 64].contains(&id.len()) || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(invalid("conflict.invalidStage"));
        }
        options.stdin.extend_from_slice(id.as_bytes());
        options.stdin.push(b'\n');
    }
    Ok(r.run(
        ReadCommand::new(ReadOperation::CatFile).args([flag]),
        &options,
    )
    .await?
    .stdout)
}
fn header(line: &str, id: &str) -> Result<usize, GitError> {
    let mut fields = line.split(' ');
    if fields.next() != Some(id) || fields.next() != Some("blob") {
        return Err(invalid("conflict.invalidBatch"));
    }
    let size = fields
        .next()
        .and_then(|n| n.parse().ok())
        .ok_or_else(|| invalid("conflict.invalidBatch"))?;
    if fields.next().is_some() {
        return Err(invalid("conflict.invalidBatch"));
    }
    Ok(size)
}
fn classify(
    mut output: &[u8],
    small: &[(&str, usize)],
    info: &mut BTreeMap<String, Info>,
) -> Result<(), GitError> {
    for (id, expected_size) in small {
        let end = output
            .iter()
            .position(|b| *b == b'\n')
            .ok_or_else(|| invalid("conflict.invalidBatch"))?;
        let line =
            std::str::from_utf8(&output[..end]).map_err(|_| invalid("conflict.invalidBatch"))?;
        let size = header(line, id)?;
        output = &output[end + 1..];
        if size != *expected_size || output.len() <= size || output[size] != b'\n' {
            return Err(invalid("conflict.invalidBatch"));
        }
        // Read the whole bounded blob to retain strict decoding (including UTF-16 BOM)
        // and detect invalid bytes beyond Git's first-8000-byte NUL heuristic.
        info.get_mut(*id).unwrap().binary = merge_text::decode(&output[..size]).is_none();
        output = &output[size + 1..];
    }
    if !output.is_empty() {
        return Err(invalid("conflict.invalidBatch"));
    }
    Ok(())
}

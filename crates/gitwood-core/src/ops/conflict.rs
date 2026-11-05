//! Shared conflict resolution for merge/rebase, history operations and stash recovery.
use super::{context::Context, index::IndexTransaction, merge_text, CommitError};
use crate::{
    git::{GitError, GitRunner, ReadCommand, ReadOperation, RunOptions, WriteCommand},
    repo::{self, operation::OperationKind, status::Entry, RepoPath},
};
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsString,
    io::Write,
    path::{Component, PathBuf},
};

const LIMIT: usize = 1024 * 1024;
enum Blob {
    Missing,
    Large,
    Bytes(Vec<u8>),
}
impl Blob {
    fn bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Missing => Some(&[]),
            Self::Large => None,
            Self::Bytes(bytes) => Some(bytes),
        }
    }
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictFile {
    pub path: RepoPath,
    pub conflict_type: String,
    pub binary: bool,
    pub large: bool,
    pub submodule: bool,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeContent {
    pub fingerprint: String,
    pub left: String,
    pub right: String,
    pub result: String,
    pub blocks: Vec<merge_text::MergeBlock>,
    pub read_only: Option<&'static str>,
}
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Yours,
    Theirs,
}

pub async fn files(
    r: &GitRunner,
    o: &RunOptions,
    entries: &[Entry],
) -> Result<Vec<ConflictFile>, GitError> {
    let blobs = super::conflict_blobs::read(r, o, entries, LIMIT).await?;
    Ok(entries
        .iter()
        .map(|e| ConflictFile {
            path: e.path.clone(),
            conflict_type: e.xy.clone(),
            binary: super::conflict_blobs::stages(e).any(|id| blobs[id].binary),
            large: super::conflict_blobs::stages(e).any(|id| blobs[id].large),
            submodule: e.metadata.iter().take(3).any(|m| m == "160000"),
        })
        .collect())
}

async fn stage_bytes(
    r: &GitRunner,
    o: &RunOptions,
    e: &Entry,
    stage: usize,
) -> Result<Blob, GitError> {
    if e.metadata
        .get(stage - 1)
        .is_none_or(|mode| mode == "000000")
    {
        return Ok(Blob::Missing);
    }
    let oid = e
        .metadata
        .get(stage + 3)
        .ok_or_else(|| repo::invalid("conflict.invalidStage"))?;
    let size = r
        .run(
            ReadCommand::new(ReadOperation::CatFile).args(["-s", oid]),
            o,
        )
        .await?;
    if String::from_utf8_lossy(&size.stdout)
        .trim()
        .parse::<usize>()
        .unwrap_or(usize::MAX)
        > LIMIT
    {
        return Ok(Blob::Large);
    }
    // Pin the blob ID from the stage metadata: an external index replacement between
    // size check and show must not turn this into an unbounded read of another blob.
    Ok(Blob::Bytes(
        r.run(
            ReadCommand::new(ReadOperation::Show).args([oid.as_str()]),
            o,
        )
        .await?
        .stdout,
    ))
}
fn path(o: &RunOptions, p: &RepoPath) -> Result<PathBuf, GitError> {
    let relative = PathBuf::from(repo::os(p.bytes.clone()));
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(repo::invalid("conflict.invalidPath"));
    }
    let full = o.cwd.join(relative);
    let mut ancestor = full
        .parent()
        .ok_or_else(|| repo::invalid("conflict.invalidPath"))?;
    loop {
        match ancestor.symlink_metadata() {
            Ok(_) => break,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                ancestor = ancestor
                    .parent()
                    .ok_or_else(|| repo::invalid("conflict.invalidPath"))?;
            }
            Err(e) => return Err(e.into()),
        }
    }
    if !ancestor.canonicalize()?.starts_with(o.cwd.canonicalize()?) {
        return Err(repo::invalid("conflict.invalidPath"));
    }
    Ok(full)
}
async fn entry(r: &GitRunner, o: &RunOptions, p: &RepoPath) -> Result<Entry, GitError> {
    repo::status::read(r, o)
        .await?
        .conflicts
        .into_iter()
        .find(|e| e.path.bytes == p.bytes)
        .ok_or_else(|| repo::invalid("conflict.noLongerUnmerged"))
}
async fn rebasing(r: &GitRunner, o: &RunOptions) -> Result<bool, GitError> {
    let repository = repo::Repository::open(r, o).await?;
    Ok(
        repo::operation::read(&repository.git_dir)?
            .is_some_and(|s| s.kind == OperationKind::Rebase),
    )
}
pub async fn read(r: &GitRunner, o: &RunOptions, p: &RepoPath) -> Result<MergeContent, GitError> {
    let before = super::snapshot(r, o).await?;
    let e = entry(r, o, p).await?;
    let full = path(o, p)?;
    let unsupported = e
        .metadata
        .iter()
        .take(3)
        .any(|m| m != "000000" && m != "100644" && m != "100755");
    let mut response = MergeContent {
        fingerprint: before.fingerprint.clone(),
        left: String::new(),
        right: String::new(),
        result: String::new(),
        blocks: vec![],
        read_only: None,
    };
    if unsupported {
        response.read_only = Some("unsupported");
        return Ok(response);
    }
    let mut texts = Vec::new();
    for stage in 1..=3 {
        let blob = stage_bytes(r, o, &e, stage).await?;
        let Some((text, _)) = blob.bytes().and_then(merge_text::decode) else {
            response.read_only = Some("binaryOrLarge");
            return Ok(response);
        };
        if text.lines().count() > 20000 {
            response.read_only = Some("binaryOrLarge");
            return Ok(response);
        }
        texts.push(text);
    }
    if full.symlink_metadata().is_ok_and(|m| !m.is_file()) {
        response.read_only = Some("unsupported");
        return Ok(response);
    }
    if full.metadata().is_ok_and(|m| m.len() > LIMIT as u64) {
        response.read_only = Some("binaryOrLarge");
        return Ok(response);
    }
    if full.exists() && merge_text::decode(&std::fs::read(&full)?).is_none() {
        response.read_only = Some("encoding");
        return Ok(response);
    }
    let reversed = rebasing(r, o).await?;
    response.left = texts[if reversed { 2 } else { 1 }].clone();
    response.right = texts[if reversed { 1 } else { 2 }].clone();
    (response.result, response.blocks) =
        merge_text::merge(&texts[0], &response.left, &response.right);
    if super::snapshot(r, o).await? != before {
        return Err(repo::invalid("remote.concurrentChange"));
    }
    Ok(response)
}

/// All writes use a locked alternate copy of the real index, preserving the unmerged
/// entries if checkout, writing or staging fails. The text remains in the caller on errors.
pub async fn resolve(
    r: &GitRunner,
    o: &RunOptions,
    p: &RepoPath,
    expected: &str,
    side: Option<Side>,
    text: Option<&str>,
) -> Result<(), CommitError> {
    if o.env.contains_key(std::ffi::OsStr::new("GIT_INDEX_FILE"))
        || std::env::var_os("GIT_INDEX_FILE").is_some()
    {
        return Err(CommitError::InvalidSelection(
            "caller must not override the real index".into(),
        ));
    }
    let _gate = tokio::select! {
        _ = o.cancellation.cancelled() => return Err(GitError::Cancelled.into()),
        guard = super::WRITES.lock() => guard,
    };
    super::remote::check(r, o, expected).await?;
    let context = Context {
        runner: r,
        options: o.clone(),
    };
    context.require_root().await?;
    let mut tx = IndexTransaction::acquire(context.git_path("index").await?)?;
    let index = tx.path("resolved");
    tx.copy_original(&index)?;
    let mut options = o.clone();
    options
        .env
        .insert("GIT_INDEX_FILE".into(), index.as_os_str().into());
    options
        .env
        .insert("GIT_LITERAL_PATHSPECS".into(), "1".into());
    let head = context.head().await?;
    let e = entry(r, o, p).await?;
    let full = path(o, p)?;
    if super::snapshot(r, o).await?.fingerprint != expected {
        return Err(CommitError::ConcurrentChange);
    }
    let needs_add = if let Some(side) = side {
        let ours = matches!(side, Side::Yours) != rebasing(r, o).await?;
        accept(r, &options, &e, ours).await?
    } else if let Some(text) = text {
        save_text(&context, &e, &full, expected, text).await?;
        true
    } else {
        return Err(CommitError::InvalidSelection(
            "conflict.missingResolution".into(),
        ));
    };
    let written = super::snapshot(r, o).await?;
    if needs_add {
        run(
            r,
            &options,
            "add",
            vec!["--".into(), repo::os(p.bytes.clone())],
        )
        .await?;
    }
    if context.head().await? != head || super::snapshot(r, o).await? != written {
        return Err(CommitError::ConcurrentChange);
    }
    tx.install(&index)
}

/// Returns whether the selected regular file still needs staging.
async fn accept(r: &GitRunner, o: &RunOptions, e: &Entry, ours: bool) -> Result<bool, GitError> {
    let stage = if ours { 2 } else { 3 };
    let name = repo::os(e.path.bytes.clone());
    match e.metadata[stage - 1].as_str() {
        "000000" => {
            run(r, o, "rm", vec!["--".into(), name]).await?;
            Ok(false)
        }
        "160000" => {
            // Select the gitlink itself; never checkout or stage the nested repository.
            run(
                r,
                o,
                "update-index",
                vec![
                    "--add".into(),
                    "--cacheinfo".into(),
                    "160000".into(),
                    e.metadata[stage + 3].clone().into(),
                    name,
                ],
            )
            .await?;
            Ok(false)
        }
        _ => {
            run(
                r,
                o,
                "checkout",
                vec![
                    if ours { "--ours" } else { "--theirs" }.into(),
                    "--".into(),
                    name,
                ],
            )
            .await?;
            Ok(true)
        }
    }
}
async fn save_text(
    context: &Context<'_>,
    e: &Entry,
    full: &std::path::Path,
    expected: &str,
    text: &str,
) -> Result<(), CommitError> {
    let parent = full
        .parent()
        .ok_or_else(|| repo::invalid("conflict.invalidPath"))?;
    if !parent.metadata()?.is_dir() {
        return Err(repo::invalid("conflict.invalidPath").into());
    }
    let (r, o) = (context.runner, &context.options);
    let content = read(r, o, &e.path).await?;
    if content.fingerprint != expected || content.read_only.is_some() || text.len() > LIMIT * 2 {
        return Err(CommitError::InvalidSelection(
            "conflict.readOnlyOrChanged".into(),
        ));
    }
    let original = if full.exists() {
        std::fs::read(full)?
    } else {
        Vec::new()
    };
    let stage = if rebasing(r, o).await? { 3 } else { 2 };
    let stage_encoding = stage_bytes(r, o, e, stage)
        .await?
        .bytes()
        .and_then(merge_text::decode)
        .map(|v| v.1);
    let bytes = result_bytes(&original, stage_encoding, text);
    let mut temp = tempfile::NamedTempFile::new_in(context.git_dir().await?)?;
    temp.write_all(&bytes)?;
    if let Ok(meta) = full.metadata() {
        temp.as_file().set_permissions(meta.permissions())?;
    }
    temp.as_file().sync_all()?;
    if super::snapshot(r, o).await?.fingerprint != expected {
        return Err(CommitError::ConcurrentChange);
    }
    temp.persist(full).map_err(|e| e.error)?;
    Ok(())
}
fn result_bytes(original: &[u8], stage: Option<merge_text::Encoding>, text: &str) -> Vec<u8> {
    let decoded = merge_text::decode(original);
    let work_encoding = decoded.as_ref().map(|v| v.1);
    let encoding = work_encoding
        .filter(|e| *e != merge_text::Encoding::Utf8)
        .or(stage)
        .or(work_encoding)
        .unwrap_or(merge_text::Encoding::Utf8);
    let original_text = decoded.map(|v| v.0).unwrap_or_default();
    let text =
        if original_text.contains("\r\n") && !original_text.replace("\r\n", "").contains('\n') {
            text.replace("\r\n", "\n").replace('\n', "\r\n")
        } else {
            text.to_owned()
        };
    merge_text::encode(&text, encoding)
}

async fn run(
    r: &GitRunner,
    o: &RunOptions,
    command: &str,
    args: Vec<OsString>,
) -> Result<(), GitError> {
    r.run(WriteCommand::new(command).args(args), o).await?;
    Ok(())
}

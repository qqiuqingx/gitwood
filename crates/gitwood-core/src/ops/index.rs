use super::{context::Context, CommitError, IndexRecovery};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tempfile::TempDir;

/// Reserve the real Git index lock throughout the operation, including commit hooks.
/// Git itself locks only the alternate index, so it never needs our real-index lock.
pub(super) struct IndexTransaction {
    pub index: PathBuf,
    pub directory: TempDir,
    original: Option<Vec<u8>>,
    lock: PathBuf,
    file: Option<File>,
}

impl IndexTransaction {
    pub fn acquire(index: PathBuf) -> Result<Self, CommitError> {
        let mut lock = index.as_os_str().to_os_string();
        lock.push(".lock");
        let lock = PathBuf::from(lock);
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock)
            .map_err(|e| {
                if e.kind() == io::ErrorKind::AlreadyExists {
                    CommitError::IndexLocked(lock.clone())
                } else {
                    CommitError::Io(e)
                }
            })?;
        // Construct the guard before fallible reads so the lock is always removed.
        let mut guard = Self {
            directory: tempfile::Builder::new()
                .prefix("gitwood-index-")
                .tempdir()
                .inspect_err(|_| {
                    let _ = fs::remove_file(&lock);
                })?,
            index,
            original: None,
            lock,
            file: Some(file),
        };
        guard.original = read_optional(&guard.index)?;
        guard.copy_original(&guard.path("original"))?;
        Ok(guard)
    }
    pub fn path(&self, name: &str) -> PathBuf {
        self.directory.path().join(name)
    }
    pub fn check_unchanged(&self) -> Result<(), CommitError> {
        if read_optional(&self.index)? != self.original {
            return Err(CommitError::ConcurrentChange);
        }
        Ok(())
    }
    pub fn copy_original(&self, target: &Path) -> Result<(), CommitError> {
        if let Some(bytes) = &self.original {
            fs::write(target, bytes)?;
        }
        Ok(())
    }
    pub fn install(&mut self, aligned: &Path) -> Result<(), CommitError> {
        self.check_unchanged()?;
        let bytes = fs::read(aligned)?;
        let mut file = self.file.take().expect("index lock held");
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&self.lock, &self.index)?;
        Ok(())
    }

    pub fn preserve(&mut self, hash: &str, base: &str, mut reason: String) -> IndexRecovery {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let saved = (|| -> io::Result<PathBuf> {
            let directory = tempfile::Builder::new()
                .prefix(&format!("gitwood-recovery-{timestamp}-{hash}-"))
                .tempdir_in(
                    self.index
                        .parent()
                        .ok_or_else(|| io::Error::other("index has no parent"))?,
                )?;
            for entry in fs::read_dir(self.directory.path())? {
                let entry = entry?;
                fs::copy(entry.path(), directory.path().join(entry.file_name()))?;
            }
            Ok(directory.keep())
        })();
        let directory = match saved {
            Ok(directory) => directory,
            Err(error) => {
                // A full/unwritable Git directory must not destroy the sole snapshot.
                self.directory.disable_cleanup(true);
                reason.push_str(&format!(
                    "; could not save recovery inside Git directory: {error}"
                ));
                self.directory.path().to_owned()
            }
        };
        let instructions = format!(
            "Commit {hash} completed. Do not retry the commit.\n\nRecovery directory: {directory:?}\nReal index: {index:?}\nBase tree: {base}\nReason: {reason}\n\nClose other Git writers first. Inspect HEAD and back up the current real index.\nThe original snapshot is `original` (absent for an initially missing index);\n`selected` is the hook's commit index; `aligned` may contain unresolved stages.\nDo not install `aligned` blindly.\n\nFrom the worktree root, POSIX shell (set RECOVERY to the directory above):\n\n```sh\nRECOVERY='/absolute/recovery/directory'\nGIT_INDEX_FILE=\"$RECOVERY/original\" git --no-optional-locks -c core.fsmonitor=false diff --cached --binary --full-index --no-ext-diff --no-textconv --src-prefix=a/ --dst-prefix=b/ --no-color --no-relative --output=\"$RECOVERY/staged.patch\" {base} --\ngit reset --mixed {hash}\ngit apply --cached --3way \"$RECOVERY/staged.patch\"\ngit status --short\n```\n\nPowerShell: set `$env:GIT_INDEX_FILE` to the original snapshot for the diff,\nthen remove that environment variable before reset/apply; use `--output` rather\nthan shell redirection to preserve binary patch bytes. Use the same Git commands.\nIf `original` is absent or the exported patch is empty, skip diff/apply as needed.\nReset changes only the index. Apply can report conflicts: review them manually,\nchoose the intended staging, and retain this directory until verified.\nThis restores original staging on top of the completed commit; it cannot decide\nwhich side of a hook conflict you intended. Never overwrite newer external changes.\n",
            index = self.index,
        );
        if let Err(error) = fs::write(directory.join("RECOVERY.md"), &instructions) {
            reason.push_str(&format!("; could not write RECOVERY.md: {error}"));
        }
        IndexRecovery {
            directory,
            index: self.index.clone(),
            instructions,
            reason,
        }
    }
}
impl Drop for IndexTransaction {
    fn drop(&mut self) {
        self.file.take();
        let _ = fs::remove_file(&self.lock);
    }
}
fn read_optional(path: &Path) -> Result<Option<Vec<u8>>, io::Error> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

pub(super) async fn align(
    context: &Context<'_>,
    tx: &IndexTransaction,
    base: &str,
    tree: &str,
    files: &[PathBuf],
    renames: &[(PathBuf, PathBuf)],
) -> Result<PathBuf, CommitError> {
    let aligned = tx.path("aligned");
    if aligned.exists() {
        fs::remove_file(&aligned)?;
    }
    tx.copy_original(&aligned)?;
    if !aligned.exists() {
        context
            .write("read-tree", &["--empty"], &aligned, &[])
            .await?;
    }
    // Whole-file selection deliberately replaces earlier staging for that path.
    context.reset_paths(&aligned, base, files).await?;
    let patch_base = if renames.is_empty() {
        base.to_owned()
    } else {
        let baseline = tx.path("renamed-base");
        context.write("read-tree", &[base], &baseline, &[]).await?;
        // Use the same pathname for all three sides of content merging. The
        // original staging may already contain the rename plus unselected edits.
        for (old, new) in renames {
            rename_entry(context, &baseline, old, new, false).await?;
            rename_entry(context, &aligned, old, new, true).await?;
        }
        super::context::as_text(&context.write("write-tree", &[], &baseline, &[]).await?)?
    };
    let patch = context.tree_patch(&patch_base, tree).await?;
    if !patch.is_empty() {
        context
            .write(
                "apply",
                &["--cached", "--3way", "--whitespace=nowarn"],
                &aligned,
                &patch,
            )
            .await
            .map_err(|e| match e {
                CommitError::Git(crate::git::GitError::Exit { .. }) => {
                    CommitError::StagedConflict(e.to_string())
                }
                other => other,
            })?;
    }
    // write-tree also rejects unresolved index stages (including pre-existing conflicts).
    context.write("write-tree", &[], &aligned, &[]).await?;
    Ok(aligned)
}

async fn index_entry(
    context: &Context<'_>,
    index: &Path,
    path: &Path,
) -> Result<Option<(String, String)>, CommitError> {
    use crate::git::{ReadCommand, ReadOperation};
    let mut options = context.options.clone();
    options
        .env
        .insert("GIT_INDEX_FILE".into(), index.as_os_str().into());
    options
        .env
        .insert("GIT_LITERAL_PATHSPECS".into(), "1".into());
    let bytes = context
        .runner
        .run(
            ReadCommand::new(ReadOperation::LsFiles).args([
                std::ffi::OsStr::new("--stage"),
                std::ffi::OsStr::new("-z"),
                std::ffi::OsStr::new("--"),
                path.as_os_str(),
            ]),
            &options,
        )
        .await?
        .stdout;
    if bytes.is_empty() {
        return Ok(None);
    }
    let records: Vec<_> = bytes.split(|b| *b == 0).filter(|r| !r.is_empty()).collect();
    if records.len() != 1 {
        return Err(CommitError::StagedConflict("unmerged rename path".into()));
    }
    let header = records[0].split(|b| *b == b'\t').next().unwrap();
    let fields: Vec<_> = header.split(|b| *b == b' ').collect();
    if fields.len() != 3 || fields[2] != b"0" {
        return Err(CommitError::StagedConflict("unmerged rename path".into()));
    }
    Ok(Some((
        super::context::as_text(fields[0])?,
        super::context::as_text(fields[1])?,
    )))
}
async fn rename_entry(
    context: &Context<'_>,
    index: &Path,
    old: &Path,
    new: &Path,
    already_renamed: bool,
) -> Result<(), CommitError> {
    let source = index_entry(context, index, old).await?;
    let destination = index_entry(context, index, new).await?;
    match (source, destination) {
        (None, Some(_)) if already_renamed => Ok(()),
        (Some((mode, oid)), None) => {
            let mut bytes = format!("0 {}\t", "0".repeat(oid.len())).into_bytes();
            bytes.extend_from_slice(&path_bytes(old)?);
            bytes.push(0);
            bytes.extend_from_slice(format!("{mode} {oid}\t").as_bytes());
            bytes.extend_from_slice(&path_bytes(new)?);
            bytes.push(0);
            context
                .write("update-index", &["-z", "--index-info"], index, &bytes)
                .await?;
            Ok(())
        }
        _ => Err(CommitError::StagedConflict(
            "rename source/destination changed or both exist in index".into(),
        )),
    }
}
#[cfg(unix)]
fn path_bytes(path: &Path) -> Result<Vec<u8>, CommitError> {
    use std::os::unix::ffi::OsStrExt;
    Ok(path.as_os_str().as_bytes().to_vec())
}
#[cfg(not(unix))]
fn path_bytes(path: &Path) -> Result<Vec<u8>, CommitError> {
    path.to_str()
        .map(|s| s.as_bytes().to_vec())
        .ok_or_else(|| CommitError::InvalidSelection("invalid UTF-8 rename path".into()))
}

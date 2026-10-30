use super::CommitError;
use crate::git::{GitError, GitRunner, ReadCommand, ReadOperation, RunOptions, WriteCommand};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

pub(super) struct Context<'a> {
    pub runner: &'a GitRunner,
    pub options: RunOptions,
}

impl Context<'_> {
    pub async fn require_root(&self) -> Result<(), CommitError> {
        let location = self
            .read(
                ReadOperation::RevParse,
                &["--is-inside-work-tree", "--show-prefix"],
            )
            .await?;
        if location != b"true\n\n" {
            return Err(CommitError::InvalidSelection(
                "cwd must be the working-tree root".into(),
            ));
        }
        Ok(())
    }

    pub async fn read(
        &self,
        operation: ReadOperation,
        args: &[&str],
    ) -> Result<Vec<u8>, CommitError> {
        Ok(self
            .runner
            .run(
                ReadCommand::new(operation).args(args.iter().copied()),
                &self.options,
            )
            .await?
            .stdout)
    }
    pub async fn write(
        &self,
        name: &str,
        args: &[&str],
        index: &Path,
        input: &[u8],
    ) -> Result<Vec<u8>, CommitError> {
        let mut options = self.options.clone();
        options
            .env
            .insert("GIT_INDEX_FILE".into(), index.as_os_str().into());
        options.stdin = input.into();
        Ok(self
            .runner
            .run(WriteCommand::new(name).args(args.iter().copied()), &options)
            .await?
            .stdout)
    }
    pub async fn tree_patch(&self, base: &str, tree: &str) -> Result<Vec<u8>, CommitError> {
        self.read(
            ReadOperation::DiffTree,
            &[
                "-p",
                "--binary",
                "--full-index",
                "--no-renames",
                "--no-color",
                "--src-prefix=a/",
                "--dst-prefix=b/",
                "--no-relative",
                "--unified=3",
                "--submodule=short",
                base,
                tree,
            ],
        )
        .await
    }
    pub async fn head(&self) -> Result<Option<String>, CommitError> {
        match self
            .read(ReadOperation::RevParse, &["--verify", "-q", "HEAD"])
            .await
        {
            Ok(out) => Ok(Some(as_text(&out)?)),
            Err(CommitError::Git(GitError::Exit { code: Some(1), .. })) => Ok(None),
            Err(err) => Err(err),
        }
    }
    pub async fn git_path(&self, name: &str) -> Result<PathBuf, CommitError> {
        let bytes = self
            .read(ReadOperation::RevParse, &["--git-path", name])
            .await?;
        let mut bytes = bytes;
        if bytes.last() == Some(&b'\n') {
            bytes.pop();
        }
        let path = PathBuf::from(path_string(bytes)?);
        Ok(if path.is_absolute() {
            path
        } else {
            self.options.cwd.join(path)
        })
    }
    pub async fn reset_paths(
        &self,
        index: &Path,
        tree: &str,
        paths: &[PathBuf],
    ) -> Result<(), CommitError> {
        for path in paths {
            let args: Vec<OsString> = vec![
                "--quiet".into(),
                tree.into(),
                "--".into(),
                path.as_os_str().into(),
            ];
            let mut options = self.options.clone();
            options
                .env
                .insert("GIT_INDEX_FILE".into(), index.as_os_str().into());
            self.runner
                .run(
                    WriteCommand::new("reset").args(args),
                    &literal_options(options),
                )
                .await?;
        }
        Ok(())
    }
}

fn literal_options(mut options: RunOptions) -> RunOptions {
    options
        .env
        .insert("GIT_LITERAL_PATHSPECS".into(), "1".into());
    options
}

pub(super) fn as_text(bytes: &[u8]) -> Result<String, CommitError> {
    String::from_utf8(bytes.to_vec())
        .map(|s| s.trim().into())
        .map_err(|_| CommitError::InvalidSelection("expected ASCII Git object ID".into()))
}
#[cfg(unix)]
fn path_string(bytes: Vec<u8>) -> Result<OsString, CommitError> {
    use std::os::unix::ffi::OsStringExt;
    Ok(OsString::from_vec(bytes))
}
#[cfg(not(unix))]
fn path_string(bytes: Vec<u8>) -> Result<OsString, CommitError> {
    String::from_utf8(bytes)
        .map(Into::into)
        .map_err(|_| CommitError::InvalidSelection("non UTF-8 repository path on Windows".into()))
}

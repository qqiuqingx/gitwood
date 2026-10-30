#![allow(dead_code)]
use gitwood_core::{
    git::{GitRunner, ReadCommand, ReadOperation, RunOptions, WriteCommand},
    ops::{partial_commit, CommitError, CommitOutcome, CommitSelection},
};
use std::{fs, path::PathBuf};
use tempfile::TempDir;

pub struct Repo {
    pub dir: TempDir,
    pub runner: GitRunner,
    pub options: RunOptions,
}
impl Repo {
    pub async fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let runner = GitRunner::discover(None).await.unwrap();
        let mut options = RunOptions::new(dir.path());
        options.env.insert("GIT_CONFIG_NOSYSTEM".into(), "1".into());
        options.env.insert(
            "GIT_CONFIG_GLOBAL".into(),
            if cfg!(windows) { "NUL" } else { "/dev/null" }.into(),
        );
        let repo = Self {
            dir,
            runner,
            options,
        };
        repo.git("init", &["-q", "--template="]).await;
        for (key, value) in [
            ("user.name", "Gitwood Test"),
            ("user.email", "test@example.invalid"),
            ("commit.gpgsign", "false"),
            ("core.autocrlf", "false"),
        ] {
            repo.git("config", &[key, value]).await;
        }
        repo
    }
    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }
    pub fn put(&self, name: &str, content: &str) {
        fs::write(self.path(name), content).unwrap();
    }
    pub async fn git(&self, name: &str, args: &[&str]) -> Vec<u8> {
        self.runner
            .run(
                WriteCommand::new(name).args(args.iter().copied()),
                &self.options,
            )
            .await
            .unwrap()
            .stdout
    }
    pub async fn read(&self, op: ReadOperation, args: &[&str]) -> Vec<u8> {
        self.runner
            .run(
                ReadCommand::new(op).args(args.iter().copied()),
                &self.options,
            )
            .await
            .unwrap()
            .stdout
    }
    pub async fn initial(&self) {
        self.put("A", "A base\n");
        self.put("B", "B base\n");
        self.git("add", &["--", "A", "B"]).await;
        self.git("commit", &["-qm", "base"]).await;
    }
    pub async fn commit(
        &self,
        files: &[&str],
        patches: Vec<Vec<u8>>,
        amend: bool,
    ) -> Result<CommitOutcome, CommitError> {
        partial_commit(
            &self.runner,
            &self.options,
            &CommitSelection {
                files: files.iter().map(PathBuf::from).collect(),
                patches,
                expected: None,
                renames: vec![],
            },
            b"selected commit\n",
            amend,
        )
        .await
    }
    pub async fn state(&self) -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) {
        (
            self.read(ReadOperation::Diff, &["--cached", "--binary"])
                .await,
            self.read(ReadOperation::Diff, &["--binary"]).await,
            fs::read(self.path(".git/index")).unwrap(),
            self.read(ReadOperation::RevParse, &["HEAD"]).await,
        )
    }
    pub fn clean_temporary_files(&self) {
        assert!(!self.path(".git/index.lock").exists());
        for entry in fs::read_dir(self.path(".git")).unwrap() {
            assert!(!entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("gitwood-index-"));
        }
    }
}

#[cfg(unix)]
pub fn script(path: &std::path::Path, body: &str) {
    use std::os::unix::fs::PermissionsExt;
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

/// A real helper's one-byte readiness event, with cleanup even if the operation
/// exits before signalling. This fixture does not impose a deadline on ops queues.
#[cfg(unix)]
pub struct ProcessSignal {
    receiver: tokio::sync::oneshot::Receiver<std::io::Result<()>>,
    unblock: std::fs::File,
    observer: Option<std::thread::JoinHandle<()>>,
}
#[cfg(unix)]
impl ProcessSignal {
    pub fn new(path: PathBuf) -> Self {
        use std::io::Read;
        assert!(std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .unwrap()
            .success());
        // O_RDWR keeps open independent of which side the OS schedules first.
        // Drop writes an explicit abort byte so early failure cannot leave an
        // observer blocked in open/read, even when no helper was ever spawned.
        let unblock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .unwrap();
        let mut reader = unblock.try_clone().unwrap();
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let observer = std::thread::spawn(move || {
            let mut byte = [0];
            let result = reader.read_exact(&mut byte).and_then(|()| {
                if byte == *b"R" {
                    Ok(())
                } else {
                    Err(std::io::Error::other("readiness observer aborted"))
                }
            });
            let _ = sender.send(result);
        });
        Self {
            receiver,
            unblock,
            observer: Some(observer),
        }
    }
    pub async fn wait(&mut self) -> std::io::Result<()> {
        (&mut self.receiver).await.map_err(std::io::Error::other)?
    }
}
#[cfg(unix)]
impl Drop for ProcessSignal {
    fn drop(&mut self) {
        use std::io::Write;
        let _ = self.unblock.write_all(b"X");
        if let Some(observer) = self.observer.take() {
            let _ = observer.join();
        }
    }
}

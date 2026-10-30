mod support;
use gitwood_core::git::{
    GitError, GitRunner, GitVersion, ReadCommand, ReadOperation, WriteCommand, MIN_GIT_VERSION,
};
use support::Repo;

#[test]
fn parses_vendor_versions() {
    for version in [
        "git version 2.39.5 (Apple Git-154)",
        "git version 2.47.1.windows.2",
        "git version 2.53.0",
    ] {
        assert!(GitVersion::parse(version.as_bytes()).unwrap() >= MIN_GIT_VERSION);
    }
    assert_eq!(
        GitVersion::parse(b"git version 2.35.2.windows.1").unwrap(),
        MIN_GIT_VERSION
    );
    assert!(GitVersion::parse(b"not git").is_err());
}

#[tokio::test]
async fn explicit_missing_executable_is_reported() {
    let repo = Repo::new().await;
    assert!(matches!(
        GitRunner::discover(Some(&repo.path("missing-git"))).await,
        Err(GitError::NotFound(_))
    ));
}

#[tokio::test]
async fn stdin_and_stdout_are_binary_and_pipes_do_not_deadlock() {
    let mut repo = Repo::new().await;
    let bytes: Vec<u8> = (0..2_000_000).map(|i| (i % 256) as u8).collect();
    repo.options.stdin = bytes.clone();
    let hash = repo
        .runner
        .run(
            WriteCommand::new("hash-object").args(["-w", "--stdin"]),
            &repo.options,
        )
        .await
        .unwrap();
    let hash = String::from_utf8(hash.stdout).unwrap();
    repo.options.stdin.clear();
    let out = repo
        .runner
        .run(
            ReadCommand::new(ReadOperation::Show).args([hash.trim()]),
            &repo.options,
        )
        .await
        .unwrap();
    assert_eq!(out.code, Some(0));
    assert_eq!(out.stdout, bytes);
    assert!(out.stderr.is_empty());
}

#[tokio::test]
async fn nonzero_exit_keeps_status_and_stderr() {
    let repo = Repo::new().await;
    let error = repo
        .runner
        .run(
            ReadCommand::new(ReadOperation::Show).args(["nonexistent"]),
            &repo.options,
        )
        .await
        .unwrap_err();
    assert!(
        matches!(error, GitError::Exit { code: Some(128), stderr } if stderr.contains("nonexistent"))
    );
}

#[cfg(unix)]
#[tokio::test]
async fn discovery_rejects_old_and_unexecutable_binaries() {
    let repo = Repo::new().await;
    let path = repo.path("custom-git");
    support::script(&path, "echo 'git version 2.14.0'");
    assert!(matches!(
        GitRunner::discover(Some(&path)).await,
        Err(GitError::VersionTooOld { .. })
    ));
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(matches!(
        GitRunner::discover(Some(&path)).await,
        Err(GitError::Spawn { .. })
    ));
}

#[cfg(unix)]
async fn slow_runner(repo: &Repo) -> GitRunner {
    let path = repo.path("slow-git");
    support::script(&path, "if [ \"$1\" = --version ]; then echo 'git version 2.53.0'; exit 0; fi\necho $$ > child-pid\nexec sleep 30");
    GitRunner::discover(Some(&path)).await.unwrap()
}

#[cfg(unix)]
async fn wait_pid(repo: &Repo) -> String {
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            if let Ok(pid) = std::fs::read_to_string(repo.path("child-pid")) {
                if pid.ends_with('\n') && pid.trim().parse::<u32>().is_ok() {
                    return pid;
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap()
}

#[cfg(unix)]
fn assert_dead(pid: &str) {
    let status = std::process::Command::new("kill")
        .args(["-0", pid.trim()])
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(!status.success(), "child {pid} still alive");
}

#[cfg(unix)]
#[tokio::test]
async fn timeout_kills_and_reaps_child() {
    let mut repo = Repo::new().await;
    let runner = slow_runner(&repo).await;
    repo.options.timeout = Some(std::time::Duration::from_millis(100));
    assert!(matches!(
        runner
            .run(ReadCommand::new(ReadOperation::Status), &repo.options)
            .await,
        Err(GitError::Timeout(_))
    ));
    assert_dead(&wait_pid(&repo).await);
}

#[cfg(unix)]
#[tokio::test]
async fn cancellation_token_kills_and_reaps_child() {
    let repo = Repo::new().await;
    let runner = slow_runner(&repo).await;
    let running = runner.run(ReadCommand::new(ReadOperation::Status), &repo.options);
    let cancel = async {
        let pid = wait_pid(&repo).await;
        repo.options.cancellation.cancel();
        pid
    };
    let (result, pid) = tokio::join!(running, cancel);
    assert!(matches!(result, Err(GitError::Cancelled)));
    assert_dead(&pid);
}

#[cfg(unix)]
#[tokio::test]
async fn cancellation_kills_hook_process_group_and_term_resistant_descendants() {
    use std::io::{BufRead, Read};
    for ignore_term in [false, true] {
        let repo = Repo::new().await;
        assert!(std::process::Command::new("mkfifo")
            .arg(repo.path("descendant-life"))
            .status()
            .unwrap()
            .success());
        let pipe = repo.path("descendant-life");
        let (ready_tx, ready) = tokio::sync::oneshot::channel();
        let (closed_tx, closed) = tokio::sync::oneshot::channel();
        let observer = std::thread::spawn(move || {
            let file = std::fs::File::open(pipe).unwrap();
            let mut reader = std::io::BufReader::new(file);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            assert_eq!(line, "ready\n");
            let _ = ready_tx.send(());
            let mut remainder = vec![];
            reader.read_to_end(&mut remainder).unwrap();
            let _ = closed_tx.send(());
        });
        // The child itself signals readiness after opening its lifetime pipe. The
        // hook installs its signal disposition before forking; exec preserves it.
        let hook = format!(
            "{}\n(printf 'ready\\n'; exec sleep 30) > descendant-life &\nprintf '%s\\n' $! > child-pid.tmp\nmv child-pid.tmp child-pid\nwait",
            if ignore_term { "trap '' TERM" } else { ":" }
        );
        support::script(&repo.path(".git/hooks/pre-commit"), &hook);
        let run = repo.runner.run(
            WriteCommand::new("commit").args(["--allow-empty", "-m", "cancel hook"]),
            &repo.options,
        );
        let cancel = async {
            tokio::time::timeout(std::time::Duration::from_secs(3), ready)
                .await
                .expect("descendant did not become ready")
                .unwrap();
            let pid = wait_pid(&repo).await;
            repo.options.cancellation.cancel();
            pid
        };
        let (result, _pid) = tokio::join!(run, cancel);
        assert!(matches!(result, Err(GitError::Cancelled)));
        // kill -0 also succeeds for zombies whose parent/init has not reaped them.
        // EOF proves the descendant released its kernel-held descriptors, without
        // depending on the host's orphan reaping latency under parallel load.
        tokio::time::timeout(std::time::Duration::from_secs(3), closed)
            .await
            .expect("hook descendant retained its lifetime pipe")
            .unwrap();
        observer.join().unwrap();
    }
}

#[cfg(unix)]
#[tokio::test]
async fn writes_wait_without_a_default_deadline_and_honor_explicit_timeout() {
    let mut repo = Repo::new().await;
    let helper = repo.path("waiting-git");
    support::script(&helper, "if [ \"$1\" = --version ]; then echo 'git version 2.53.0'; exit 0; fi\nsleep 0.2\necho completed");
    let runner = GitRunner::discover(Some(&helper)).await.unwrap();
    assert_eq!(repo.options.timeout, None);
    assert_eq!(
        runner
            .run(WriteCommand::new("commit"), &repo.options)
            .await
            .unwrap()
            .stdout,
        b"completed\n"
    );
    repo.options.timeout = Some(std::time::Duration::from_millis(20));
    assert!(matches!(
        runner.run(WriteCommand::new("commit"), &repo.options).await,
        Err(GitError::Timeout(_))
    ));
}

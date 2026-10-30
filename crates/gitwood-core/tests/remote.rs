mod support;
use gitwood_core::ops::{
    remote::{self, PushOptions, RemoteOutcome, UpdateStrategy},
    snapshot,
};
use support::Repo;
async fn token(r: &Repo) -> String {
    snapshot(&r.runner, &r.options).await.unwrap().fingerprint
}
fn options() -> PushOptions {
    PushOptions {
        remote: Some("origin".into()),
        branch: Some("main".into()),
        force_with_lease: false,
        tags: false,
    }
}
async fn fixture() -> (Repo, Repo, tempfile::TempDir) {
    let bare = tempfile::tempdir().unwrap();
    let a = Repo::new().await;
    let b = Repo::new().await;
    a.git("init", &["--bare", bare.path().to_str().unwrap()])
        .await;
    for r in [&a, &b] {
        r.git("remote", &["add", "origin", bare.path().to_str().unwrap()])
            .await;
    }
    a.initial().await;
    a.git("branch", &["-M", "main"]).await;
    (a, b, bare)
}
async fn diverged(configured: Option<&str>) -> (Repo, Repo, tempfile::TempDir) {
    let (remote, target, bare) = fixture().await;
    remote::push(
        &remote.runner,
        &remote.options,
        &options(),
        &token(&remote).await,
    )
    .await
    .unwrap();
    target.git("fetch", &[]).await;
    target
        .git("checkout", &["-b", "main", "--track", "origin/main"])
        .await;
    target.put("A", "target\n");
    target.git("commit", &["-am", "target"]).await;
    remote.put("B", "remote\n");
    remote.git("commit", &["-am", "remote"]).await;
    remote.git("push", &[]).await;
    if let Some(value) = configured {
        target.git("config", &["pull.rebase", value]).await;
    }
    (remote, target, bare)
}
#[tokio::test]
async fn push_fetch_preview_rejection_and_lease() {
    let (a, b, _bare) = fixture().await;
    let p = options();
    assert_eq!(
        remote::preview(&a.runner, &a.options, &p)
            .await
            .unwrap()
            .commits
            .len(),
        1
    );
    assert!(matches!(
        remote::push(&a.runner, &a.options, &p, &token(&a).await)
            .await
            .unwrap(),
        RemoteOutcome::Push { success: true, .. }
    ));
    b.git("fetch", &["origin"]).await;
    b.git("checkout", &["-b", "main", "--track", "origin/main"])
        .await;
    b.put("B", "b\n");
    b.git("commit", &["-am", "b"]).await;
    b.git("push", &[]).await;
    a.put("A", "a\n");
    a.git("commit", &["-am", "a"]).await;
    assert!(
        matches!(remote::push(&a.runner,&a.options,&p,&token(&a).await).await.unwrap(),RemoteOutcome::Push{success:false,refs} if refs.iter().any(|r|r.rejection == Some(remote::PushRejection::NeedsUpdate)))
    );
    let mut stale = options();
    stale.force_with_lease = true;
    assert!(
        matches!(remote::push(&a.runner, &a.options, &stale, &token(&a).await).await.unwrap(), RemoteOutcome::Push {success:false, refs} if refs.iter().any(|r| r.rejection == Some(remote::PushRejection::StaleLease)))
    );
    remote::fetch(
        &a.runner,
        &a.options,
        Some("origin"),
        false,
        &token(&a).await,
    )
    .await
    .unwrap();
    assert!(
        matches!(remote::push(&a.runner, &a.options, &p, &token(&a).await).await.unwrap(), RemoteOutcome::Push { success:false, refs } if refs.iter().any(|r| r.summary.contains("(non-fast-forward)") && r.rejection == Some(remote::PushRejection::NeedsUpdate)))
    );
    let mut force = p;
    force.force_with_lease = true;
    assert!(matches!(
        remote::push(&a.runner, &a.options, &force, &token(&a).await)
            .await
            .unwrap(),
        RemoteOutcome::Push { success: true, .. }
    ));
}
#[tokio::test]
async fn update_conflict_abort_and_local_changes() {
    let (a, b, _bare) = fixture().await;
    remote::push(&a.runner, &a.options, &options(), &token(&a).await)
        .await
        .unwrap();
    b.git("fetch", &["origin"]).await;
    b.git("checkout", &["-b", "main", "--track", "origin/main"])
        .await;
    b.put("A", "remote\n");
    b.git("commit", &["-am", "remote"]).await;
    b.git("push", &[]).await;
    a.put("B", "local\n");
    assert!(matches!(
        remote::update(&a.runner, &a.options, &token(&a).await, None)
            .await
            .unwrap(),
        RemoteOutcome::Completed
    ));
    assert_eq!(std::fs::read_to_string(a.path("B")).unwrap(), "local\n");
    a.git("reset", &["--hard"]).await;
    a.put("A", "a divergent\n");
    a.git("commit", &["-am", "a"]).await;
    b.put("A", "b divergent\n");
    b.git("commit", &["-am", "b"]).await;
    b.git("push", &[]).await;
    let head = a.git("rev-parse", &["HEAD"]).await;
    assert!(
        matches!(remote::update(&a.runner,&a.options,&token(&a).await, None).await.unwrap(),RemoteOutcome::Conflicts{files,..} if !files.is_empty())
    );
    remote::abort_update(&a.runner, &a.options, &token(&a).await, None)
        .await
        .unwrap();
    assert_eq!(a.git("rev-parse", &["HEAD"]).await, head);
}
#[tokio::test]
async fn unreachable_and_redaction() {
    let r = Repo::new().await;
    r.initial().await;
    r.git("remote", &["add", "origin", "/nonexistent/gitwood/remote"])
        .await;
    let err = remote::fetch(&r.runner, &r.options, None, false, &token(&r).await)
        .await
        .unwrap_err();
    assert_eq!(err.key, "remote.unreachable");
    assert!(!remote::redact("fatal https://user:secret@host/repo").contains("secret"));
}
#[tokio::test]
async fn fetch_prunes_and_update_rebases() {
    let (a, b, _bare) = fixture().await;
    remote::push(&a.runner, &a.options, &options(), &token(&a).await)
        .await
        .unwrap();
    a.git("push", &["origin", "HEAD:refs/heads/disposable"])
        .await;
    remote::fetch(&b.runner, &b.options, None, true, &token(&b).await)
        .await
        .unwrap();
    b.git("checkout", &["-b", "main", "--track", "origin/main"])
        .await;
    a.git("push", &["origin", ":refs/heads/disposable"]).await;
    b.put("A", "local commit\n");
    b.git("commit", &["-am", "local"]).await;
    a.put("B", "remote commit\n");
    a.git("commit", &["-am", "remote"]).await;
    a.git("push", &[]).await;
    b.git("config", &["pull.rebase", "true"]).await;
    assert!(matches!(
        remote::update(&b.runner, &b.options, &token(&b).await, None)
            .await
            .unwrap(),
        RemoteOutcome::Completed
    ));
    let refs = b
        .git("for-each-ref", &["refs/remotes/origin/disposable"])
        .await;
    assert!(refs.is_empty());
    assert_eq!(
        b.git("log", &["-2", "--format=%s"]).await,
        b"local\nremote\n"
    );
}

#[tokio::test]
async fn update_explicit_strategy_overrides_configuration_and_omitted_uses_it() {
    let (_remote, target, _bare) = diverged(Some("false")).await;
    assert!(matches!(
        remote::update(
            &target.runner,
            &target.options,
            &token(&target).await,
            Some(UpdateStrategy::Rebase),
        )
        .await
        .unwrap(),
        RemoteOutcome::Completed
    ));
    assert_eq!(
        target
            .git("rev-list", &["--count", "--merges", "HEAD"])
            .await,
        b"0\n"
    );

    let (_remote, target, _bare) = diverged(Some("true")).await;
    assert!(matches!(
        remote::update(
            &target.runner,
            &target.options,
            &token(&target).await,
            Some(UpdateStrategy::Merge),
        )
        .await
        .unwrap(),
        RemoteOutcome::Completed
    ));
    assert_eq!(
        target
            .git("rev-list", &["--count", "--merges", "HEAD"])
            .await,
        b"1\n"
    );

    let (_remote, target, _bare) = diverged(Some("true")).await;
    assert!(matches!(
        remote::update(&target.runner, &target.options, &token(&target).await, None)
            .await
            .unwrap(),
        RemoteOutcome::Completed
    ));
    assert_eq!(
        target
            .git("rev-list", &["--count", "--merges", "HEAD"])
            .await,
        b"0\n"
    );
}
#[cfg(unix)]
#[tokio::test]
async fn ssh_error_classification_and_secret_redaction() {
    use gitwood_core::git::GitError;
    let r = Repo::new().await;
    r.initial().await;
    r.git("remote", &["add", "origin", "ssh://host.invalid/repo"])
        .await;
    let helper = r.path("ssh-fail");
    support::script(
        &helper,
        "echo 'Host key verification failed.' >&2\nexit 255",
    );
    let mut options = r.options.clone();
    options
        .env
        .insert("GIT_SSH_COMMAND".into(), helper.into_os_string());
    let err = remote::fetch(&r.runner, &options, None, false, &token(&r).await)
        .await
        .unwrap_err();
    assert_eq!(err.key, "remote.hostKey");
    let err: remote::RemoteError = GitError::Exit {
        code: Some(128),
        stderr: "Authentication failed for 'https://user:token-secret@host/repo'".into(),
    }
    .into();
    assert_eq!(err.key, "remote.authentication");
    assert!(!err.detail.contains("token-secret"));
}
#[tokio::test]
async fn commit_and_push_keeps_two_results_and_does_not_push_failed_commit() {
    use gitwood_core::repo::{inclusion::Selection, session::Sessions, write::ResultState};
    let (a, b, _bare) = fixture().await;
    remote::push(&a.runner, &a.options, &options(), &token(&a).await)
        .await
        .unwrap();
    b.git("fetch", &["origin"]).await;
    b.git("checkout", &["-b", "main", "--track", "origin/main"])
        .await;
    b.put("B", "other\n");
    b.git("commit", &["-am", "other"]).await;
    b.git("push", &[]).await;
    let sessions = Sessions::default();
    let lease = sessions
        .open(a.runner.clone(), a.options.clone())
        .await
        .unwrap();
    assert!(lease
        .commit_and_push(
            &Selection::default(),
            "",
            false,
            &token(&a).await,
            &options(),
            None,
        )
        .await
        .is_err());
    a.put("A", "new commit\n");
    let result = lease
        .commit_and_push(
            &Selection::default(),
            "local",
            false,
            &token(&a).await,
            &options(),
            None,
        )
        .await
        .unwrap();
    assert!(matches!(result.commit, ResultState::Committed { .. }));
    assert!(matches!(
        result.push,
        Some(Ok(RemoteOutcome::Push { success: false, .. }))
    ));
}

#[tokio::test]
async fn preview_session_pages_over_200_commits() {
    use gitwood_core::repo::session::Sessions;
    let (a, _, _bare) = fixture().await;
    for _ in 0..204 {
        a.git("commit", &["--allow-empty", "-qm", "page"]).await;
    }
    let sessions = Sessions::default();
    let lease = sessions
        .open(a.runner.clone(), a.options.clone())
        .await
        .unwrap();
    let first = lease.push_preview(&options()).await.unwrap();
    assert_eq!(first.total, 205);
    assert_eq!(first.commits.len(), 200);
    let last = lease.push_preview_page(200, 500).unwrap();
    assert_eq!(last.commits.len(), 5);
    assert!(!first.commits.contains(&last.commits[0]));
}

#[cfg(unix)]
#[tokio::test]
async fn remote_hook_rejection_and_partial_ref_success() {
    use remote::PushRejection;
    let (a, _, bare) = fixture().await;
    support::script(
        &bare.path().join("hooks/pre-receive"),
        "cat >/dev/null\necho protected >&2\nexit 1",
    );
    let result = remote::push(&a.runner, &a.options, &options(), &token(&a).await)
        .await
        .unwrap();
    assert!(
        matches!(result, RemoteOutcome::Push{success:false, refs} if refs.iter().all(|r| r.rejection == Some(PushRejection::RemoteRule)))
    );
    std::fs::remove_file(bare.path().join("hooks/pre-receive")).unwrap();
    support::script(
        &bare.path().join("hooks/update"),
        "test \"$1\" != refs/tags/blocked",
    );
    a.git("tag", &["blocked"]).await;
    let mut p = options();
    p.tags = true;
    let result = remote::push(&a.runner, &a.options, &p, &token(&a).await)
        .await
        .unwrap();
    assert!(
        matches!(result, RemoteOutcome::Push{success:false, refs} if refs.iter().any(|r|r.flag=='*') && refs.iter().any(|r| r.rejection == Some(PushRejection::RemoteRule)))
    );
}

#[cfg(unix)]
#[tokio::test]
async fn file_remote_permission_denied() {
    use std::os::unix::fs::PermissionsExt;
    let (a, _, bare) = fixture().await;
    a.git(
        "remote",
        &[
            "set-url",
            "origin",
            &format!("file://{}", bare.path().display()),
        ],
    )
    .await;
    std::fs::set_permissions(bare.path(), std::fs::Permissions::from_mode(0o0)).unwrap();
    let result = remote::fetch(&a.runner, &a.options, None, false, &token(&a).await).await;
    std::fs::set_permissions(bare.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(result.is_err());
}

#[cfg(unix)]
#[tokio::test]
async fn fetch_cancel_running_group_heartbeat_and_retry() {
    use std::time::Duration;
    let (a, _, _bare) = fixture().await;
    let ready = a.path("ready");
    let mut signal = support::ProcessSignal::new(ready);
    a.git("config", &["remote.origin.uploadpack", "./blocked-upload"])
        .await;
    support::script(
        &a.path("blocked-upload"),
        "echo $$ > helper.pid\nprintf R > ready\nexec tail -f /dev/null",
    );
    let mut o = a.options.clone();
    let (sender, mut receiver) = gitwood_core::git::progress_channel();
    o.cancellation = gitwood_core::git::CancellationToken::new();
    o.progress = Some(sender);
    o.heartbeat_interval = Duration::from_millis(20);
    let expected = token(&a).await;
    let operation = remote::fetch(&a.runner, &o, None, false, &expected);
    let cancel = async {
        tokio::time::timeout(Duration::from_secs(20), signal.wait())
            .await
            .unwrap()
            .unwrap();
        loop {
            let event = tokio::time::timeout(Duration::from_secs(20), receiver.recv())
                .await
                .unwrap()
                .unwrap();
            if event == "remote.noOutput" {
                break;
            }
        }
        o.cancellation.cancel();
    };
    let (result, ()) = tokio::join!(operation, cancel);
    assert_eq!(result.unwrap_err().key, "remote.cancelled");
    let pid = std::fs::read_to_string(a.path("helper.pid")).unwrap();
    assert!(!std::process::Command::new("kill")
        .args(["-0", pid.trim()])
        .status()
        .unwrap()
        .success());
    a.clean_temporary_files();
    let mut directories = vec![a.path(".git")];
    while let Some(directory) = directories.pop() {
        for entry in std::fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                directories.push(entry.path());
            } else {
                assert!(
                    !entry.file_name().to_string_lossy().ends_with(".lock"),
                    "residual lock: {:?}",
                    entry.path()
                );
            }
        }
    }
    a.git("config", &["--unset", "remote.origin.uploadpack"])
        .await;
    remote::fetch(&a.runner, &a.options, None, false, &token(&a).await)
        .await
        .unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn update_external_stash_is_preserved() {
    let (a, b, _bare) = fixture().await;
    remote::push(&a.runner, &a.options, &options(), &token(&a).await)
        .await
        .unwrap();
    b.git("fetch", &["origin"]).await;
    b.git("checkout", &["-b", "main", "--track", "origin/main"])
        .await;
    b.put("B", "upstream\n");
    b.git("commit", &["-am", "upstream"]).await;
    b.git("push", &[]).await;
    support::script(
        &a.path(".git/hooks/post-merge"),
        "printf external > B\ngit stash push -qm external",
    );
    a.put("A", "local\n");
    assert!(matches!(
        remote::update(&a.runner, &a.options, &token(&a).await, None)
            .await
            .unwrap(),
        RemoteOutcome::Completed
    ));
    assert_eq!(std::fs::read_to_string(a.path("A")).unwrap(), "local\n");
    assert_eq!(
        a.git("stash", &["list", "--format=%s"])
            .await
            .split(|b| *b == b'\n')
            .filter(|s| !s.is_empty())
            .count(),
        1
    );
    assert!(String::from_utf8(a.git("stash", &["show", "-p"]).await)
        .unwrap()
        .contains("external"));
}

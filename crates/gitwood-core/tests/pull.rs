mod support;
use gitwood_core::{
    git::ReadOperation,
    ops::{
        self,
        branch::{self, BranchOperation},
        integrate::{self, Integration, MergeOptions},
        remote::{self, RemoteOutcome},
        sequence::SequenceAction,
    },
    repo,
};
use support::Repo;
async fn token(r: &Repo) -> String {
    ops::snapshot(&r.runner, &r.options)
        .await
        .unwrap()
        .fingerprint
}
async fn run(r: &Repo, request: Integration) -> Result<RemoteOutcome, remote::RemoteError> {
    integrate::run(&r.runner, &r.options, &request, &token(r).await).await
}
fn pull(branches: &[&str], rebase: bool) -> Integration {
    Integration::Pull {
        remote: "origin".into(),
        branches: branches.iter().map(|s| (*s).into()).collect(),
        rebase,
        options: Default::default(),
    }
}
async fn fixture() -> (Repo, Repo, tempfile::TempDir) {
    let a = Repo::new().await;
    a.initial().await;
    a.git("branch", &["-M", "main"]).await;
    let bare = tempfile::tempdir().unwrap();
    a.git("init", &["--bare", bare.path().to_str().unwrap()])
        .await;
    a.git("remote", &["add", "origin", bare.path().to_str().unwrap()])
        .await;
    a.git("push", &["-u", "origin", "main"]).await;
    let b = Repo::new().await;
    b.git("remote", &["add", "origin", bare.path().to_str().unwrap()])
        .await;
    b.git("fetch", &["origin"]).await;
    b.git("checkout", &["-b", "main", "--track", "origin/main"])
        .await;
    (a, b, bare)
}
async fn parents(r: &Repo) -> usize {
    String::from_utf8_lossy(&r.read(ReadOperation::Log, &["-1", "--format=%P"]).await)
        .split_whitespace()
        .count()
}
#[tokio::test]
async fn pull_merge_and_rebase_divergence_with_dirty_worktree() {
    for rebase in [false, true] {
        let (a, b, _bare) = fixture().await;
        a.put("A", "remote\n");
        a.git("commit", &["-am", "remote"]).await;
        a.git("push", &[]).await;
        b.put("B", "local commit\n");
        b.git("commit", &["-am", "local"]).await;
        b.put("B", "local dirty\n");
        b.put("untracked", "keep\n");
        assert!(matches!(
            run(&b, pull(&["main"], rebase)).await.unwrap(),
            RemoteOutcome::Completed
        ));
        assert_eq!(parents(&b).await, if rebase { 1 } else { 2 });
        assert_eq!(std::fs::read_to_string(b.path("A")).unwrap(), "remote\n");
        assert_eq!(
            std::fs::read_to_string(b.path("B")).unwrap(),
            "local dirty\n"
        );
        assert_eq!(
            std::fs::read_to_string(b.path("untracked")).unwrap(),
            "keep\n"
        );
        assert_eq!(b.git("stash", &["list"]).await, b"");
    }
}
#[tokio::test]
async fn pull_multiple_branches_performs_octopus_merge() {
    let (a, b, _bare) = fixture().await;
    for (branch, file) in [("one", "one"), ("two", "two")] {
        a.git("checkout", &["-b", branch, "main"]).await;
        a.put(file, branch);
        a.git("add", &[file]).await;
        a.git("commit", &["-m", branch]).await;
        a.git("push", &["origin", branch]).await;
    }
    assert!(matches!(
        run(&b, pull(&["one", "two"], false)).await.unwrap(),
        RemoteOutcome::Completed
    ));
    assert!(b.path("one").exists() && b.path("two").exists());
    assert_eq!(parents(&b).await, 2);
}
#[tokio::test]
async fn pull_options_ff_only_squash_no_commit() {
    for mode in ["ffOnly", "squash", "noCommit"] {
        let (a, b, _bare) = fixture().await;
        a.put("A", "remote\n");
        a.git("commit", &["-am", "remote"]).await;
        a.git("push", &[]).await;
        b.put("B", "local\n");
        b.git("commit", &["-am", "local"]).await;
        let options = MergeOptions {
            ff_only: mode == "ffOnly",
            squash: mode == "squash",
            no_commit: mode == "noCommit",
            ..Default::default()
        };
        let result = run(
            &b,
            Integration::Pull {
                remote: "origin".into(),
                branches: vec!["main".into()],
                rebase: false,
                options,
            },
        )
        .await;
        match mode {
            "ffOnly" => assert!(result.unwrap_err().detail.contains("fast-forward")),
            "squash" => {
                assert!(matches!(result.unwrap(), RemoteOutcome::Completed));
                assert!(!b.path(".git/MERGE_HEAD").exists());
                assert!(!repo::status::read(&b.runner, &b.options)
                    .await
                    .unwrap()
                    .staged
                    .is_empty());
            }
            _ => {
                assert!(matches!(result.unwrap(), RemoteOutcome::Interrupted { .. }));
                assert!(b.path(".git/MERGE_HEAD").exists());
            }
        }
    }
}
#[tokio::test]
async fn pull_conflict_and_abort_restore_saved_local_changes() {
    let (a, b, _bare) = fixture().await;
    a.put("A", "remote\n");
    a.git("commit", &["-am", "remote"]).await;
    a.git("push", &[]).await;
    b.put("A", "local\n");
    b.git("commit", &["-am", "local"]).await;
    b.put("B", "unsaved\n");
    assert!(matches!(
        run(&b, pull(&["main"], true)).await.unwrap(),
        RemoteOutcome::Conflicts { .. }
    ));
    assert!(matches!(
        remote::continue_update(
            &b.runner,
            &b.options,
            &token(&b).await,
            None,
            SequenceAction::Abort,
            None
        )
        .await
        .unwrap(),
        RemoteOutcome::Completed
    ));
    assert_eq!(std::fs::read_to_string(b.path("B")).unwrap(), "unsaved\n");
    assert_eq!(std::fs::read_to_string(b.path("A")).unwrap(), "local\n");
}
#[tokio::test]
async fn invalid_pull_branches_and_multibranch_rebase_do_not_change_head() {
    let (_a, b, _bare) = fixture().await;
    let before = token(&b).await;
    for request in [
        pull(&["-main"], false),
        pull(&["main", "other"], true),
        pull(&["main", "main"], false),
        pull(&[], false),
        Integration::Pull {
            remote: "-origin".into(),
            branches: vec!["main".into()],
            rebase: false,
            options: Default::default(),
        },
        Integration::Pull {
            remote: "origin".into(),
            branches: vec!["main".into()],
            rebase: true,
            options: MergeOptions {
                no_ff: true,
                ..Default::default()
            },
        },
    ] {
        assert!(run(&b, request).await.is_err());
        assert_eq!(token(&b).await, before);
    }
}
#[tokio::test]
async fn set_and_unset_upstream_preserve_local_files() {
    let (_a, b, _bare) = fixture().await;
    b.put("B", "dirty\n");
    for upstream in [
        None,
        Some("refs/remotes/origin/main".into()),
        Some("refs/heads/local".into()),
    ] {
        b.git("branch", &["-f", "local", "HEAD"]).await;
        branch::branch(
            &b.runner,
            &b.options,
            &BranchOperation::SetUpstream {
                name: "main".into(),
                upstream: upstream.clone(),
            },
            &token(&b).await,
        )
        .await
        .unwrap();
        let current = repo::refs::read(&b.runner, &b.options)
            .await
            .unwrap()
            .local
            .into_iter()
            .find(|r| r.name == "refs/heads/main")
            .unwrap();
        assert_eq!(current.upstream, upstream);
        assert_eq!(std::fs::read_to_string(b.path("B")).unwrap(), "dirty\n");
    }
    assert!(branch::branch(
        &b.runner,
        &b.options,
        &BranchOperation::SetUpstream {
            name: "--bad".into(),
            upstream: None
        },
        &token(&b).await
    )
    .await
    .is_err());
}
#[tokio::test]
async fn remote_checkout_rebase_creates_tracking_branch_and_preserves_current_branch() {
    let (a, b, _bare) = fixture().await;
    a.git("checkout", &["-b", "topic"]).await;
    a.put("A", "topic\n");
    a.git("commit", &["-am", "topic"]).await;
    a.git("push", &["origin", "topic"]).await;
    b.git("fetch", &["origin"]).await;
    b.put("B", "main\n");
    b.git("commit", &["-am", "main"]).await;
    let main = b.read(ReadOperation::RevParse, &["HEAD"]).await;
    assert!(matches!(
        run(
            &b,
            Integration::CheckoutRebase {
                branch: "refs/remotes/origin/topic".into(),
                local_name: Some("local-topic".into())
            }
        )
        .await
        .unwrap(),
        RemoteOutcome::Completed
    ));
    assert_eq!(b.read(ReadOperation::RevParse, &["HEAD~1"]).await, main);
    let refs = repo::refs::read(&b.runner, &b.options).await.unwrap();
    assert_eq!(refs.head.head.as_deref(), Some("local-topic"));
    assert_eq!(
        refs.local
            .iter()
            .find(|r| r.name == "refs/heads/local-topic")
            .unwrap()
            .upstream
            .as_deref(),
        Some("refs/remotes/origin/topic")
    );
    assert!(run(
        &b,
        Integration::CheckoutRebase {
            branch: "refs/remotes/origin/topic".into(),
            local_name: Some("main".into())
        }
    )
    .await
    .is_err());
}

#[tokio::test]
async fn configured_remote_is_available_before_first_fetch_and_cannot_mutate_via_read() {
    let r = Repo::new().await;
    r.initial().await;
    r.git("remote", &["add", "origin", "/nonexistent/local/bare"])
        .await;
    assert_eq!(
        repo::refs::read(&r.runner, &r.options)
            .await
            .unwrap()
            .remotes
            .get("origin")
            .unwrap()
            .len(),
        0
    );
    assert!(r
        .runner
        .run(
            gitwood_core::git::ReadCommand::new(ReadOperation::RemoteNames)
                .args(["remove", "origin"]),
            &r.options
        )
        .await
        .is_err());
    assert!(repo::refs::read(&r.runner, &r.options)
        .await
        .unwrap()
        .remotes
        .contains_key("origin"));
}

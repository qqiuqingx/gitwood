mod support;
use gitwood_core::{
    git::ReadOperation,
    ops::{
        self,
        integrate::{self, Integration, MergeOptions},
        remote::{self, RemoteOutcome},
        sequence::SequenceAction,
    },
    repo::{self, operation::OperationKind},
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
fn merge(options: MergeOptions) -> Integration {
    Integration::Merge {
        target: "refs/heads/topic".into(),
        options,
    }
}
async fn fixture(diverge: bool) -> Repo {
    let r = Repo::new().await;
    r.initial().await;
    r.git("branch", &["-M", "main"]).await;
    r.git("checkout", &["-b", "topic"]).await;
    r.put("A", "topic\n");
    r.git("commit", &["-am", "topic"]).await;
    r.git("checkout", &["main"]).await;
    if diverge {
        r.put("B", "main\n");
        r.git("commit", &["-am", "main"]).await;
    }
    r
}
async fn text(r: &Repo, op: ReadOperation, args: &[&str]) -> String {
    String::from_utf8(r.read(op, args).await)
        .unwrap()
        .trim()
        .into()
}
async fn parents(r: &Repo) -> usize {
    text(r, ReadOperation::Log, &["-1", "--format=%P"])
        .await
        .split_whitespace()
        .count()
}
#[tokio::test]
async fn merge_fast_forward_and_no_ff_message() {
    let r = fixture(false).await;
    assert!(matches!(
        run(&r, merge(Default::default())).await.unwrap(),
        RemoteOutcome::Completed
    ));
    assert_eq!(parents(&r).await, 1);
    let r = fixture(false).await;
    let options = MergeOptions {
        no_ff: true,
        message: Some("chosen message\n\nbody".into()),
        ..Default::default()
    };
    assert!(matches!(
        run(&r, merge(options)).await.unwrap(),
        RemoteOutcome::Completed
    ));
    assert_eq!(parents(&r).await, 2);
    assert_eq!(
        text(&r, ReadOperation::Log, &["-1", "--format=%B"]).await,
        "chosen message\n\nbody"
    );
}
#[tokio::test]
async fn ff_only_refuses_divergence_and_restores_local_changes() {
    let r = fixture(true).await;
    r.put("B", "dirty main\n");
    let head = r.read(ReadOperation::RevParse, &["HEAD"]).await;
    let error = run(
        &r,
        merge(MergeOptions {
            ff_only: true,
            ..Default::default()
        }),
    )
    .await
    .unwrap_err();
    assert!(error.detail.contains("fast-forward"), "{error:?}");
    assert_eq!(head, r.read(ReadOperation::RevParse, &["HEAD"]).await);
    assert_eq!(
        std::fs::read_to_string(r.path("B")).unwrap(),
        "dirty main\n"
    );
    assert!(!r.path(".git/gitwood-update-stash").exists());
}
#[tokio::test]
async fn squash_stages_without_merge_commit() {
    let r = fixture(true).await;
    let head = r.read(ReadOperation::RevParse, &["HEAD"]).await;
    assert!(matches!(
        run(
            &r,
            merge(MergeOptions {
                squash: true,
                ..Default::default()
            })
        )
        .await
        .unwrap(),
        RemoteOutcome::Completed
    ));
    assert_eq!(head, r.read(ReadOperation::RevParse, &["HEAD"]).await);
    assert!(!r.path(".git/MERGE_HEAD").exists());
    assert!(!repo::status::read(&r.runner, &r.options)
        .await
        .unwrap()
        .staged
        .is_empty());
    r.git("commit", &["-m", "squashed"]).await;
    assert_eq!(parents(&r).await, 1);
}
#[tokio::test]
async fn no_commit_keeps_merge_head_and_stash_until_continue() {
    let r = fixture(false).await;
    r.put("B", "local edit\n");
    assert!(matches!(
        run(
            &r,
            merge(MergeOptions {
                no_ff: true,
                no_commit: true,
                ..Default::default()
            })
        )
        .await
        .unwrap(),
        RemoteOutcome::Interrupted { .. }
    ));
    assert!(r.path(".git/MERGE_HEAD").exists());
    assert!(r.path(".git/gitwood-update-stash").exists());
    assert!(matches!(
        remote::continue_update(
            &r.runner,
            &r.options,
            &token(&r).await,
            None,
            SequenceAction::Continue,
            Some("reviewed merge")
        )
        .await
        .unwrap(),
        RemoteOutcome::Completed
    ));
    assert_eq!(parents(&r).await, 2);
    assert_eq!(
        std::fs::read_to_string(r.path("B")).unwrap(),
        "local edit\n"
    );
    assert!(!r.path(".git/gitwood-update-stash").exists());
}
#[tokio::test]
async fn no_commit_fast_forward_does_not_pause() {
    let r = fixture(false).await;
    assert!(matches!(
        run(
            &r,
            merge(MergeOptions {
                no_commit: true,
                ..Default::default()
            })
        )
        .await
        .unwrap(),
        RemoteOutcome::Completed
    ));
    assert!(!r.path(".git/MERGE_HEAD").exists());
}
#[tokio::test]
async fn merge_conflicts_can_abort_and_restore_stash() {
    let r = fixture(false).await;
    r.put("A", "main\n");
    r.git("commit", &["-am", "main"]).await;
    r.put("B", "local\n");
    assert!(matches!(
        run(&r, merge(Default::default())).await.unwrap(),
        RemoteOutcome::Conflicts { .. }
    ));
    assert_eq!(
        repo::operation::read(&r.path(".git"))
            .unwrap()
            .unwrap()
            .kind,
        OperationKind::Merge
    );
    assert!(matches!(
        remote::abort_update(&r.runner, &r.options, &token(&r).await, None)
            .await
            .unwrap(),
        RemoteOutcome::Completed
    ));
    assert_eq!(std::fs::read_to_string(r.path("B")).unwrap(), "local\n");
}
#[tokio::test]
async fn rebase_target_and_onto_transplant_only_selected_range() {
    for advanced in [false, true] {
        let r = fixture(true).await;
        let main = text(&r, ReadOperation::RevParse, &["main"]).await;
        r.git("checkout", &["topic"]).await;
        let old = text(&r, ReadOperation::RevParse, &["HEAD~1"]).await;
        let request = if advanced {
            Integration::Rebase {
                target: old,
                onto: Some(main.clone()),
            }
        } else {
            Integration::Rebase {
                target: "refs/heads/main".into(),
                onto: None,
            }
        };
        assert!(matches!(
            run(&r, request).await.unwrap(),
            RemoteOutcome::Completed
        ));
        assert_eq!(text(&r, ReadOperation::RevParse, &["HEAD~1"]).await, main);
        assert_eq!(std::fs::read_to_string(r.path("A")).unwrap(), "topic\n");
        assert_eq!(std::fs::read_to_string(r.path("B")).unwrap(), "main\n");
    }
}
#[tokio::test]
async fn rebase_continuous_conflicts_restore_saved_changes_once() {
    let r = fixture(false).await;
    r.put("A", "main conflict\n");
    r.git("commit", &["-am", "main"]).await;
    r.git("checkout", &["topic"]).await;
    r.put("A", "topic second\n");
    r.git("commit", &["-am", "second"]).await;
    r.put("B", "local edit\n");
    assert!(matches!(
        run(
            &r,
            Integration::Rebase {
                target: "refs/heads/main".into(),
                onto: None
            }
        )
        .await
        .unwrap(),
        RemoteOutcome::Conflicts { .. }
    ));
    for (i, expected) in [(0, "resolved first\n"), (1, "resolved second\n")] {
        r.put("A", expected);
        r.git("add", &["A"]).await;
        let value = remote::continue_update(
            &r.runner,
            &r.options,
            &token(&r).await,
            None,
            SequenceAction::Continue,
            None,
        )
        .await
        .unwrap();
        if i == 0 {
            assert!(matches!(value, RemoteOutcome::Conflicts { .. }));
        } else {
            assert!(matches!(value, RemoteOutcome::Completed));
        }
    }
    assert_eq!(
        std::fs::read_to_string(r.path("B")).unwrap(),
        "local edit\n"
    );
    assert_eq!(r.git("stash", &["list"]).await, b"");
}
#[tokio::test]
async fn checkout_and_rebase_is_one_protected_operation() {
    let r = fixture(true).await;
    let main = text(&r, ReadOperation::RevParse, &["main"]).await;
    r.put("B", "local main edit\n");
    assert!(matches!(
        run(
            &r,
            Integration::CheckoutRebase {
                branch: "refs/heads/topic".into(),
                local_name: None
            }
        )
        .await
        .unwrap(),
        RemoteOutcome::Completed
    ));
    assert_eq!(
        text(&r, ReadOperation::RevParse, &["--abbrev-ref", "HEAD"]).await,
        "topic"
    );
    assert_eq!(text(&r, ReadOperation::RevParse, &["HEAD~1"]).await, main);
    assert_eq!(
        std::fs::read_to_string(r.path("B")).unwrap(),
        "local main edit\n"
    );
}
#[tokio::test]
async fn invalid_targets_options_concurrency_and_locks_are_rejected_without_writes() {
    let r = fixture(true).await;
    let before = token(&r).await;
    let mut requests = vec![
        Integration::Rebase {
            target: "--exec=touch bad".into(),
            onto: None,
        },
        Integration::CheckoutRebase {
            branch: "-bad".into(),
            local_name: None,
        },
        Integration::Rebase {
            target: "main".into(),
            onto: Some("-bad".into()),
        },
        Integration::Merge {
            target: "-bad".into(),
            options: Default::default(),
        },
    ];
    for options in [
        MergeOptions {
            no_ff: true,
            ff_only: true,
            ..Default::default()
        },
        MergeOptions {
            squash: true,
            message: Some("x".into()),
            ..Default::default()
        },
        MergeOptions {
            no_commit: true,
            message: Some("x".into()),
            ..Default::default()
        },
    ] {
        requests.push(merge(options));
    }
    for request in requests {
        assert!(run(&r, request).await.is_err());
        assert_eq!(token(&r).await, before);
    }
    assert!(
        integrate::run(&r.runner, &r.options, &merge(Default::default()), "stale")
            .await
            .is_err()
    );
    r.put(".git/index.lock", "external");
    assert!(run(&r, merge(Default::default()))
        .await
        .unwrap_err()
        .detail
        .contains("indexLocked"));
    assert_eq!(
        std::fs::read_to_string(r.path(".git/index.lock")).unwrap(),
        "external"
    );
}
#[tokio::test]
async fn untracked_overwrites_are_refused_and_user_files_survive() {
    let r = fixture(false).await;
    r.git("checkout", &["topic"]).await;
    r.put("new", "incoming\n");
    r.git("add", &["new"]).await;
    r.git("commit", &["-m", "new"]).await;
    r.git("checkout", &["main"]).await;
    r.put("new", "user\n");
    r.put("B", "dirty\n");
    assert!(run(&r, merge(Default::default())).await.is_err());
    assert_eq!(std::fs::read_to_string(r.path("new")).unwrap(), "user\n");
    assert_eq!(std::fs::read_to_string(r.path("B")).unwrap(), "dirty\n");
}
#[cfg(unix)]
#[tokio::test]
async fn merge_preserves_hook_rejection_and_does_not_claim_success() {
    let r = fixture(true).await;
    support::script(
        &r.path(".git/hooks/pre-merge-commit"),
        "echo 'hook refuses merge' >&2; exit 1",
    );
    let value = run(&r, merge(Default::default())).await.unwrap();
    assert!(
        matches!(value, RemoteOutcome::Interrupted { error, .. } if error.detail.contains("hook refuses merge"))
    );
}

#[tokio::test]
async fn rebase_skip_restores_saved_changes_and_stash_apply_conflicts_are_retained() {
    let r = fixture(false).await;
    r.put("A", "main conflict\n");
    r.git("commit", &["-am", "main"]).await;
    r.git("checkout", &["topic"]).await;
    r.put("B", "dirty\n");
    assert!(matches!(
        run(
            &r,
            Integration::Rebase {
                target: "refs/heads/main".into(),
                onto: None
            }
        )
        .await
        .unwrap(),
        RemoteOutcome::Conflicts { .. }
    ));
    assert!(matches!(
        remote::continue_update(
            &r.runner,
            &r.options,
            &token(&r).await,
            None,
            SequenceAction::Skip,
            None
        )
        .await
        .unwrap(),
        RemoteOutcome::Completed
    ));
    assert_eq!(std::fs::read_to_string(r.path("B")).unwrap(), "dirty\n");
    let r = fixture(false).await;
    r.put("A", "local dirty\n");
    let result = run(&r, merge(Default::default())).await.unwrap();
    let RemoteOutcome::StashRecovery { stash, files, .. } = result else {
        panic!("{result:?}")
    };
    assert!(!files.is_empty());
    assert!(
        String::from_utf8_lossy(&r.git("stash", &["list", "--format=%H"]).await).contains(&stash)
    );
    assert!(!r.path(".git/gitwood-update-stash").exists());
    assert!(run(&r, merge(Default::default())).await.is_err());
}
#[tokio::test]
async fn session_integration_uses_expected_snapshot_and_cancel_boundary() {
    let r = fixture(true).await;
    let sessions = repo::session::Sessions::default();
    let lease = sessions
        .open(r.runner.clone(), r.options.clone())
        .await
        .unwrap();
    let (progress, _events) = gitwood_core::git::progress_channel();
    let request = merge(Default::default());
    assert!(lease
        .integrate(&request, "stale", progress.clone(), None)
        .await
        .is_err());
    assert!(matches!(
        lease
            .integrate(&request, &token(&r).await, progress, None)
            .await
            .unwrap(),
        RemoteOutcome::Completed
    ));
    assert_eq!(parents(&r).await, 2);
    let r = fixture(true).await;
    let mut options = r.options.clone();
    options.cancellation = gitwood_core::git::CancellationToken::new();
    options.cancellation.cancel();
    assert_eq!(
        integrate::run(&r.runner, &options, &request, &token(&r).await)
            .await
            .unwrap_err()
            .key,
        "remote.cancelled"
    );
    assert!(!r.path(".git/MERGE_HEAD").exists());
}

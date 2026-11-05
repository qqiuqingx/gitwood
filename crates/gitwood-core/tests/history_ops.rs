mod support;
use gitwood_core::{
    git::ReadOperation,
    ops::{
        history::{self, EmptyCherryPick, HistoryOutcome, HistoryWrite, ResetMode},
        sequence::{self, SequenceAction},
        snapshot,
    },
};
use support::Repo;

async fn head(r: &Repo) -> String {
    String::from_utf8(r.read(ReadOperation::RevParse, &["HEAD"]).await)
        .unwrap()
        .trim()
        .into()
}
async fn apply(r: &Repo, request: HistoryWrite) -> HistoryOutcome {
    execute(r, request).await.unwrap()
}
async fn execute(
    r: &Repo,
    request: HistoryWrite,
) -> Result<HistoryOutcome, gitwood_core::ops::CommitError> {
    let expected = snapshot(&r.runner, &r.options).await.unwrap().fingerprint;
    history::execute(&r.runner, &r.options, &request, &expected).await
}

#[tokio::test]
async fn cherry_pick_orders_old_to_new_and_records_origin() {
    let mut r = Repo::new().await;
    r.initial().await;
    let branch = String::from_utf8(r.git("branch", &["--show-current"]).await)
        .unwrap()
        .trim()
        .to_owned();
    for key in ["GIT_AUTHOR_DATE", "GIT_COMMITTER_DATE"] {
        r.options
            .env
            .insert(key.into(), "2000-01-01T00:00:00+00:00".into());
    }
    r.git("checkout", &["-qb", "source"]).await;
    r.put("A", "first\n");
    r.git("commit", &["-qam", "first"]).await;
    let first = head(&r).await;
    r.put("B", "second\n");
    r.git("commit", &["-qam", "second"]).await;
    let second = head(&r).await;
    r.options
        .env
        .remove(std::ffi::OsStr::new("GIT_AUTHOR_DATE"));
    r.options
        .env
        .remove(std::ffi::OsStr::new("GIT_COMMITTER_DATE"));
    r.git("update-ref", &["refs/remotes/origin/source", &second])
        .await;
    r.git("checkout", &["-q", &branch]).await;
    assert!(matches!(
        apply(
            &r,
            HistoryWrite::CherryPick {
                commits: vec![second.clone(), first.clone()],
                record_origin: true,
                empty: EmptyCherryPick::Skip
            }
        )
        .await,
        HistoryOutcome::Completed
    ));
    assert_eq!(std::fs::read_to_string(r.path("A")).unwrap(), "first\n");
    assert_eq!(std::fs::read_to_string(r.path("B")).unwrap(), "second\n");
    let messages = r.read(ReadOperation::Log, &["-2", "--format=%B"]).await;
    let messages = String::from_utf8_lossy(&messages);
    assert!(messages.contains(&first));
    assert!(messages.contains(&second));
}

#[tokio::test]
async fn cherry_pick_and_revert_do_not_commit_existing_staged_changes() {
    let r = Repo::new().await;
    r.initial().await;
    r.put("A", "existing\n");
    r.git("commit", &["-qam", "existing"]).await;
    let revert_target = head(&r).await;
    let branch = String::from_utf8(r.git("branch", &["--show-current"]).await)
        .unwrap()
        .trim()
        .to_owned();
    r.git("checkout", &["-qb", "topic"]).await;
    r.put("B", "picked\n");
    r.git("commit", &["-qam", "picked"]).await;
    let pick_target = head(&r).await;
    r.git("checkout", &["-q", &branch]).await;
    r.put("C", "staged\n");
    r.git("add", &["--", "C"]).await;
    let before = head(&r).await;
    assert!(execute(
        &r,
        HistoryWrite::CherryPick {
            commits: vec![pick_target],
            record_origin: false,
            empty: EmptyCherryPick::Skip
        }
    )
    .await
    .is_err());
    assert_eq!(head(&r).await, before);
    assert!(r
        .git("diff", &["--cached", "--name-only"])
        .await
        .ends_with(b"C\n"));
    assert!(execute(
        &r,
        HistoryWrite::Revert {
            commits: vec![revert_target]
        }
    )
    .await
    .is_err());
    assert_eq!(head(&r).await, before);
}

#[tokio::test]
async fn cherry_pick_empty_commit_skip_and_conflict_continue() {
    let r = Repo::new().await;
    r.initial().await;
    let branch = String::from_utf8(r.git("branch", &["--show-current"]).await)
        .unwrap()
        .trim()
        .to_owned();
    r.git("checkout", &["-qb", "source"]).await;
    r.put("A", "picked\n");
    r.git("commit", &["-qam", "picked"]).await;
    let picked = head(&r).await;
    r.git("commit", &["--allow-empty", "-qm", "empty"]).await;
    let empty = head(&r).await;
    r.git("checkout", &["-q", &branch]).await;
    r.put("A", "picked\n");
    r.git("commit", &["-qam", "already applied"]).await;
    assert!(matches!(
        apply(
            &r,
            HistoryWrite::CherryPick {
                commits: vec![empty, picked],
                record_origin: false,
                empty: EmptyCherryPick::Skip
            }
        )
        .await,
        HistoryOutcome::Completed
    ));

    let empty_repo = Repo::new().await;
    empty_repo.initial().await;
    let branch = String::from_utf8(empty_repo.git("branch", &["--show-current"]).await)
        .unwrap()
        .trim()
        .to_owned();
    empty_repo.git("checkout", &["-qb", "source"]).await;
    empty_repo.put("A", "same\n");
    empty_repo.git("commit", &["-qam", "source change"]).await;
    let source = head(&empty_repo).await;
    empty_repo.git("checkout", &["-q", &branch]).await;
    empty_repo.put("A", "same\n");
    empty_repo
        .git("commit", &["-qam", "same change already here"])
        .await;
    let before = String::from_utf8(empty_repo.git("rev-list", &["--count", "HEAD"]).await).unwrap();
    assert!(matches!(
        apply(
            &empty_repo,
            HistoryWrite::CherryPick {
                commits: vec![source],
                record_origin: false,
                empty: EmptyCherryPick::CreateEmpty
            }
        )
        .await,
        HistoryOutcome::Completed
    ));
    let after = String::from_utf8(empty_repo.git("rev-list", &["--count", "HEAD"]).await).unwrap();
    assert_eq!(
        after.trim().parse::<usize>().unwrap(),
        before.trim().parse::<usize>().unwrap() + 1
    );
    assert_eq!(
        empty_repo.git("show", &["-s", "--format=%T", "HEAD"]).await,
        empty_repo
            .git("show", &["-s", "--format=%T", "HEAD^"])
            .await
    );

    let next = Repo::new().await;
    next.initial().await;
    let branch = String::from_utf8(next.git("branch", &["--show-current"]).await)
        .unwrap()
        .trim()
        .to_owned();
    next.git("checkout", &["-qb", "source"]).await;
    next.put("A", "incoming\n");
    next.git("commit", &["-qam", "incoming"]).await;
    let incoming = head(&next).await;
    next.git("checkout", &["-q", &branch]).await;
    next.put("A", "local\n");
    next.git("commit", &["-qam", "local"]).await;
    assert!(matches!(
        apply(
            &next,
            HistoryWrite::CherryPick {
                commits: vec![incoming],
                record_origin: false,
                empty: EmptyCherryPick::Skip
            }
        )
        .await,
        HistoryOutcome::Conflicts { .. }
    ));
    next.put("A", "resolved\n");
    next.git("add", &["--", "A"]).await;
    let fingerprint = snapshot(&next.runner, &next.options)
        .await
        .unwrap()
        .fingerprint;
    assert!(matches!(
        sequence::run(
            &next.runner,
            &next.options,
            &fingerprint,
            SequenceAction::Continue,
            None
        )
        .await
        .unwrap(),
        sequence::SequenceOutcome::Completed
    ));
}

#[tokio::test]
async fn revert_many_and_reject_merge() {
    let r = Repo::new().await;
    r.initial().await;
    r.put("A", "one\n");
    r.git("commit", &["-qam", "one"]).await;
    let one = head(&r).await;
    r.put("A", "two\n");
    r.git("commit", &["-qam", "two"]).await;
    let two = head(&r).await;
    assert!(matches!(
        apply(
            &r,
            HistoryWrite::Revert {
                commits: vec![two, one]
            }
        )
        .await,
        HistoryOutcome::Completed
    ));
    assert_eq!(std::fs::read_to_string(r.path("A")).unwrap(), "A base\n");
    let current = String::from_utf8(r.git("branch", &["--show-current"]).await)
        .unwrap()
        .trim()
        .to_owned();
    r.git("checkout", &["-qb", "side"]).await;
    r.put("B", "side\n");
    r.git("commit", &["-qam", "side"]).await;
    r.git("checkout", &["-q", &current]).await;
    r.put("A", "main\n");
    r.git("commit", &["-qam", "main"]).await;
    r.git("merge", &["--no-ff", "side", "-m", "merge"]).await;
    assert!(
        history::selection_has_merge(&r.runner, &r.options, &[head(&r).await])
            .await
            .unwrap()
    );
    assert!(history::execute(
        &r.runner,
        &r.options,
        &HistoryWrite::Revert {
            commits: vec![head(&r).await]
        },
        &snapshot(&r.runner, &r.options).await.unwrap().fingerprint
    )
    .await
    .is_err());
}

#[tokio::test]
async fn reset_modes_checkout_revision_tags_children_and_paged_tree() {
    let r = Repo::new().await;
    r.initial().await;
    let base = head(&r).await;
    r.put("A", "next\n");
    r.git("commit", &["-qam", "next"]).await;
    let next = head(&r).await;
    assert!(matches!(
        apply(
            &r,
            HistoryWrite::Reset {
                target: base.clone(),
                mode: ResetMode::Soft
            }
        )
        .await,
        HistoryOutcome::Completed
    ));
    assert_eq!(head(&r).await, base);
    assert!(!r.git("diff", &["--cached", "--name-only"]).await.is_empty());
    r.git("reset", &["--hard", &next]).await;
    assert!(matches!(
        apply(
            &r,
            HistoryWrite::Reset {
                target: base.clone(),
                mode: ResetMode::Mixed
            }
        )
        .await,
        HistoryOutcome::Completed
    ));
    assert_eq!(head(&r).await, base);
    assert!(r.git("diff", &["--cached", "--name-only"]).await.is_empty());
    assert!(!r.git("diff", &["--name-only"]).await.is_empty());
    assert!(matches!(
        apply(
            &r,
            HistoryWrite::Reset {
                target: base.clone(),
                mode: ResetMode::Hard
            }
        )
        .await,
        HistoryOutcome::Completed
    ));
    assert_eq!(std::fs::read_to_string(r.path("A")).unwrap(), "A base\n");
    r.git("reset", &["--hard", &next]).await;

    let branch = String::from_utf8(r.git("branch", &["--show-current"]).await)
        .unwrap()
        .trim()
        .to_owned();
    assert!(matches!(
        apply(
            &r,
            HistoryWrite::CheckoutRevision {
                target: base.clone(),
                mode: gitwood_core::ops::branch::CheckoutMode::Normal
            }
        )
        .await,
        HistoryOutcome::Completed
    ));
    r.git("checkout", &["-q", &branch]).await;
    r.put("A", "dirty\n");
    assert!(matches!(
        apply(
            &r,
            HistoryWrite::Reset {
                target: base.clone(),
                mode: ResetMode::Keep
            }
        )
        .await,
        HistoryOutcome::Stopped { .. }
    ));
    assert_eq!(head(&r).await, next);
    r.git("reset", &["--hard", &next]).await;

    assert!(matches!(
        apply(
            &r,
            HistoryWrite::CreateTag {
                name: "light".into(),
                target: next.clone(),
                message: None
            }
        )
        .await,
        HistoryOutcome::Completed
    ));
    assert!(execute(
        &r,
        HistoryWrite::CreateTag {
            name: "light".into(),
            target: next.clone(),
            message: Some("annotation".into())
        }
    )
    .await
    .is_err());
    assert!(matches!(
        apply(
            &r,
            HistoryWrite::CreateTag {
                name: "annotated".into(),
                target: next.clone(),
                message: Some("release".into())
            }
        )
        .await,
        HistoryOutcome::Completed
    ));
    assert!(matches!(
        apply(
            &r,
            HistoryWrite::DeleteTag {
                name: "light".into()
            }
        )
        .await,
        HistoryOutcome::Completed
    ));
    r.git("checkout", &["-qb", "side", &base]).await;
    r.put("B", "side child\n");
    r.git("commit", &["-qam", "side child"]).await;
    r.git("checkout", &["-q", &branch]).await;
    let children = history::children(&r.runner, &r.options, &base)
        .await
        .unwrap();
    assert!(children.children.contains(&next));
    assert_eq!(children.children.len(), 2);

    let sessions = gitwood_core::repo::session::Sessions::default();
    let lease = sessions
        .open(r.runner.clone(), r.options.clone())
        .await
        .unwrap();
    let summary = lease.history_tree_open(&next).await.unwrap();
    assert!(summary.files > 0);
    let page = lease
        .history_tree_files(
            &summary.id,
            &std::collections::HashSet::from(["files".to_owned()]),
            0,
            5,
        )
        .await
        .unwrap();
    assert!(page.total > 0 && page.rows.len() <= 5);
    let file = page
        .rows
        .iter()
        .find_map(|row| row.node.path.clone())
        .unwrap();
    let diff = lease.history_tree_diff(&summary.id, &file).await.unwrap();
    assert_eq!(diff.right_label, next);
}

#[tokio::test]
async fn checkout_revision_local_changes_and_remote_tag_operations() {
    let r = Repo::new().await;
    r.initial().await;
    let base = head(&r).await;
    let bare = tempfile::tempdir().unwrap();
    r.git("init", &["--bare", bare.path().to_str().unwrap()])
        .await;
    r.git("remote", &["add", "origin", bare.path().to_str().unwrap()])
        .await;
    assert!(matches!(
        apply(
            &r,
            HistoryWrite::CreateTag {
                name: "v1".into(),
                target: base.clone(),
                message: None
            }
        )
        .await,
        HistoryOutcome::Completed
    ));
    assert!(matches!(
        apply(
            &r,
            HistoryWrite::PushTag {
                remote: "origin".into(),
                name: "v1".into()
            }
        )
        .await,
        HistoryOutcome::Completed
    ));
    assert!(!r
        .git("ls-remote", &["origin", "refs/tags/v1"])
        .await
        .is_empty());
    assert!(matches!(
        apply(
            &r,
            HistoryWrite::DeleteRemoteTag {
                remote: "origin".into(),
                name: "v1".into()
            }
        )
        .await,
        HistoryOutcome::Completed
    ));
    assert!(r
        .git("ls-remote", &["origin", "refs/tags/v1"])
        .await
        .is_empty());

    r.put("A", "next\n");
    r.git("commit", &["-qam", "next"]).await;
    r.put("A", "local\n");
    assert!(matches!(
        apply(
            &r,
            HistoryWrite::CheckoutRevision {
                target: base.clone(),
                mode: gitwood_core::ops::branch::CheckoutMode::Normal
            }
        )
        .await,
        HistoryOutcome::LocalChanges { .. }
    ));
    r.put("A", "next\n");
    r.put("B", "local\n");
    assert!(matches!(
        apply(
            &r,
            HistoryWrite::CheckoutRevision {
                target: base.clone(),
                mode: gitwood_core::ops::branch::CheckoutMode::Smart
            }
        )
        .await,
        HistoryOutcome::Completed
    ));
    assert_eq!(std::fs::read_to_string(r.path("A")).unwrap(), "A base\n");
    assert_eq!(std::fs::read_to_string(r.path("B")).unwrap(), "local\n");
    assert_eq!(
        r.git("rev-parse", &["--abbrev-ref", "HEAD"]).await,
        b"HEAD\n"
    );
}

#[tokio::test]
async fn malicious_revision_and_tag_inputs_are_rejected() {
    let r = Repo::new().await;
    r.initial().await;
    assert!(execute(
        &r,
        HistoryWrite::CreateTag {
            name: "-bad".into(),
            target: head(&r).await,
            message: None
        }
    )
    .await
    .is_err());
    assert!(execute(
        &r,
        HistoryWrite::CheckoutRevision {
            target: "--help".into(),
            mode: gitwood_core::ops::branch::CheckoutMode::Normal
        }
    )
    .await
    .is_err());
}

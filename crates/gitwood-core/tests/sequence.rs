mod support;
use gitwood_core::{
    git::{ReadOperation, WriteCommand},
    ops::{
        operation_state,
        sequence::{run, SequenceAction, SequenceOutcome},
        snapshot,
    },
    repo::operation::OperationKind,
};
use support::Repo;

async fn token(r: &Repo) -> String {
    snapshot(&r.runner, &r.options).await.unwrap().fingerprint
}
async fn head(r: &Repo) -> String {
    String::from_utf8(r.read(ReadOperation::RevParse, &["HEAD"]).await)
        .unwrap()
        .trim()
        .into()
}
async fn kind(r: &Repo) -> Option<OperationKind> {
    operation_state(&r.runner, &r.options)
        .await
        .unwrap()
        .map(|o| o.kind)
}
async fn commit(r: &Repo, file: &str, content: &str, message: &str) {
    r.put(file, content);
    r.git("add", &["--", file]).await;
    r.git("commit", &["-qm", message]).await;
}
/// Runs a Git command expected to stop (conflict); its failure is the point.
async fn stop(r: &Repo, name: &str, args: &[&str]) {
    let result = r
        .runner
        .run(
            WriteCommand::new(name).args(args.iter().copied()),
            &r.options,
        )
        .await;
    assert!(result.is_err(), "{name} {args:?} should stop");
}
async fn status(r: &Repo) -> Vec<u8> {
    r.read(ReadOperation::Status, &["--porcelain=v1"]).await
}
/// main: A=main; topic (from base): A=topic.
async fn diverged(r: &Repo) -> String {
    r.initial().await;
    r.git("branch", &["-M", "main"]).await;
    r.git("checkout", &["-qb", "topic"]).await;
    commit(r, "A", "topic\n", "topic").await;
    r.git("checkout", &["-q", "main"]).await;
    commit(r, "A", "main\n", "main").await;
    head(r).await
}

#[tokio::test]
async fn merge_continue_requires_resolution_and_uses_supplied_message() {
    let r = Repo::new().await;
    let main = diverged(&r).await;
    stop(&r, "merge", &["topic"]).await;
    assert_eq!(kind(&r).await, Some(OperationKind::Merge));
    let error = run(
        &r.runner,
        &r.options,
        &token(&r).await,
        SequenceAction::Continue,
        None,
    )
    .await
    .unwrap_err();
    assert_eq!(error.key, "sequence.unresolved");
    let skip = run(
        &r.runner,
        &r.options,
        &token(&r).await,
        SequenceAction::Skip,
        None,
    )
    .await
    .unwrap_err();
    assert_eq!(skip.key, "sequence.unsupported");
    let stale = token(&r).await;
    commit_free_resolve(&r).await;
    let concurrent = run(
        &r.runner,
        &r.options,
        &stale,
        SequenceAction::Continue,
        None,
    )
    .await
    .unwrap_err();
    assert_eq!(concurrent.key, "remote.failed");
    assert!(concurrent.detail.contains("remote.concurrentChange"));
    let outcome = run(
        &r.runner,
        &r.options,
        &token(&r).await,
        SequenceAction::Continue,
        Some("Merge topic\n\nresolved by test\n"),
    )
    .await
    .unwrap();
    assert!(matches!(outcome, SequenceOutcome::Completed), "{outcome:?}");
    assert_eq!(kind(&r).await, None);
    let parents = r.read(ReadOperation::Log, &["-1", "--format=%P %B"]).await;
    let text = String::from_utf8(parents).unwrap();
    assert!(text.starts_with(&main), "{text}");
    assert!(text.contains("resolved by test"), "{text}");
    let topic = String::from_utf8(r.read(ReadOperation::RevParse, &["topic"]).await).unwrap();
    assert_eq!(text.split_whitespace().nth(1), Some(topic.trim()));
    assert!(status(&r).await.is_empty());
}
async fn commit_free_resolve(r: &Repo) {
    r.put("A", "resolved\n");
    r.git("add", &["--", "A"]).await;
}

#[tokio::test]
async fn rebase_stops_on_each_conflict_and_abort_restores_branch() {
    let r = Repo::new().await;
    diverged(&r).await;
    r.git("checkout", &["-q", "topic"]).await;
    commit(&r, "A", "topic two\n", "topic two").await;
    let original = head(&r).await;
    stop(&r, "rebase", &["main"]).await;
    let state = operation_state(&r.runner, &r.options)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(state.kind, OperationKind::Rebase);
    assert_eq!(state.branch.as_deref(), Some("topic"));
    assert_eq!((state.step, state.total), (Some(1), Some(2)));
    let unresolved = run(
        &r.runner,
        &r.options,
        &token(&r).await,
        SequenceAction::Continue,
        None,
    )
    .await
    .unwrap_err();
    assert_eq!(unresolved.key, "sequence.unresolved");
    commit_free_resolve(&r).await;
    let rejected = run(
        &r.runner,
        &r.options,
        &token(&r).await,
        SequenceAction::Continue,
        Some("x"),
    )
    .await
    .unwrap_err();
    assert_eq!(rejected.key, "sequence.messageUnsupported");
    match run(
        &r.runner,
        &r.options,
        &token(&r).await,
        SequenceAction::Continue,
        None,
    )
    .await
    .unwrap()
    {
        SequenceOutcome::Conflicts { operation, files } => {
            assert_eq!(operation.step, Some(2));
            assert_eq!(files.len(), 1);
            assert_eq!(files[0].display, "A");
        }
        other => panic!("{other:?}"),
    }
    let outcome = run(
        &r.runner,
        &r.options,
        &token(&r).await,
        SequenceAction::Abort,
        None,
    )
    .await
    .unwrap();
    assert!(matches!(outcome, SequenceOutcome::Completed));
    assert_eq!(kind(&r).await, None);
    assert_eq!(head(&r).await, original);
    assert_eq!(std::fs::read(r.path("A")).unwrap(), b"topic two\n");
    assert!(status(&r).await.is_empty());
}

#[tokio::test]
async fn cherry_pick_empty_resolution_stops_then_skip_completes() {
    let r = Repo::new().await;
    let main = diverged(&r).await;
    let topic = String::from_utf8(r.read(ReadOperation::RevParse, &["topic"]).await).unwrap();
    stop(&r, "cherry-pick", &[topic.trim()]).await;
    assert_eq!(kind(&r).await, Some(OperationKind::CherryPick));
    // Resolving to the current content leaves nothing to commit.
    r.put("A", "main\n");
    r.git("add", &["--", "A"]).await;
    match run(
        &r.runner,
        &r.options,
        &token(&r).await,
        SequenceAction::Continue,
        None,
    )
    .await
    .unwrap()
    {
        SequenceOutcome::Stopped { operation, detail } => {
            assert_eq!(operation.kind, OperationKind::CherryPick);
            assert!(!detail.is_empty());
        }
        other => panic!("{other:?}"),
    }
    let outcome = run(
        &r.runner,
        &r.options,
        &token(&r).await,
        SequenceAction::Skip,
        None,
    )
    .await
    .unwrap();
    assert!(matches!(outcome, SequenceOutcome::Completed), "{outcome:?}");
    assert_eq!(head(&r).await, main);
    assert_eq!(kind(&r).await, None);
}

#[tokio::test]
async fn revert_and_bisect_abort_and_idle_repository_is_rejected() {
    let r = Repo::new().await;
    r.initial().await;
    let none = run(
        &r.runner,
        &r.options,
        &token(&r).await,
        SequenceAction::Abort,
        None,
    )
    .await
    .unwrap_err();
    assert_eq!(none.key, "sequence.none");
    commit(&r, "A", "one\n", "one").await;
    let one = head(&r).await;
    commit(&r, "A", "two\n", "two").await;
    let two = head(&r).await;
    stop(&r, "revert", &["--no-edit", &one]).await;
    assert_eq!(kind(&r).await, Some(OperationKind::Revert));
    let outcome = run(
        &r.runner,
        &r.options,
        &token(&r).await,
        SequenceAction::Abort,
        None,
    )
    .await
    .unwrap();
    assert!(matches!(outcome, SequenceOutcome::Completed));
    assert_eq!(head(&r).await, two);
    assert!(status(&r).await.is_empty());
    r.git("bisect", &["start", &two, &one]).await;
    assert_eq!(kind(&r).await, Some(OperationKind::Bisect));
    let skip = run(
        &r.runner,
        &r.options,
        &token(&r).await,
        SequenceAction::Skip,
        None,
    )
    .await
    .unwrap_err();
    assert_eq!(skip.key, "sequence.unsupported");
    let outcome = run(
        &r.runner,
        &r.options,
        &token(&r).await,
        SequenceAction::Abort,
        None,
    )
    .await
    .unwrap();
    assert!(matches!(outcome, SequenceOutcome::Completed));
    assert_eq!(kind(&r).await, None);
}

#[tokio::test]
async fn status_summary_reports_operation_and_selection_resolves_row_ranges() {
    use gitwood_core::{history::details::Filter, repo::session::Sessions};
    let r = Repo::new().await;
    diverged(&r).await;
    let sessions = Sessions::default();
    let l = sessions
        .open(r.runner.clone(), r.options.clone())
        .await
        .unwrap();
    let idle = l.status().await.unwrap();
    assert!(idle.operation.is_none());
    assert_eq!(idle.conflicts, 0);
    stop(&r, "merge", &["topic"]).await;
    let merging = l.status().await.unwrap();
    assert_eq!(merging.operation.unwrap().kind, OperationKind::Merge);
    assert_eq!(merging.conflicts, 1);
    // `git status` agrees that a merge is in progress.
    let long = r.read(ReadOperation::Status, &[]).await;
    assert!(String::from_utf8_lossy(&long).contains("unmerged paths"));
    let total = l.layout(&Filter::default()).await.unwrap().total as u32;
    assert_eq!(total, 3);
    let all = l.hashes(&[(0, 1), (1, total)]).await.unwrap();
    let rows = l.rows(0, total as usize).await.unwrap();
    let shown: Vec<_> = rows.commits.iter().map(|c| c.hash.clone()).collect();
    assert_eq!(all, shown);
    assert!(l.hashes(&[(0, total + 1)]).await.is_err());
    assert!(l.hashes(&[(2, 1)]).await.is_err());
}

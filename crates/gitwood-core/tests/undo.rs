mod support;
use gitwood_core::{
    git::ReadOperation,
    ops::{undo_commit, CommitError},
};
use support::Repo;
async fn head(r: &Repo) -> String {
    String::from_utf8(r.read(ReadOperation::RevParse, &["HEAD"]).await)
        .unwrap()
        .trim()
        .into()
}
async fn second(r: &Repo) -> String {
    r.put("A", "committed\n");
    r.git("add", &["A"]).await;
    r.git(
        "commit",
        &["-qm", "subject\n\ncomplete body\n\ntrailer: value"],
    )
    .await;
    head(r).await
}
#[tokio::test]
async fn soft_reset_preserves_index_worktree_and_complete_message() {
    let r = Repo::new().await;
    r.initial().await;
    let parent = head(&r).await;
    let selected = second(&r).await;
    r.put("B", "staged\n");
    r.git("add", &["B"]).await;
    r.put("B", "unstaged\n");
    let index = std::fs::read(r.path(".git/index")).unwrap();
    let unstaged = r.read(ReadOperation::Diff, &["--binary"]).await;
    let message = undo_commit(&r.runner, &r.options, &selected).await.unwrap();
    assert_eq!(message, "subject\n\ncomplete body\n\ntrailer: value\n");
    assert_eq!(head(&r).await, parent);
    assert_eq!(std::fs::read(r.path(".git/index")).unwrap(), index);
    assert_eq!(r.read(ReadOperation::Diff, &["--binary"]).await, unstaged);
    assert_eq!(r.read(ReadOperation::Show, &[":A"]).await, b"committed\n");
    assert_eq!(std::fs::read(r.path("A")).unwrap(), b"committed\n");
    assert_eq!(std::fs::read(r.path("B")).unwrap(), b"unstaged\n");
    let staged = r
        .read(ReadOperation::Diff, &["--cached", "--name-only"])
        .await;
    assert_eq!(staged, b"A\nB\n");
    r.clean_temporary_files();
}
#[tokio::test]
async fn stale_root_merge_lock_and_active_operations_are_rejected_without_changes() {
    let r = Repo::new().await;
    r.initial().await;
    let root = head(&r).await;
    let before = r.state().await;
    assert!(undo_commit(&r.runner, &r.options, &root).await.is_err());
    assert_eq!(r.state().await, before);
    let selected = second(&r).await;
    let before = r.state().await;
    assert!(matches!(
        undo_commit(&r.runner, &r.options, &root).await,
        Err(CommitError::ConcurrentChange)
    ));
    assert_eq!(r.state().await, before);
    std::fs::write(r.path(".git/index.lock"), b"external lock").unwrap();
    assert!(matches!(
        undo_commit(&r.runner, &r.options, &selected).await,
        Err(CommitError::IndexLocked(_))
    ));
    assert_eq!(
        std::fs::read(r.path(".git/index.lock")).unwrap(),
        b"external lock"
    );
    assert_eq!(r.state().await, before);
    std::fs::remove_file(r.path(".git/index.lock")).unwrap();
    for marker in [
        "MERGE_HEAD",
        "CHERRY_PICK_HEAD",
        "REVERT_HEAD",
        "rebase-merge",
        "rebase-apply",
        "sequencer",
    ] {
        std::fs::write(r.path(&format!(".git/{marker}")), &selected).unwrap();
        assert!(undo_commit(&r.runner, &r.options, &selected).await.is_err());
        std::fs::remove_file(r.path(&format!(".git/{marker}"))).unwrap();
        assert_eq!(r.state().await, before);
    }
    r.git("checkout", &["-qb", "side", &root]).await;
    r.put("C", "side\n");
    r.git("add", &["C"]).await;
    r.git("commit", &["-qm", "side"]).await;
    r.git("merge", &["--no-ff", "-m", "merge", &selected]).await;
    let merge = head(&r).await;
    let before = r.state().await;
    assert!(undo_commit(&r.runner, &r.options, &merge).await.is_err());
    assert_eq!(r.state().await, before);
}
#[tokio::test]
async fn protected_remote_branches_are_checked_without_the_ui_page_limit() {
    let r = Repo::new().await;
    r.initial().await;
    let selected = second(&r).await;
    // Put protected refs beyond the 200-name UI page.
    let mut options = r.options.clone();
    options.stdin = (0..205)
        .map(|i| format!("update refs/heads/topic-{i:03} {selected}\n"))
        .collect::<String>()
        .into_bytes();
    r.runner
        .run(
            gitwood_core::git::WriteCommand::new("update-ref").args(["--stdin"]),
            &options,
        )
        .await
        .unwrap();
    for name in ["refs/remotes/origin/main", "refs/remotes/upstream/master"] {
        r.git("update-ref", &[name, &selected]).await;
        let before = r.state().await;
        assert!(undo_commit(&r.runner, &r.options, &selected).await.is_err());
        assert_eq!(r.state().await, before);
        r.git("update-ref", &["-d", name]).await;
    }
    r.git("update-ref", &["refs/remotes/origin/topic/main", &selected])
        .await;
    undo_commit(&r.runner, &r.options, &selected).await.unwrap();
}

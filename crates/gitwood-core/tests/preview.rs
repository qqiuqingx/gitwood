mod support;
use gitwood_core::{
    diff::Content,
    git::ReadOperation,
    repo::{preview::Area, session::Sessions, RepoPath},
};
use support::Repo;
fn text(c: Content) -> String {
    match c {
        Content::Text { text, .. } => text,
        _ => panic!("expected text"),
    }
}
#[tokio::test]
async fn preview_sources_and_contains() {
    let r = Repo::new().await;
    r.initial().await;
    let hash = String::from_utf8(r.read(ReadOperation::RevParse, &["HEAD"]).await)
        .unwrap()
        .trim()
        .to_owned();
    r.put("A", "staged\n");
    r.git("add", &["A"]).await;
    r.put("A", "working\n");
    r.put("new", "new\n");
    let sessions = Sessions::default();
    let l = sessions
        .open(r.runner.clone(), r.options.clone())
        .await
        .unwrap();
    let path = RepoPath::new(b"A");
    let p = l.worktree_diff(&path, Area::Staged).await.unwrap();
    assert_eq!(text(p.left), "A base\n");
    assert_eq!(text(p.right), "staged\n");
    let p = l.worktree_diff(&path, Area::Unstaged).await.unwrap();
    assert_eq!(text(p.left), "staged\n");
    assert_eq!(text(p.right), "working\n");
    let p = l
        .worktree_diff(&RepoPath::new(b"new"), Area::Untracked)
        .await
        .unwrap();
    assert_eq!(text(p.left), "");
    let p = l.commit_diff(&hash, &path).await.unwrap();
    assert_eq!(text(p.left), "");
    assert_eq!(text(p.right), "A base\n");
    assert!(l.containing_branches(&hash).await.unwrap().total > 0);
    assert!(l.containing_branches("--all").await.is_err());
    std::fs::remove_file(r.path("A")).unwrap();
    assert_eq!(
        text(l.worktree_diff(&path, Area::Unstaged).await.unwrap().right),
        ""
    );
    std::fs::write(r.path("new"), b"\0binary").unwrap();
    assert!(matches!(
        l.worktree_diff(&RepoPath::new(b"new"), Area::Untracked)
            .await
            .unwrap()
            .right,
        Content::Binary { .. }
    ));
}

#[tokio::test]
async fn rename_normal_merge_large_and_truncation() {
    let r = Repo::new().await;
    r.initial().await;
    r.git("branch", &["side"]).await;
    let main = String::from_utf8(r.git("branch", &["--show-current"]).await)
        .unwrap()
        .trim()
        .to_owned();
    r.git("mv", &["A", "renamed"]).await;
    let sessions = Sessions::default();
    let l = sessions
        .open(r.runner.clone(), r.options.clone())
        .await
        .unwrap();
    let path = RepoPath::new(b"renamed");
    let p = l.worktree_diff(&path, Area::Staged).await.unwrap();
    assert_eq!(text(p.left), "A base\n");
    r.git("commit", &["-qm", "rename"]).await;
    let hash = String::from_utf8(r.read(ReadOperation::RevParse, &["HEAD"]).await)
        .unwrap()
        .trim()
        .to_owned();
    assert_eq!(
        text(l.commit_diff(&hash, &path).await.unwrap().left),
        "A base\n"
    );
    r.git("checkout", &["side"]).await;
    r.put("B", "side\n");
    r.git("add", &["B"]).await;
    r.git("commit", &["-qm", "side"]).await;
    r.git("checkout", &[&main]).await;
    r.git("merge", &["--no-ff", "-m", "merge", "side"]).await;
    let hash = String::from_utf8(r.read(ReadOperation::RevParse, &["HEAD"]).await)
        .unwrap()
        .trim()
        .to_owned();
    let p = l.commit_diff(&hash, &RepoPath::new(b"B")).await.unwrap();
    assert_eq!(text(p.left), "B base\n");
    assert_eq!(text(p.right), "side\n");
    let file = std::fs::File::create(r.path("large")).unwrap();
    file.set_len(gitwood_core::diff::DEFAULT_MAX_BYTES + 1)
        .unwrap();
    assert!(matches!(
        l.worktree_diff(&RepoPath::new(b"large"), Area::Untracked)
            .await
            .unwrap()
            .right,
        Content::TooLarge { .. }
    ));
    for i in 0..205 {
        r.git("update-ref", &[&format!("refs/heads/test{i:03}"), &hash])
            .await;
    }
    let branches = l.containing_branches(&hash).await.unwrap();
    assert_eq!(branches.branches.len(), 200);
    assert!(branches.total >= 206);
    sessions.close();
    assert!(sessions.check(&l).is_err());
}

mod support;
use gitwood_core::{
    diff::Content,
    git::ReadOperation,
    repo::{session::Sessions, RepoPath},
};
use support::Repo;
#[tokio::test]
async fn comparison_pins_revisions_pages_files_and_previews_renames_additions_deletions() {
    let r = Repo::new().await;
    r.initial().await;
    r.git("branch", &["-M", "main"]).await;
    r.git("branch", &["selected"]).await;
    r.git("mv", &["A", "renamed"]).await;
    r.put("new", "new\n");
    r.git("rm", &["B"]).await;
    r.git("add", &["new"]).await;
    r.git("commit", &["-m", "current"]).await;
    let sessions = Sessions::default();
    let lease = sessions
        .open(r.runner.clone(), r.options.clone())
        .await
        .unwrap();
    let c = lease
        .comparison_open("refs/heads/selected", false)
        .await
        .unwrap();
    assert_eq!(c.commits, 1);
    let page = lease.comparison_commits(&c.id, 0, 100).await.unwrap();
    assert_eq!(page.commits[0].subject, "current");
    assert_eq!(page.current, [true]);
    let files = lease
        .comparison_files(
            &c.id,
            &std::collections::HashSet::from(["files".into()]),
            0,
            100,
        )
        .await
        .unwrap();
    assert_eq!(files.files, 3);
    for name in ["renamed", "new", "B"] {
        let value = lease
            .comparison_diff(&c.id, &RepoPath::new(name.as_bytes()))
            .await
            .unwrap();
        assert!(matches!(value.left, Content::Text { .. }));
        assert!(value.result.is_some());
        if name == "renamed" {
            assert!(matches!(value.left, Content::Text { ref text, .. } if text == "A base\n"));
        }
        if name == "new" {
            assert!(matches!(value.left, Content::Text { empty: true, .. }));
        }
        if name == "B" {
            assert!(matches!(value.right, Content::Text { empty: true, .. }));
        }
    }
    r.git("branch", &["-f", "selected", "HEAD"]).await;
    assert!(lease
        .comparison_diff(&c.id, &RepoPath::new(b"renamed"))
        .await
        .is_ok());
    assert!(lease
        .comparison_diff(&c.id, &RepoPath::new(b"../bad"))
        .await
        .is_err());
    assert!(lease
        .comparison_files(&c.id, &Default::default(), 0, 201)
        .await
        .is_err());
    assert!(lease.comparison_commits(&c.id, 0, 201).await.is_err());
    assert!(lease.comparison_open("--output=bad", false).await.is_err());
}
#[tokio::test]
async fn worktree_comparison_detects_concurrent_edits_and_preserves_binary() {
    let r = Repo::new().await;
    r.initial().await;
    r.put("A", "local\n");
    std::fs::write(r.path("B"), b"binary\0bytes").unwrap();
    let sessions = Sessions::default();
    let lease = sessions
        .open(r.runner.clone(), r.options.clone())
        .await
        .unwrap();
    let c = lease.comparison_open("HEAD", true).await.unwrap();
    let a = lease
        .comparison_diff(&c.id, &RepoPath::new(b"A"))
        .await
        .unwrap();
    assert!(matches!(a.right, Content::Text { text, .. } if text == "local\n"));
    let b = lease
        .comparison_diff(&c.id, &RepoPath::new(b"B"))
        .await
        .unwrap();
    assert!(matches!(b.right, Content::Binary { .. }));
    assert!(b.result.is_none());
    r.put("A", "external edit\n");
    assert!(lease
        .comparison_diff(&c.id, &RepoPath::new(b"A"))
        .await
        .is_err());
    assert_eq!(
        std::fs::read_to_string(r.path("A")).unwrap(),
        "external edit\n"
    );
}
#[tokio::test]
async fn comparison_commit_sides_and_cache_are_scoped_to_session() {
    let r = Repo::new().await;
    r.initial().await;
    let original = String::from_utf8(r.read(ReadOperation::RevParse, &["HEAD"]).await).unwrap();
    r.git("checkout", &["-b", "selected"]).await;
    r.put("A", "selected\n");
    r.git("commit", &["-am", "selected"]).await;
    r.git("checkout", &["-b", "current", original.trim()]).await;
    r.put("B", "current\n");
    r.git("commit", &["-am", "current"]).await;
    let sessions = Sessions::default();
    let lease = sessions
        .open(r.runner.clone(), r.options.clone())
        .await
        .unwrap();
    let c = lease
        .comparison_open("refs/heads/selected", false)
        .await
        .unwrap();
    let page = lease.comparison_commits(&c.id, 0, 200).await.unwrap();
    assert_eq!(page.commits.len(), 2);
    assert!(page.current.contains(&true) && page.current.contains(&false));
    let other = Repo::new().await;
    other.initial().await;
    let other = sessions.open(other.runner, other.options).await.unwrap();
    assert!(lease.comparison_commits(&c.id, 0, 200).await.is_err());
    assert!(other.comparison_commits(&c.id, 0, 200).await.is_err());
}

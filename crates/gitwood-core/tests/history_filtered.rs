mod support;
use gitwood_core::{
    git::ReadOperation,
    history::{
        details::{self, Filter},
        CommitId, EdgeId, GraphLayout,
    },
    repo::RepoPath,
};
use support::Repo;

async fn commit(repo: &Repo, file: &str, message: &str, author: &str) -> String {
    repo.put(file, message);
    repo.git("add", &["--", file]).await;
    repo.git("commit", &["-qm", message, "--author", author])
        .await;
    String::from_utf8(repo.read(ReadOperation::RevParse, &["HEAD"]).await)
        .unwrap()
        .trim()
        .into()
}
fn edges(layout: &GraphLayout) -> Vec<(String, String, bool)> {
    (0..layout.graph().edge_count())
        .map(|index| {
            let id = EdgeId(index as u32);
            let edge = layout.graph().edge(id).unwrap();
            (
                layout.graph().hash(edge.child).unwrap(),
                layout.graph().hash(edge.parent).unwrap(),
                layout.edge_is_dotted(id),
            )
        })
        .collect()
}
async fn filtered(repo: &Repo, filter: Filter) -> GraphLayout {
    details::layout(&repo.runner, &repo.options, &filter)
        .await
        .unwrap()
}
#[tokio::test]
async fn alternating_linear_matches_author_text_empty_and_all() {
    let repo = Repo::new().await;
    let mut matching = vec![];
    for index in 0..7 {
        let hit = index % 2 == 0;
        let hash = commit(
            &repo,
            "A",
            &format!("{} {index}", if hit { "hit" } else { "skip" }),
            if hit {
                "Match <match@example.invalid>"
            } else {
                "Other <other@example.invalid>"
            },
        )
        .await;
        if hit {
            matching.push(hash);
        }
    }
    matching.reverse();
    for filter in [
        Filter {
            text: Some("hit".into()),
            ..Filter::default()
        },
        Filter {
            author: Some("Match".into()),
            ..Filter::default()
        },
    ] {
        let layout = filtered(&repo, filter).await;
        assert_eq!(layout.graph().len(), 4);
        assert_eq!(
            edges(&layout),
            matching
                .windows(2)
                .map(|pair| (pair[0].clone(), pair[1].clone(), true))
                .collect::<Vec<_>>()
        );
    }
    assert!(filtered(
        &repo,
        Filter {
            text: Some("absent".into()),
            ..Filter::default()
        }
    )
    .await
    .graph()
    .is_empty());
    let all = filtered(&repo, Filter::default()).await;
    assert_eq!(all.graph().len(), 7);
    assert_eq!(edges(&all).len(), 6);
    assert!(edges(&all).iter().all(|edge| !edge.2));
}
#[tokio::test]
async fn fork_merge_nearest_ancestors_deduplicate_and_stop_at_matches() {
    let repo = Repo::new().await;
    let root = commit(&repo, "root", "hit root", "Match <m@example.invalid>").await;
    let main = String::from_utf8(repo.git("branch", &["--show-current"]).await).unwrap();
    repo.git("checkout", &["-qb", "side"]).await;
    let side = commit(&repo, "side", "hit side", "Match <m@example.invalid>").await;
    commit(&repo, "side", "skip side", "Other <o@example.invalid>").await;
    repo.git("checkout", &["-q", main.trim()]).await;
    commit(&repo, "main", "skip main", "Other <o@example.invalid>").await;
    repo.git("merge", &["--no-ff", "side", "-m", "hit merge"])
        .await;
    let merge = String::from_utf8(repo.read(ReadOperation::RevParse, &["HEAD"]).await)
        .unwrap()
        .trim()
        .to_owned();
    let layout = filtered(
        &repo,
        Filter {
            text: Some("hit".into()),
            ..Filter::default()
        },
    )
    .await;
    assert_eq!(layout.graph().len(), 3);
    let result = edges(&layout);
    assert!(result.contains(&(merge.clone(), side.clone(), true)));
    assert!(result.contains(&(merge, root.clone(), true)));
    assert!(result.contains(&(side, root, false)));
    for edge in
        (0..layout.graph().edge_count()).filter_map(|id| layout.graph().edge(EdgeId(id as u32)))
    {
        assert!(edge.child < edge.parent);
    }
    // Both invisible arms converge on one matching ancestor.
    let layout = filtered(
        &repo,
        Filter {
            text: Some("hit root\nhit merge".into()),
            ..Filter::default()
        },
    )
    .await;
    assert_eq!(layout.graph().len(), 2);
    assert_eq!(layout.graph().edge_count(), 1);
}
#[tokio::test]
async fn path_parent_rewriting_and_combined_visibility_keep_original_connectivity() {
    let repo = Repo::new().await;
    let root = commit(&repo, "A", "hit root", "Match <m@example.invalid>").await;
    commit(&repo, "B", "skip other path", "Other <o@example.invalid>").await;
    commit(&repo, "A", "skip same path", "Other <o@example.invalid>").await;
    let tip = commit(&repo, "A", "hit tip", "Match <m@example.invalid>").await;
    let path = Filter {
        path: Some(RepoPath::new(b"A")),
        ..Filter::default()
    };
    let rewritten = repo
        .read(
            ReadOperation::Log,
            &[
                "--format=%H %P",
                "--parents",
                "--full-history",
                "--topo-order",
                "--all",
                "--",
                "A",
            ],
        )
        .await;
    let lines = String::from_utf8(rewritten).unwrap();
    assert_eq!(lines.lines().count(), 3);
    // Git rewrites across B; %P without --parents would point outside the path graph.
    assert!(lines.lines().nth(1).unwrap().ends_with(&root));
    let path_only = filtered(&repo, path.clone()).await;
    assert_eq!(path_only.graph().len(), 3);
    assert_eq!(edges(&path_only).iter().filter(|edge| edge.2).count(), 1);
    for filter in [
        Filter {
            text: Some("hit".into()),
            ..path.clone()
        },
        Filter {
            author: Some("Match".into()),
            ..path
        },
    ] {
        let layout = filtered(&repo, filter).await;
        assert_eq!(edges(&layout), vec![(tip.clone(), root.clone(), true)]);
        assert_eq!(layout.graph().hash(CommitId(0)), Some(tip.clone()));
    }
}

#[tokio::test]
async fn paged_commit_tree_keeps_directory_and_byte_identity() {
    use gitwood_core::history::details::FileChange;
    use gitwood_core::repo::{file_tree::FileTree, session::Sessions};
    use std::collections::HashSet;
    let repo = Repo::new().await;
    std::fs::create_dir(repo.path("src")).unwrap();
    commit(&repo, "src/file", "body", "Test <t@example.invalid>").await;
    let sessions = Sessions::default();
    let lease = sessions
        .open(repo.runner.clone(), repo.options.clone())
        .await
        .unwrap();
    lease.layout(&Filter::default()).await.unwrap();
    let hash = lease.rows(0, 1).await.unwrap().commits[0].hash.clone();
    let mut expanded: HashSet<String> = ["files".into()].into();
    let roots = lease
        .file_tree(&hash, &expanded, 0, 200, &[])
        .await
        .unwrap();
    assert_eq!(roots.total, 2);
    let directory = roots.rows[1].node.id.clone();
    expanded.insert(directory);
    let page = lease.file_tree(&hash, &expanded, 2, 3, &[]).await.unwrap();
    assert_eq!(page.total, 3);
    assert_eq!(page.rows[0].node.path.as_ref().unwrap().bytes, b"src/file");
    assert_eq!(page.rows[0].parent_index, Some(1));
    assert_eq!(lease.position(&hash).await.unwrap(), Some(0));
    assert!(lease
        .file_tree(&hash, &expanded, 0, 201, &[])
        .await
        .is_err());
    let files: Vec<_> = (0..1000)
        .map(|index| FileChange {
            status: "A".into(),
            path: RepoPath::new(format!("file-{index}").as_bytes()),
            original: None,
        })
        .collect();
    let tree = FileTree::new(&files);
    let page = tree.page(&expanded, 400, 440, &[]);
    assert_eq!(page.total, 1001);
    assert_eq!(page.rows.len(), 40);
    let invalid = FileTree::new(&[FileChange {
        status: "R100".into(),
        path: RepoPath::new(b"dir/x\xff\n"),
        original: Some(RepoPath::new(b"old")),
    }]);
    let first = invalid.page(&expanded, 0, 200, &[]);
    expanded.insert(first.rows[1].node.id.clone());
    let second = invalid.page(&expanded, 0, 200, &[]);
    assert_eq!(
        second.rows[2].node.path.as_ref().unwrap().bytes,
        b"dir/x\xff\n"
    );
    assert_eq!(second.rows[2].node.note.as_deref(), Some("old"));
}

#[tokio::test]
async fn changes_tree_merges_staging_and_pages_without_transferring_full_status() {
    use gitwood_core::repo::session::Sessions;
    use std::collections::HashSet;
    let repo = Repo::new().await;
    repo.initial().await;
    repo.put("A", "staged");
    repo.git("add", &["A"]).await;
    repo.put("A", "unstaged");
    repo.put("new", "untracked");
    let sessions = Sessions::default();
    let lease = sessions
        .open(repo.runner.clone(), repo.options.clone())
        .await
        .unwrap();
    let summary = lease.status().await.unwrap();
    assert_eq!((summary.changes, summary.untracked), (1, 1));
    let expanded: HashSet<String> = ["*".into()].into();
    let page = lease
        .changes_tree(&expanded, false, 0, 200, &[])
        .await
        .unwrap();
    let changed: Vec<_> = page
        .rows
        .iter()
        .filter(|row| {
            row.node
                .path
                .as_ref()
                .is_some_and(|path| path.bytes == b"A")
        })
        .collect();
    assert_eq!(changed.len(), 1);
    assert!(changed[0].node.staged && changed[0].node.unstaged);
    let selected_id = changed[0].node.id.clone();
    repo.put("0-before-A", "new staged");
    repo.git("add", &["0-before-A"]).await;
    lease.status().await.unwrap();
    let moved = lease
        .changes_tree(
            &expanded,
            false,
            0,
            200,
            &[selected_id.clone(), selected_id],
        )
        .await
        .unwrap();
    assert_eq!(moved.positions, vec![Some(2), Some(2)]);

    let mut collapsed = expanded;
    collapsed.insert("!changes".into());
    let hidden = lease
        .changes_tree(&collapsed, false, 0, 200, &[])
        .await
        .unwrap();
    assert!(hidden.rows.iter().all(|row| row
        .node
        .path
        .as_ref()
        .is_none_or(|path| path.bytes != b"A")));
    assert!(lease
        .changes_tree(&collapsed, true, 0, 201, &[])
        .await
        .is_err());
}

#[tokio::test]
async fn history_refresh_preserves_independent_status_but_rejects_old_layout() {
    use gitwood_core::repo::session::Sessions;
    use std::collections::HashSet;
    let repo = Repo::new().await;
    repo.initial().await;
    repo.put("A", "changed");
    let sessions = Sessions::default();
    let old = sessions
        .open(repo.runner.clone(), repo.options.clone())
        .await
        .unwrap();
    old.status().await.unwrap();
    old.layout(&Filter::default()).await.unwrap();
    let generation = sessions.refresh(old.generation).unwrap();
    let current = sessions.lease(generation).unwrap();
    let expanded: HashSet<String> = ["*".into()].into();
    assert_eq!(
        current
            .changes_tree(&expanded, false, 0, 200, &[])
            .await
            .unwrap()
            .files,
        1
    );
    assert!(current.rows(0, 1).await.is_err());
    assert!(old.layout(&Filter::default()).await.is_err());
    current.layout(&Filter::default()).await.unwrap();
    assert_eq!(current.rows(0, 1).await.unwrap().commits.len(), 1);
}

#[tokio::test]
async fn detached_head_filter_only_includes_its_ancestors() {
    let repo = Repo::new().await;
    let root = commit(&repo, "A", "root", "Test <t@example.invalid>").await;
    commit(&repo, "A", "branch tip", "Test <t@example.invalid>").await;
    repo.git("checkout", &["--detach", "-q", &root]).await;
    commit(&repo, "B", "detached tip", "Test <t@example.invalid>").await;
    let layout = filtered(
        &repo,
        Filter {
            branch: Some("HEAD".into()),
            ..Filter::default()
        },
    )
    .await;
    assert_eq!(layout.graph().len(), 2);
    assert_eq!(filtered(&repo, Filter::default()).await.graph().len(), 3);
}
#[tokio::test]
async fn text_filter_honors_regex_and_match_case_toggles() {
    let repo = Repo::new().await;
    let author = "A <a@example.invalid>";
    let upper = commit(&repo, "A", "Fix Bug 1", author).await;
    let lower = commit(&repo, "A", "fix bug 2", author).await;
    commit(&repo, "A", "feature 3", author).await;
    let count = |text: &str, regex: bool, match_case: bool| {
        let filter = Filter {
            text: Some(text.into()),
            regex,
            match_case,
            ..Filter::default()
        };
        let repo = &repo;
        async move { filtered(repo, filter).await.graph().len() }
    };
    assert_eq!(count("fix", false, false).await, 2);
    assert_eq!(count("fix", false, true).await, 1);
    // Literal mode keeps regex metacharacters literal.
    assert_eq!(count("bug [12]", false, false).await, 0);
    assert_eq!(count("bug [12]", true, false).await, 2);
    assert_eq!(count("^Fix", true, true).await, 1);
    assert_eq!(count("^fix", true, false).await, 2);
    let prefix = upper[..10].to_ascii_uppercase();
    assert_eq!(count(&prefix, false, false).await, 1);
    assert_eq!(count(&prefix, false, true).await, 0);
    assert_eq!(count(&lower[..10], false, true).await, 1);
}
#[tokio::test]
async fn current_branch_rows_follow_idea_highlighter_rules() {
    let repo = Repo::new().await;
    let author = "A <a@example.invalid>";
    let base = commit(&repo, "A", "base", author).await;
    let head_name = String::from_utf8(
        repo.read(ReadOperation::RevParse, &["--abbrev-ref", "HEAD"])
            .await,
    )
    .unwrap()
    .trim()
    .to_owned();
    repo.git("checkout", &["-qb", "side"]).await;
    let side = commit(&repo, "B", "side", author).await;
    repo.git("checkout", &["-q", &head_name]).await;
    let main = commit(&repo, "A", "main", author).await;
    let rows = |filter: Filter| {
        let repo = &repo;
        async move {
            let layout = filtered(repo, filter.clone()).await;
            let current =
                details::current_branch_rows(&repo.runner, &repo.options, &filter, &layout)
                    .await
                    .unwrap();
            (0..current.len())
                .map(|row| {
                    (
                        layout.graph().hash(CommitId(row as u32)).unwrap(),
                        current[row],
                    )
                })
                .collect::<std::collections::HashMap<_, _>>()
        }
    };
    let all = rows(Filter::default()).await;
    assert_eq!(all.len(), 3);
    assert!(all[&base] && all[&main] && !all[&side]);
    let other = rows(Filter {
        branch: Some("refs/heads/side".into()),
        ..Filter::default()
    })
    .await;
    assert!(other[&base] && !other[&side]);
    for branch in ["HEAD".to_owned(), format!("refs/heads/{head_name}")] {
        let only_current = rows(Filter {
            branch: Some(branch),
            ..Filter::default()
        })
        .await;
        assert!(only_current.is_empty());
    }
    repo.git("checkout", &["-q", "--detach"]).await;
    assert!(rows(Filter::default()).await.is_empty());
}

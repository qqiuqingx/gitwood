mod support;

use gitwood_core::{
    history::details::FileChange,
    repo::{
        file_tree::FileTree,
        inclusion::{self, Inclusion, Rule, Selection},
        status, RepoPath,
    },
};
use std::collections::HashSet;
use support::Repo;

#[test]
fn compact_paths_stop_at_branches_and_files_and_keep_paging_metadata() {
    let files: Vec<_> = [
        "pom.xml",
        "src/main/java/cn/migu/music/notify/entity/Order.java",
        "src/main/java/cn/migu/music/notify/service/impl/Notify.java",
        "src/test/java/NotifyTest.java",
    ]
    .into_iter()
    .map(|name| FileChange {
        status: "M".into(),
        path: RepoPath::new(name.as_bytes()),
        original: None,
    })
    .collect();
    let tree = FileTree::new(&files);
    let expanded = HashSet::from(["*".into()]);
    let all = tree.page(&expanded, 0, 200, &[]);
    assert_eq!(all.files, 4);
    assert_eq!(
        all.rows
            .iter()
            .map(|row| row.node.label.as_str())
            .collect::<Vec<_>>(),
        [
            "",
            "pom.xml",
            "src",
            "main/java/cn/migu/music/notify",
            "entity",
            "Order.java",
            "service/impl",
            "Notify.java",
            "test/java",
            "NotifyTest.java",
        ]
    );
    let directory = &all.rows[3];
    assert_eq!(directory.node.count, 2);
    assert_eq!(all.rows[2].node.count, 3);
    assert_eq!(all.rows[6].node.count, 1);
    assert_eq!(directory.level, 3);
    assert_eq!(directory.parent_index, Some(2));
    let located = tree.page(&expanded, 6, 8, &[all.rows[7].node.id.clone()]);
    assert_eq!(located.total, 10);
    assert_eq!(located.rows.len(), 2);
    assert_eq!(located.positions, [Some(7)]);
    assert_eq!(located.rows[0].parent_index, Some(3));
    assert_eq!(located.rows[0].level, 4);
    assert_eq!(located.rows[0].pos_in_set, 2);
    assert_eq!(located.rows[0].set_size, 2);
    let mut collapsed = expanded.clone();
    collapsed.insert(format!("!{}", directory.node.id));
    let page = tree.page(&collapsed, 0, 200, &[]);
    assert_eq!(page.total, 6);
    assert_eq!(page.rows[3].expanded, Some(false));
    assert_eq!(page.rows[4].node.label, "test/java");

    let with_parent_file = FileTree::new(&[
        files[1].clone(),
        FileChange {
            path: RepoPath::new(b"src/main/README.md"),
            ..files[0].clone()
        },
    ]);
    let page = with_parent_file.page(&expanded, 0, 200, &[]);
    assert_eq!(page.rows[1].node.label, "src/main");
    assert_eq!(page.rows[1].node.count, 2);
    assert_eq!(page.rows[2].node.label, "java/cn/migu/music/notify/entity");
}

#[test]
fn compact_paths_keep_undecodable_directory_ids_and_original_file_paths() {
    let files: Vec<_> = [b"src/\xff/deep/file".as_slice(), b"src/\xfe/deep/file"]
        .into_iter()
        .map(|bytes| FileChange {
            status: "R100".into(),
            path: RepoPath::new(bytes),
            original: Some(RepoPath::new(b"old/file")),
        })
        .collect();
    let tree = FileTree::new(&files);
    let page = tree.page(&HashSet::from(["*".into()]), 0, 200, &[]);
    assert_eq!(page.total, 6);
    assert_eq!(page.rows[2].node.label, page.rows[4].node.label);
    assert_ne!(page.rows[2].node.id, page.rows[4].node.id);
    assert_eq!(page.rows[3].node.path.as_ref(), Some(&files[0].path));
    assert_eq!(page.rows[5].node.path.as_ref(), Some(&files[1].path));
    assert_eq!(page.rows[3].node.note.as_deref(), Some("old/file"));
}

#[tokio::test]
async fn compact_changes_keep_directory_inclusion_flat_view_and_group_boundaries() {
    let repo = Repo::new().await;
    std::fs::create_dir_all(repo.path("src/main/java/pkg")).unwrap();
    let path = "src/main/java/pkg/Order.java";
    repo.put(path, "original\n");
    repo.git("add", &[path]).await;
    repo.git("commit", &["-qm", "base"]).await;
    repo.put(path, "changed\n");
    repo.git("add", &[path]).await;
    repo.put(path, "changed again\n");
    repo.put("src/main/java/pkg/New.java", "untracked\n");
    let state = status::read(&repo.runner, &repo.options).await.unwrap();
    let expanded = HashSet::from(["*".into()]);
    let tree = FileTree::changes(&state, false);
    let page = tree.page(&expanded, 0, 200, &[]);
    assert_eq!(page.total, 6);
    assert_eq!(page.files, 2);
    assert_eq!(page.rows[0].node.id, "changes");
    assert_eq!(page.rows[3].node.id, "unversioned");
    for index in [1, 4] {
        assert_eq!(page.rows[index].node.label, "src/main/java/pkg");
        assert_eq!(page.rows[index].node.count, 1);
    }
    assert!(page.rows[2].node.staged && page.rows[2].node.unstaged);
    let selection = Selection {
        rules: vec![Rule {
            id: page.rows[1].node.id.clone(),
            included: true,
        }],
        ..Selection::default()
    };
    let entries = inclusion::entries(&state);
    assert_eq!(selection.node(&page.rows[1].node, &entries), Inclusion::All);
    assert_eq!(selection.node(&page.rows[2].node, &entries), Inclusion::All);
    assert_eq!(
        selection.node(&page.rows[4].node, &entries),
        Inclusion::None
    );
    assert!(!inclusion::matches(
        &page.rows[1].node.id,
        &RepoPath::new(b"src/main/java/pkg-other/Other.java"),
        false
    ));
    let flat = FileTree::changes(&state, true).page(&expanded, 0, 200, &[]);
    assert_eq!(flat.total, 4);
    assert_eq!(flat.rows[1].node.label, path);
    assert_eq!(flat.rows[1].node.id, page.rows[2].node.id);
    assert_eq!(flat.rows[3].node.id, page.rows[5].node.id);
    assert_eq!(FileTree::new(&[]).page(&expanded, 0, 200, &[]).total, 1);
}

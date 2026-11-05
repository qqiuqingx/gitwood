mod support;
use gitwood_core::{
    git::ReadOperation,
    ops::{
        changes, snapshot,
        stash::{self, StashAction, StashEntry, StashOutcome},
    },
    repo::{self, RepoPath},
};
use support::Repo;

async fn token(repo: &Repo) -> String {
    snapshot(&repo.runner, &repo.options)
        .await
        .unwrap()
        .fingerprint
}
async fn entries(repo: &Repo) -> Vec<StashEntry> {
    stash::page(&repo.runner, &repo.options, 0, 200)
        .await
        .unwrap()
        .0
}
async fn create(repo: &Repo, message: &str, keep_index: bool, include_untracked: bool) {
    stash::create(
        &repo.runner,
        &repo.options,
        &token(repo).await,
        message,
        keep_index,
        include_untracked,
    )
    .await
    .unwrap();
}

#[test]
fn stash_ipc_action_fields_use_the_frontend_camel_case_contract() {
    let payload = serde_json::json!({
        "action": "apply",
        "entry": { "selector": "stash@{0}", "oid": "abcd1234", "subject": "note" },
        "reinstateIndex": true
    });
    assert!(matches!(
        serde_json::from_value::<StashAction>(payload).unwrap(),
        StashAction::Apply {
            reinstate_index: true,
            ..
        }
    ));
    let clear = serde_json::json!({ "action": "clear", "listToken": "list-token" });
    assert!(matches!(
        serde_json::from_value::<StashAction>(clear).unwrap(),
        StashAction::Clear { list_token } if list_token == "list-token"
    ));
}

#[tokio::test]
async fn stash_message_keep_index_and_untracked_match_git_reflog_and_show() {
    let repo = Repo::new().await;
    repo.initial().await;
    repo.put("A", "staged A\n");
    repo.git("add", &["--", "A"]).await;
    repo.put("B", "unstaged B\n");
    repo.put("untracked", "untracked content\n");
    create(&repo, "M4 stash note", true, true).await;
    let listed = entries(&repo).await;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].selector, "stash@{0}");
    assert!(listed[0].subject.contains("M4 stash note"));
    assert_eq!(
        repo.git("stash", &["list", "--format=%gd%x00%H%x00%gs"])
            .await
            .strip_suffix(b"\n")
            .unwrap(),
        format!(
            "{}\0{}\0{}",
            listed[0].selector, listed[0].oid, listed[0].subject
        )
        .as_bytes()
    );
    let status = repo::status::read(&repo.runner, &repo.options)
        .await
        .unwrap();
    assert_eq!(status.staged.len(), 1);
    assert!(status.untracked.is_empty());
    assert_eq!(std::fs::read(repo.path("A")).unwrap(), b"staged A\n");
    assert_eq!(std::fs::read(repo.path("B")).unwrap(), b"B base\n");
    assert!(!repo.path("untracked").exists());
    let diff = stash::diff(&repo.runner, &repo.options, &listed[0])
        .await
        .unwrap();
    assert!(diff
        .windows(b"unstaged B".len())
        .any(|v| v == b"unstaged B"));
    assert!(diff
        .windows(b"untracked content".len())
        .any(|v| v == b"untracked content"));
    let show = repo
        .git(
            "stash",
            &[
                "show",
                "--patch",
                "--binary",
                "--include-untracked",
                "--no-color",
                "--no-ext-diff",
                "--no-textconv",
                "--ignore-submodules=all",
                &listed[0].oid,
            ],
        )
        .await;
    assert_eq!(diff, show);
}

#[tokio::test]
async fn apply_and_pop_reinstate_the_index_and_pop_conflicts_retain_the_stash() {
    let repo = Repo::new().await;
    repo.initial().await;
    repo.put("A", "staged\n");
    repo.git("add", &["--", "A"]).await;
    repo.put("B", "unstaged\n");
    create(&repo, "index state", false, false).await;
    let item = entries(&repo).await.remove(0);
    assert!(matches!(
        stash::execute(
            &repo.runner,
            &repo.options,
            &token(&repo).await,
            &StashAction::Apply {
                entry: item.clone(),
                reinstate_index: true
            }
        )
        .await
        .unwrap(),
        StashOutcome::Completed
    ));
    let state = repo::status::read(&repo.runner, &repo.options)
        .await
        .unwrap();
    assert_eq!(state.staged.len(), 1);
    assert_eq!(state.unstaged.len(), 1);
    repo.git("reset", &["--hard", "HEAD"]).await;
    assert!(matches!(
        stash::execute(
            &repo.runner,
            &repo.options,
            &token(&repo).await,
            &StashAction::Pop {
                entry: item,
                reinstate_index: true
            }
        )
        .await
        .unwrap(),
        StashOutcome::Completed
    ));
    assert!(entries(&repo).await.is_empty());
    assert_eq!(
        repo::status::read(&repo.runner, &repo.options)
            .await
            .unwrap()
            .staged
            .len(),
        1
    );

    let conflicted = Repo::new().await;
    conflicted.initial().await;
    conflicted.put("A", "stash side\n");
    create(&conflicted, "will conflict", false, false).await;
    conflicted.git("checkout", &["-qb", "other"]).await;
    conflicted.put("A", "committed other side\n");
    conflicted.git("commit", &["-am", "other side"]).await;
    let item = entries(&conflicted).await.remove(0);
    let outcome = stash::execute(
        &conflicted.runner,
        &conflicted.options,
        &token(&conflicted).await,
        &StashAction::Pop {
            entry: item,
            reinstate_index: false,
        },
    )
    .await
    .unwrap();
    assert!(matches!(outcome, StashOutcome::Conflicts { .. }));
    assert_eq!(entries(&conflicted).await.len(), 1);
    assert!(!repo::status::read(&conflicted.runner, &conflicted.options)
        .await
        .unwrap()
        .conflicts
        .is_empty());
}

#[tokio::test]
async fn drop_clear_branch_and_stale_or_malicious_stash_targets_are_guarded() {
    let repo = Repo::new().await;
    repo.initial().await;
    repo.put("A", "one\n");
    create(&repo, "one", false, false).await;
    let old = entries(&repo).await.remove(0);
    repo.put("A", "two\n");
    create(&repo, "two", false, false).await;
    let before = token(&repo).await;
    let (first_page, total, _) = stash::page(&repo.runner, &repo.options, 0, 1)
        .await
        .unwrap();
    let (second_page, _, _) = stash::page(&repo.runner, &repo.options, 1, 2)
        .await
        .unwrap();
    assert_eq!(total, 2);
    assert_eq!(first_page[0].selector, "stash@{0}");
    assert_eq!(second_page[0].selector, "stash@{1}");
    assert!(stash::execute(
        &repo.runner,
        &repo.options,
        &before,
        &StashAction::Drop { entry: old.clone() }
    )
    .await
    .is_err());
    assert_eq!(entries(&repo).await.len(), 2);
    let current = entries(&repo).await.remove(0);
    stash::execute(
        &repo.runner,
        &repo.options,
        &before,
        &StashAction::Drop { entry: current },
    )
    .await
    .unwrap();
    let (page, count, list_token) = stash::page(&repo.runner, &repo.options, 0, 20)
        .await
        .unwrap();
    assert_eq!(count, 1);
    assert_eq!(page.len(), 1);
    stash::execute(
        &repo.runner,
        &repo.options,
        &before,
        &StashAction::Clear { list_token },
    )
    .await
    .unwrap();
    assert!(entries(&repo).await.is_empty());

    repo.put("A", "branch from stash\n");
    create(&repo, "branch target", false, false).await;
    let item = entries(&repo).await.remove(0);
    let expected = token(&repo).await;
    stash::execute(
        &repo.runner,
        &repo.options,
        &expected,
        &StashAction::Branch {
            entry: item,
            name: "from-stash".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        String::from_utf8(repo.git("branch", &["--show-current"]).await)
            .unwrap()
            .trim(),
        "from-stash"
    );
    assert!(entries(&repo).await.is_empty());

    let bad = StashEntry {
        selector: "-stash".into(),
        oid: "-bad".into(),
        subject: String::new(),
    };
    assert!(stash::execute(
        &repo.runner,
        &repo.options,
        &token(&repo).await,
        &StashAction::Drop { entry: bad }
    )
    .await
    .is_err());
    repo.put("A", "bad branch stash\n");
    create(&repo, "bad branch", false, false).await;
    let item = entries(&repo).await.remove(0);
    assert!(stash::execute(
        &repo.runner,
        &repo.options,
        &token(&repo).await,
        &StashAction::Branch {
            entry: item,
            name: "-option".into()
        }
    )
    .await
    .is_err());
}

#[tokio::test]
async fn changes_add_delete_and_escape_paths_follow_real_status_and_filesystem() {
    let repo = Repo::new().await;
    repo.initial().await;
    repo.put("new file", "untracked\n");
    changes::add(
        &repo.runner,
        &repo.options,
        &[RepoPath::new(b"new file")],
        &token(&repo).await,
    )
    .await
    .unwrap();
    assert_eq!(
        repo.read(ReadOperation::Diff, &["--cached", "--name-only"])
            .await,
        b"new file\n"
    );
    changes::delete(
        &repo.runner,
        &repo.options,
        &[RepoPath::new(b"new file")],
        &token(&repo).await,
    )
    .await
    .unwrap();
    assert!(!repo.path("new file").exists());
    assert!(changes::add(
        &repo.runner,
        &repo.options,
        &[RepoPath::new(b"-option")],
        &token(&repo).await
    )
    .await
    .is_err());
    assert!(changes::delete(
        &repo.runner,
        &repo.options,
        &[RepoPath::new(b"../outside")],
        &token(&repo).await
    )
    .await
    .is_err());

    std::fs::create_dir(repo.path("dir")).unwrap();
    repo.put("dir/tracked", "tracked\n");
    repo.git("add", &["--", "dir/tracked"]).await;
    repo.git("commit", &["-qm", "nested"]).await;
    let expected = token(&repo).await;
    std::fs::remove_file(repo.path("dir/tracked")).unwrap();
    std::fs::remove_dir(repo.path("dir")).unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("tracked"), b"outside\n").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(outside.path(), repo.path("dir")).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(outside.path(), repo.path("dir")).unwrap();
    let error = changes::delete(
        &repo.runner,
        &repo.options,
        &[RepoPath::new(b"dir/tracked")],
        &expected,
    )
    .await;
    assert!(error.is_err());
    assert_eq!(
        std::fs::read(outside.path().join("tracked")).unwrap(),
        b"outside\n"
    );
}

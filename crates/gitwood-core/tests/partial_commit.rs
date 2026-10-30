mod support;
use gitwood_core::{
    git::ReadOperation as R,
    ops::{CommitError, CommitOutcome},
};
use std::fs;
use support::Repo;

#[tokio::test]
async fn staged_a_survives_committing_worktree_b() {
    let repo = Repo::new().await;
    repo.initial().await;
    repo.put("A", "A staged\n");
    repo.git("add", &["A"]).await;
    repo.put("B", "B worktree\n");
    assert!(matches!(
        repo.commit(&["B"], vec![], false).await.unwrap(),
        CommitOutcome::Committed { .. }
    ));
    assert_eq!(repo.read(R::Show, &["HEAD:A"]).await, b"A base\n");
    assert_eq!(repo.read(R::Show, &["HEAD:B"]).await, b"B worktree\n");
    assert_eq!(
        repo.read(R::Diff, &["--cached", "--name-only"]).await,
        b"A\n"
    );
    assert!(repo.read(R::Diff, &[]).await.is_empty());
    repo.clean_temporary_files();
}

#[tokio::test]
async fn partial_hunk_preserves_disjoint_staging_and_unselected_worktree() {
    let repo = Repo::new().await;
    let base = "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\nnine\n";
    repo.put("file", base);
    repo.git("add", &["file"]).await;
    repo.git("commit", &["-qm", "base"]).await;
    repo.put("file", &base.replace("one", "STAGED"));
    repo.git("add", &["file"]).await;
    repo.put(
        "file",
        &base
            .replace("one", "STAGED")
            .replace("five", "SELECTED")
            .replace("nine", "WORKTREE"),
    );
    let patch = b"diff --git a/file b/file\n--- a/file\n+++ b/file\n@@ -4,3 +4,3 @@\n four\n-five\n+SELECTED\n six\n".to_vec();
    repo.commit(&[], vec![patch], false).await.unwrap();
    assert_eq!(
        repo.read(R::Show, &["HEAD:file"]).await,
        base.replace("five", "SELECTED").as_bytes()
    );
    assert_eq!(
        repo.read(R::Show, &[":file"]).await,
        base.replace("one", "STAGED")
            .replace("five", "SELECTED")
            .as_bytes()
    );
    let cached = String::from_utf8(repo.read(R::Diff, &["--cached"]).await).unwrap();
    let unstaged = String::from_utf8(repo.read(R::Diff, &[]).await).unwrap();
    assert!(cached.contains("+STAGED") && !cached.contains("+SELECTED"));
    assert!(unstaged.contains("+WORKTREE") && !unstaged.contains("+STAGED"));
    repo.clean_temporary_files();
}

#[tokio::test]
async fn conflicting_staging_rejects_before_commit() {
    let repo = Repo::new().await;
    repo.initial().await;
    repo.put("B", "staged\n");
    repo.git("add", &["B"]).await;
    repo.put("B", "selected\n");
    let before = repo.state().await;
    let patch = b"diff --git a/B b/B\n--- a/B\n+++ b/B\n@@ -1 +1 @@\n-B base\n+selected\n";
    assert!(matches!(
        repo.commit(&[], vec![patch.to_vec()], false).await,
        Err(CommitError::StagedConflict(_))
    ));
    assert_eq!(before, repo.state().await);
    repo.clean_temporary_files();
}

#[tokio::test]
async fn whole_files_cover_deletion_rename_untracked_and_literal_names() {
    let repo = Repo::new().await;
    repo.initial().await;
    fs::remove_file(repo.path("A")).unwrap();
    fs::rename(repo.path("B"), repo.path("renamed B")).unwrap();
    let names = vec![
        "A",
        "B",
        "renamed B",
        "中文 空格.txt",
        "-leading",
        "[literal]*.txt",
    ];
    #[cfg(not(windows))]
    let names = [names, vec!["tab\tname", "line\nname"]].concat();
    for name in names.iter().skip(3) {
        repo.put(name, "new\n");
    }
    repo.commit(&names, vec![], false).await.unwrap();
    assert!(repo
        .read(R::Status, &["--porcelain=v2", "-z"])
        .await
        .is_empty());
    assert_eq!(repo.read(R::Show, &["HEAD:renamed B"]).await, b"B base\n");
    for name in names.iter().skip(3) {
        assert_eq!(
            repo.read(R::Show, &[&format!("HEAD:{name}")]).await,
            b"new\n"
        );
    }
    repo.clean_temporary_files();
}

#[cfg(unix)]
#[tokio::test]
async fn hook_rejection_leaves_head_index_and_worktree_unchanged() {
    let repo = Repo::new().await;
    repo.initial().await;
    repo.put("A", "staged\n");
    repo.git("add", &["A"]).await;
    repo.put("B", "worktree\n");
    support::script(&repo.path(".git/hooks/pre-commit"), "git add -- A\nexit 1");
    let before = repo.state().await;
    assert!(matches!(
        repo.commit(&["B"], vec![], false).await,
        Err(CommitError::Git(_))
    ));
    assert_eq!(before, repo.state().await);
    repo.clean_temporary_files();
}

#[cfg(unix)]
#[tokio::test]
async fn signing_failure_leaves_state_unchanged() {
    let repo = Repo::new().await;
    repo.initial().await;
    repo.put("B", "worktree\n");
    let script = repo.path("fail-gpg");
    support::script(&script, "exit 1");
    repo.git("config", &["commit.gpgsign", "true"]).await;
    repo.git("config", &["gpg.program", script.to_str().unwrap()])
        .await;
    let before = repo.state().await;
    assert!(matches!(
        repo.commit(&["B"], vec![], false).await,
        Err(CommitError::Git(_))
    ));
    assert_eq!(before, repo.state().await);
    repo.clean_temporary_files();
}

#[tokio::test]
async fn existing_index_lock_is_reported_and_preserved() {
    let repo = Repo::new().await;
    repo.initial().await;
    repo.put("B", "change\n");
    fs::write(repo.path(".git/index.lock"), "external lock").unwrap();
    let before = repo.state().await;
    assert!(matches!(
        repo.commit(&["B"], vec![], false).await,
        Err(CommitError::IndexLocked(_))
    ));
    assert_eq!(before, repo.state().await);
    assert_eq!(
        fs::read(repo.path(".git/index.lock")).unwrap(),
        b"external lock"
    );
}

#[tokio::test]
async fn amend_preserves_other_head_changes_and_parent() {
    let repo = Repo::new().await;
    repo.initial().await;
    let parent = repo.read(R::RevParse, &["HEAD"]).await;
    repo.put("A", "previous commit\n");
    repo.git("add", &["A"]).await;
    repo.git("commit", &["-qm", "old"]).await;
    let old = repo.read(R::RevParse, &["HEAD"]).await;
    repo.put("B", "amended\n");
    repo.commit(&["B"], vec![], true).await.unwrap();
    assert_ne!(repo.read(R::RevParse, &["HEAD"]).await, old);
    assert_eq!(repo.read(R::RevParse, &["HEAD^1"]).await, parent);
    assert_eq!(repo.read(R::Show, &["HEAD:A"]).await, b"previous commit\n");
    assert_eq!(repo.read(R::Show, &["HEAD:B"]).await, b"amended\n");
    assert!(repo.read(R::Status, &["--porcelain"]).await.is_empty());
}

#[tokio::test]
async fn first_commit_and_amend_of_root_commit() {
    let repo = Repo::new().await;
    repo.put("new", "first\n");
    repo.commit(&["new"], vec![], false).await.unwrap();
    repo.put("new", "amended first\n");
    repo.commit(&["new"], vec![], true).await.unwrap();
    assert_eq!(repo.read(R::Show, &["HEAD:new"]).await, b"amended first\n");
    let parents = repo.read(R::Log, &["-1", "--format=%P"]).await;
    assert_eq!(parents, b"\n");
    repo.clean_temporary_files();
}

#[tokio::test]
async fn already_staged_selected_hunk_is_consumed_once() {
    let repo = Repo::new().await;
    repo.initial().await;
    repo.put("B", "selected\n");
    repo.git("add", &["B"]).await;
    let patch = b"diff --git a/B b/B\n--- a/B\n+++ b/B\n@@ -1 +1 @@\n-B base\n+selected\n";
    repo.commit(&[], vec![patch.to_vec()], false).await.unwrap();
    assert!(repo.read(R::Status, &["--porcelain"]).await.is_empty());
}

#[cfg(unix)]
#[tokio::test]
async fn successful_hook_staging_is_respected() {
    let repo = Repo::new().await;
    repo.initial().await;
    repo.put("A", "hook selected\n");
    repo.put("B", "user selected\n");
    support::script(&repo.path(".git/hooks/pre-commit"), "git add -- A");
    repo.commit(&["B"], vec![], false).await.unwrap();
    assert_eq!(repo.read(R::Show, &["HEAD:A"]).await, b"hook selected\n");
    assert!(repo.read(R::Status, &["--porcelain"]).await.is_empty());
    repo.clean_temporary_files();
}

#[tokio::test]
async fn linked_worktree_uses_its_own_git_index() {
    let mut repo = Repo::new().await;
    repo.initial().await;
    let main_index = fs::read(repo.path(".git/index")).unwrap();
    let linked = repo.path("linked");
    repo.git(
        "worktree",
        &["add", "-q", "-b", "linked", linked.to_str().unwrap()],
    )
    .await;
    repo.options.cwd = linked.clone();
    fs::write(linked.join("B"), "linked selected\n").unwrap();
    repo.commit(&["B"], vec![], false).await.unwrap();
    assert_eq!(repo.read(R::Show, &["HEAD:B"]).await, b"linked selected\n");
    assert_eq!(fs::read(repo.path(".git/index")).unwrap(), main_index);
    assert!(repo.read(R::Status, &["--porcelain"]).await.is_empty());
}

#[tokio::test]
async fn split_index_preserves_unrelated_staging() {
    let repo = Repo::new().await;
    repo.initial().await;
    repo.git("update-index", &["--split-index"]).await;
    repo.put("A", "staged\n");
    repo.git("add", &["A"]).await;
    repo.put("B", "selected\n");
    repo.commit(&["B"], vec![], false).await.unwrap();
    assert_eq!(repo.read(R::Show, &[":A"]).await, b"staged\n");
    assert_eq!(
        repo.read(R::Diff, &["--cached", "--name-only"]).await,
        b"A\n"
    );
    repo.clean_temporary_files();
}

#[tokio::test]
async fn user_diff_formatting_does_not_corrupt_internal_alignment_patch() {
    let repo = Repo::new().await;
    repo.initial().await;
    for (key, value) in [
        ("diff.noprefix", "true"),
        ("diff.mnemonicPrefix", "true"),
        ("color.ui", "always"),
        ("diff.context", "0"),
    ] {
        repo.git("config", &[key, value]).await;
    }
    repo.put("B", "selected\n");
    repo.commit(&["B"], vec![], false).await.unwrap();
    assert!(repo.read(R::Status, &["--porcelain"]).await.is_empty());
}

#[cfg(unix)]
#[tokio::test]
async fn hook_created_staging_conflict_reports_completed_commit_with_recovery() {
    let repo = Repo::new().await;
    repo.initial().await;
    repo.put("A", "original staging\n");
    repo.git("add", &["A"]).await;
    repo.put("A", "hook override\n");
    repo.put("B", "selected\n");
    support::script(&repo.path(".git/hooks/pre-commit"), "git add -- A");
    let base = String::from_utf8(repo.read(R::RevParse, &["HEAD"]).await).unwrap();
    let original = fs::read(repo.path(".git/index")).unwrap();
    match repo.commit(&["B"], vec![], false).await.unwrap() {
        CommitOutcome::CommittedWithIndexRecovery { hash, recovery } => {
            assert_eq!(
                String::from_utf8(repo.read(R::RevParse, &["HEAD"]).await)
                    .unwrap()
                    .trim(),
                hash
            );
            assert_eq!(repo.read(R::Show, &["HEAD:A"]).await, b"hook override\n");
            assert_eq!(fs::read(repo.path(".git/index")).unwrap(), original);
            assert_eq!(
                fs::read(recovery.directory.join("original")).unwrap(),
                original
            );
            assert!(recovery.directory.join("selected").exists());
            assert_eq!(recovery.directory.parent().unwrap(), repo.path(".git"));
            let name = recovery.directory.file_name().unwrap().to_string_lossy();
            assert!(name.starts_with("gitwood-recovery-") && name.contains(&hash));
            assert!(name
                .trim_start_matches("gitwood-recovery-")
                .split('-')
                .next()
                .unwrap()
                .parse::<u128>()
                .is_ok());
            assert!(
                recovery.instructions.contains("git reset --mixed")
                    && recovery.instructions.contains("git apply --cached --3way")
            );
            assert_eq!(
                fs::read_to_string(recovery.directory.join("RECOVERY.md")).unwrap(),
                recovery.instructions
            );
            // Exercise the documented recovery against the actual saved index.
            let worktree = fs::read(repo.path("A")).unwrap();
            let mut snapshot_options = repo.options.clone();
            snapshot_options.env.insert(
                "GIT_INDEX_FILE".into(),
                recovery.directory.join("original").into_os_string(),
            );
            let patch = recovery.directory.join("staged.patch");
            repo.runner
                .run(
                    gitwood_core::git::WriteCommand::new("diff").args([
                        "--cached",
                        "--binary",
                        "--full-index",
                        "--no-ext-diff",
                        "--no-textconv",
                        &format!("--output={}", patch.display()),
                        base.trim(),
                        "--",
                    ]),
                    &snapshot_options,
                )
                .await
                .unwrap();
            let staged_a = repo
                .runner
                .run(
                    gitwood_core::git::ReadCommand::new(R::RevParse).args([":A"]),
                    &snapshot_options,
                )
                .await
                .unwrap()
                .stdout;
            repo.git("reset", &["--mixed", &hash]).await;
            assert!(matches!(
                repo.runner
                    .run(
                        gitwood_core::git::WriteCommand::new("apply").args([
                            "--cached",
                            "--3way",
                            patch.to_str().unwrap()
                        ]),
                        &repo.options
                    )
                    .await,
                Err(gitwood_core::git::GitError::Exit { .. })
            ));
            assert!(!repo.read(R::LsFiles, &["--unmerged"]).await.is_empty());
            // Choose the original staging for A after inspecting the hook conflict.
            let entry = format!("100644,{},A", String::from_utf8(staged_a).unwrap().trim());
            repo.git("update-index", &["--cacheinfo", &entry]).await;
            assert!(repo.read(R::LsFiles, &["--unmerged"]).await.is_empty());
            assert_eq!(repo.read(R::Show, &[":A"]).await, b"original staging\n");
            assert_eq!(repo.read(R::Show, &[":B"]).await, b"selected\n");
            assert_eq!(fs::read(repo.path("A")).unwrap(), worktree);
            fs::remove_dir_all(recovery.directory).unwrap();
        }
        other => panic!("expected committed-with-recovery, got {other:?}"),
    }
    repo.clean_temporary_files();
}

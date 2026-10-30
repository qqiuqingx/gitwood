mod support;
use gitwood_core::{
    git::ReadOperation as R,
    repo::{
        inclusion::{Hunks, Rule, Selection},
        session::{Lease, Sessions},
        write::ResultState,
        RepoPath,
    },
};
use support::Repo;
fn path(s: &str) -> RepoPath {
    RepoPath::new(s.as_bytes())
}
async fn lease(repo: &Repo) -> Lease {
    Sessions::default()
        .open(repo.runner.clone(), repo.options.clone())
        .await
        .unwrap()
}
fn only(name: &str) -> Selection {
    Selection {
        rules: vec![
            Rule {
                id: "changes".into(),
                included: false,
            },
            Rule {
                id: format!(
                    "change/{}",
                    gitwood_core::repo::inclusion::hex(name.as_bytes())
                ),
                included: true,
            },
        ],
        ..Default::default()
    }
}
#[tokio::test]
async fn staged_a_only_b_through_session_selection_and_patch() {
    let repo = Repo::new().await;
    repo.initial().await;
    repo.put("A", "staged A\n");
    repo.git("add", &["A"]).await;
    repo.put("B", "selected B\n");
    let l = lease(&repo).await;
    let state = l.status().await.unwrap();
    let preview = l.selection_preview(&path("B")).await.unwrap();
    let s = Selection {
        rules: vec![Rule {
            id: "changes".into(),
            included: false,
        }],
        hunks: vec![Hunks {
            path: path("B"),
            token: preview.token,
            blocks: vec![0],
        }],
    };
    assert!(matches!(
        l.commit_selection(&s, "only B", false, &state.fingerprint)
            .await
            .unwrap(),
        ResultState::Committed { .. }
    ));
    assert_eq!(repo.read(R::Show, &["HEAD:A"]).await, b"A base\n");
    assert_eq!(repo.read(R::Show, &["HEAD:B"]).await, b"selected B\n");
    assert_eq!(repo.read(R::Show, &[":A"]).await, b"staged A\n");
    assert!(repo.read(R::Diff, &["--cached", "--name-only"]).await == b"A\n");
}
#[tokio::test]
async fn same_file_staged_unstaged_partial_and_rollback_lines() {
    let repo = Repo::new().await;
    let base = (0..30).map(|i| format!("line {i}\n")).collect::<String>();
    repo.put("file", &base);
    repo.git("add", &["file"]).await;
    repo.git("commit", &["-qm", "base"]).await;
    let staged = base.replace("line 1\n", "STAGED\n");
    repo.put("file", &staged);
    repo.git("add", &["file"]).await;
    let work = staged
        .replace("line 14\n", "SELECTED\n")
        .replace("line 27\n", "REMAIN\n");
    repo.put("file", &work);
    let l = lease(&repo).await;
    let summary = l.status().await.unwrap();
    let preview = l.selection_preview(&path("file")).await.unwrap();
    assert!(preview.selectable);
    assert_eq!(preview.preview.result.unwrap().blocks.len(), 3);
    let h = Hunks {
        path: path("file"),
        token: preview.token,
        blocks: vec![1],
    };
    let s = Selection {
        rules: vec![],
        hunks: vec![h],
    };
    assert!(matches!(
        l.commit_selection(&s, "middle", false, &summary.fingerprint)
            .await
            .unwrap(),
        ResultState::Committed { .. }
    ));
    assert_eq!(
        repo.read(R::Show, &["HEAD:file"]).await,
        base.replace("line 14\n", "SELECTED\n").as_bytes()
    );
    assert_eq!(
        repo.read(R::Show, &[":file"]).await,
        staged.replace("line 14\n", "SELECTED\n").as_bytes()
    );
    assert_eq!(std::fs::read(repo.path("file")).unwrap(), work.as_bytes());
    let preview = l.selection_preview(&path("file")).await.unwrap();
    l.rollback_hunk(&Hunks {
        path: path("file"),
        token: preview.token,
        blocks: vec![1],
    })
    .await
    .unwrap();
    assert_eq!(
        std::fs::read(repo.path("file")).unwrap(),
        staged.replace("line 14\n", "SELECTED\n").as_bytes()
    );
}
#[tokio::test]
async fn whole_delete_rename_untracked_special_and_initial_amend() {
    let repo = Repo::new().await;
    repo.initial().await;
    repo.git("mv", &["A", "renamed"]).await;
    std::fs::remove_file(repo.path("B")).unwrap();
    let mut names = vec!["-option", "中文 space"];
    #[cfg(not(windows))]
    {
        names.extend(["tab\tname", "line\nname"]);
    }
    for name in &names {
        repo.put(name, "new\r\nno newline");
    }
    let l = lease(&repo).await;
    let state = l.status().await.unwrap();
    let s = Selection {
        rules: vec![Rule {
            id: "unversioned".into(),
            included: true,
        }],
        hunks: vec![],
    };
    l.commit_selection(&s, "files", false, &state.fingerprint)
        .await
        .unwrap();
    assert!(repo
        .read(R::Diff, &["HEAD", "--name-only"])
        .await
        .is_empty());
    for name in names {
        assert_eq!(
            repo.read(R::Show, &[&format!("HEAD:{name}")]).await,
            b"new\r\nno newline"
        );
    }
    let repo = Repo::new().await;
    repo.put("new", "first\n");
    let l = lease(&repo).await;
    let state = l.status().await.unwrap();
    l.commit_selection(
        &Selection {
            rules: vec![Rule {
                id: "unversioned".into(),
                included: true,
            }],
            ..Default::default()
        },
        "first",
        false,
        &state.fingerprint,
    )
    .await
    .unwrap();
    assert_eq!(l.last_message().await.unwrap(), "first");
    let state = l.status().await.unwrap();
    l.commit_selection(&Selection::default(), "amended", true, &state.fingerprint)
        .await
        .unwrap();
    assert_eq!(l.last_message().await.unwrap(), "amended");
}
#[tokio::test]
async fn rollback_modified_added_deleted_staged_dash_and_lock() {
    for delete_added in [false, true] {
        let repo = Repo::new().await;
        repo.initial().await;
        repo.put("-dash", "base");
        repo.git("add", &["--", "-dash"]).await;
        repo.git("commit", &["-qm", "dash"]).await;
        repo.put("A", "new staged");
        repo.git("add", &["A"]).await;
        repo.put("-dash", "changed");
        std::fs::remove_file(repo.path("B")).unwrap();
        repo.put("added", "new");
        repo.git("add", &["added"]).await;
        repo.put("untracked", "new");
        let l = lease(&repo).await;
        let state = l.status().await.unwrap();
        let paths = ["A", "B", "-dash", "added", "untracked"].map(path);
        std::fs::write(repo.path(".git/index.lock"), "external").unwrap();
        assert!(l
            .rollback_selection(&paths, delete_added, &state.fingerprint)
            .await
            .unwrap_err()
            .to_string()
            .contains("locked"));
        assert_eq!(
            std::fs::read(repo.path(".git/index.lock")).unwrap(),
            b"external"
        );
        std::fs::remove_file(repo.path(".git/index.lock")).unwrap();
        l.rollback_selection(&paths, delete_added, &state.fingerprint)
            .await
            .unwrap();
        assert_eq!(std::fs::read(repo.path("A")).unwrap(), b"A base\n");
        assert_eq!(std::fs::read(repo.path("B")).unwrap(), b"B base\n");
        assert_eq!(std::fs::read(repo.path("-dash")).unwrap(), b"base");
        assert_eq!(repo.path("added").exists(), !delete_added);
        assert_eq!(repo.path("untracked").exists(), !delete_added);
        assert!(repo.read(R::Diff, &["--cached"]).await.is_empty());
    }
}
#[tokio::test]
async fn external_content_change_same_status_is_rejected_and_empty_message() {
    let repo = Repo::new().await;
    repo.initial().await;
    repo.put("B", "first\n");
    let l = lease(&repo).await;
    let state = l.status().await.unwrap();
    repo.put("B", "other\n");
    assert!(l
        .commit_selection(&only("B"), "test", false, &state.fingerprint)
        .await
        .unwrap_err()
        .to_string()
        .contains("externally"));
    assert!(l
        .rollback_selection(&[path("B")], false, &state.fingerprint)
        .await
        .is_err());
    assert_eq!(std::fs::read(repo.path("B")).unwrap(), b"other\n");
    assert!(l
        .commit_selection(&only("B"), " \n", false, &state.fingerprint)
        .await
        .is_err());
}
#[cfg(unix)]
#[tokio::test]
async fn hooks_path_template_signing_failures_and_recovery() {
    let repo = Repo::new().await;
    repo.initial().await;
    repo.put("A", "staged\n");
    repo.git("add", &["A"]).await;
    repo.put("B", "selected\n");
    repo.put("template", "template content");
    repo.git("config", &["commit.template", "template"]).await;
    repo.git("config", &["core.hooksPath", "custom-hooks"])
        .await;
    support::script(
        &repo.path("custom-hooks/pre-commit"),
        "echo rejected-by-custom-hook >&2; exit 1",
    );
    let l = lease(&repo).await;
    let state = l.status().await.unwrap();
    let before = repo.state().await;
    assert!(l
        .commit_selection(&only("B"), "message", false, &state.fingerprint)
        .await
        .unwrap_err()
        .to_string()
        .contains("rejected-by-custom-hook"));
    assert_eq!(before, repo.state().await);
    std::fs::remove_file(repo.path("custom-hooks/pre-commit")).unwrap();
    support::script(&repo.path("signer"), "echo signer-refused >&2; exit 1");
    repo.git(
        "config",
        &["gpg.program", repo.path("signer").to_str().unwrap()],
    )
    .await;
    repo.git("config", &["commit.gpgsign", "true"]).await;
    let state = l.status().await.unwrap();
    assert!(l
        .commit_selection(&only("B"), "message", false, &state.fingerprint)
        .await
        .unwrap_err()
        .to_string()
        .contains("signer-refused"));
    assert_eq!(before, repo.state().await);
    repo.git("config", &["commit.gpgsign", "false"]).await;
    support::script(
        &repo.path("custom-hooks/pre-commit"),
        "echo custom-hook-ran >&2\necho marker > .git/hook-marker",
    );
    let state = l.status().await.unwrap();
    let result = l
        .commit_selection(&only("B"), "message", false, &state.fingerprint)
        .await
        .unwrap();
    assert!(matches!(result, ResultState::Committed { .. }));
    assert!(repo.path(".git/hook-marker").exists());
    assert_eq!(
        repo.git("config", &["--get", "commit.template"]).await,
        b"template\n"
    );
}
#[tokio::test]
async fn directory_states_and_refresh_path_rules() {
    use gitwood_core::repo::{
        file_tree::FileTree,
        inclusion::{matches, Inclusion},
    };
    let repo = Repo::new().await;
    repo.initial().await;
    repo.put("A", "changed");
    repo.put("B", "changed");
    repo.put("new", "new");
    let l = lease(&repo).await;
    l.status().await.unwrap();
    let state = gitwood_core::repo::status::read(&repo.runner, &repo.options)
        .await
        .unwrap();
    let tree = FileTree::changes(&state, false);
    let page = tree.page(&std::collections::HashSet::from(["*".into()]), 0, 200, &[]);
    let nodes = page.rows.into_iter().map(|r| r.node).collect::<Vec<_>>();
    let states = l.inclusion(&only("B"), &nodes).await.unwrap();
    assert_eq!(states.states[0], Inclusion::Some);
    assert!(states.states.contains(&Inclusion::None));
    assert_eq!(states.count, 1);
    assert!(!matches("changes/dir/61", &path("B"), false));
    repo.put("C", "new");
    l.status().await.unwrap();
    assert_eq!(l.inclusion(&only("B"), &nodes).await.unwrap().count, 1);
}
#[cfg(unix)]
#[tokio::test]
async fn session_recovery_is_distinct_and_preserves_index() {
    let repo = Repo::new().await;
    repo.initial().await;
    repo.put("A", "staged\n");
    repo.git("add", &["A"]).await;
    repo.put("A", "hook override\n");
    repo.put("B", "selected\n");
    support::script(&repo.path(".git/hooks/pre-commit"), "git add -- A");
    let original = std::fs::read(repo.path(".git/index")).unwrap();
    let l = lease(&repo).await;
    let state = l.status().await.unwrap();
    match l
        .commit_selection(&only("B"), "recover", false, &state.fingerprint)
        .await
        .unwrap()
    {
        ResultState::Recovery {
            hash,
            directory,
            instructions,
            ..
        } => {
            assert!(instructions.contains("Do not retry"));
            assert!(instructions.contains(directory.to_str().unwrap()));
            assert_eq!(
                repo.read(R::RevParse, &["HEAD"]).await,
                (hash + "\n").as_bytes()
            );
            assert!(directory.join("RECOVERY.md").exists());
            assert_eq!(std::fs::read(repo.path(".git/index")).unwrap(), original);
        }
        other => panic!("expected recovery, got {other:?}"),
    }
}
#[cfg(unix)]
#[tokio::test]
async fn session_write_rejects_second_write_and_cancels_hook() {
    for signer in [false, true] {
        let repo = Repo::new().await;
        repo.initial().await;
        repo.put("B", "selected\n");
        let helper = if signer {
            repo.path("signer")
        } else {
            repo.path(".git/hooks/pre-commit")
        };
        let mut ready = support::ProcessSignal::new(repo.path(".git/ready-pipe"));
        support::script(&helper, "printf R > .git/ready-pipe\nexec sleep 30");
        if signer {
            repo.git("config", &["gpg.program", helper.to_str().unwrap()])
                .await;
            repo.git("config", &["commit.gpgsign", "true"]).await;
        }
        let l = lease(&repo).await;
        let state = l.status().await.unwrap();
        let selection = only("B");
        let before = repo.state().await;
        let run = l.commit_selection(&selection, "cancel me", false, &state.fingerprint);
        tokio::pin!(run);
        // Other real-repository tests can hold the shared ops write gate. Wait
        // for the actual helper, rather than charging queue time to a startup
        // deadline. An early commit error must fail here, not hang the observer.
        tokio::select! {
            signal = ready.wait() => signal.expect("helper readiness signal"),
            result = &mut run => panic!("commit ended before helper readiness (signer={signer}): {result:?}"),
        }
        assert!(l
            .rollback_selection(&[path("B")], false, &state.fingerprint)
            .await
            .unwrap_err()
            .to_string()
            .contains("already running"));
        assert!(l
            .commit_selection(&selection, "second", false, &state.fingerprint)
            .await
            .unwrap_err()
            .to_string()
            .contains("already running"));
        l.cancel_write();
        let result = run.await;
        assert!(result.unwrap_err().to_string().contains("cancel"));
        assert_eq!(before, repo.state().await);
        repo.clean_temporary_files();
        // Awaiting cancellation must also release the session guard and real
        // index lock: a subsequent production write is allowed to complete.
        if signer {
            repo.git("config", &["commit.gpgsign", "false"]).await;
        } else {
            std::fs::remove_file(helper).unwrap();
        }
        let state = l.status().await.unwrap();
        assert!(matches!(
            l.commit_selection(&selection, "after cancellation", false, &state.fingerprint)
                .await
                .unwrap(),
            ResultState::Committed { .. }
        ));
        assert_eq!(repo.read(R::Show, &["HEAD:B"]).await, b"selected\n");
        repo.clean_temporary_files();
    }
}
#[tokio::test]
async fn partial_patch_special_names_crlf_empty_and_no_newline() {
    let mut names = vec!["- leading 中文"];
    #[cfg(not(windows))]
    {
        names.push("tab\tand\nline");
    }
    for name in names {
        for (base, work) in [
            ("one\r\ntwo\r\nthree", "ONE\r\ntwo\r\nTHREE"),
            ("last\n", "last"),
            ("x", ""),
            ("", "new"),
        ] {
            let repo = Repo::new().await;
            repo.put(name, base);
            repo.git("add", &["--", name]).await;
            repo.git("commit", &["-qm", "base"]).await;
            repo.put(name, work);
            let l = lease(&repo).await;
            let state = l.status().await.unwrap();
            let preview = l.selection_preview(&path(name)).await.unwrap();
            let count = preview.preview.result.unwrap().blocks.len();
            assert!(count > 0);
            let selection = Selection {
                rules: vec![Rule {
                    id: "changes".into(),
                    included: false,
                }],
                hunks: vec![Hunks {
                    path: path(name),
                    token: preview.token,
                    blocks: vec![0],
                }],
            };
            l.commit_selection(&selection, "partial", false, &state.fingerprint)
                .await
                .unwrap();
            let committed = repo.read(R::Show, &[&format!("HEAD:{name}")]).await;
            assert_eq!(
                committed,
                if count == 1 {
                    work.as_bytes()
                } else {
                    b"ONE\r\ntwo\r\nthree"
                }
            );
            assert_eq!(std::fs::read(repo.path(name)).unwrap(), work.as_bytes());
        }
    }
}
#[tokio::test]
async fn rollback_directory_preview_is_bounded_and_initial_added_default_keeps_disk() {
    let repo = Repo::new().await;
    std::fs::create_dir(repo.path("dir")).unwrap();
    repo.put("dir/new", "new");
    repo.git("add", &["dir/new"]).await;
    let l = lease(&repo).await;
    let state = l.status().await.unwrap();
    let page = l.rollback_files(&["changes".into()], 0, 200).await.unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.paths[0], path("dir/new"));
    assert!(l.rollback_files(&["changes".into()], 0, 201).await.is_err());
    l.rollback_ids(&["changes".into()], false, &state.fingerprint)
        .await
        .unwrap();
    assert!(repo.path("dir/new").exists());
    assert!(repo.read(R::LsFiles, &[]).await.is_empty());
}
#[tokio::test]
async fn commit_index_lock_and_stale_hunk_do_not_write() {
    let repo = Repo::new().await;
    repo.initial().await;
    repo.put("B", "first\n");
    let l = lease(&repo).await;
    let state = l.status().await.unwrap();
    let preview = l.selection_preview(&path("B")).await.unwrap();
    std::fs::write(repo.path(".git/index.lock"), "other writer").unwrap();
    assert!(l
        .commit_selection(&only("B"), "locked", false, &state.fingerprint)
        .await
        .unwrap_err()
        .to_string()
        .contains("locked"));
    std::fs::remove_file(repo.path(".git/index.lock")).unwrap();
    repo.put("B", "later\n");
    let state = l.status().await.unwrap();
    let selection = Selection {
        rules: vec![],
        hunks: vec![Hunks {
            path: path("B"),
            token: preview.token,
            blocks: vec![0],
        }],
    };
    assert!(l
        .commit_selection(&selection, "stale", false, &state.fingerprint)
        .await
        .is_err());
    assert_eq!(repo.read(R::Show, &["HEAD:B"]).await, b"B base\n");
}
#[tokio::test]
async fn rollback_rename_restores_source_and_removes_destination() {
    let repo = Repo::new().await;
    repo.initial().await;
    repo.git("mv", &["A", "renamed"]).await;
    let l = lease(&repo).await;
    let state = l.status().await.unwrap();
    l.rollback_selection(&[path("renamed")], false, &state.fingerprint)
        .await
        .unwrap();
    assert_eq!(std::fs::read(repo.path("A")).unwrap(), b"A base\n");
    assert!(!repo.path("renamed").exists());
    assert!(repo.read(R::Status, &["--porcelain"]).await.is_empty());
}

#[tokio::test]
async fn partial_rename_preserves_staged_edits_and_other_worktree_blocks() {
    for change_mode in [false, true] {
        if change_mode && !cfg!(unix) {
            continue;
        }
        let repo = Repo::new().await;
        let base = (0..40).map(|i| format!("line {i}\n")).collect::<String>();
        let old = "old name";
        let new = "- renamed 中文";
        repo.put(old, &base);
        repo.git("add", &["--", old]).await;
        repo.git("commit", &["-qm", "base"]).await;
        repo.git("mv", &["--", old, new]).await;
        let staged = base.replace("line 1\n", "STAGED\n");
        repo.put(new, &staged);
        repo.git("add", &["--", new]).await;
        let working = staged
            .replace("line 17\n", "SELECTED\n")
            .replace("line 35\n", "WORKTREE\n");
        repo.put(new, &working);
        #[cfg(unix)]
        if change_mode {
            use std::os::unix::fs::PermissionsExt;
            repo.git("config", &["core.filemode", "true"]).await;
            std::fs::set_permissions(repo.path(new), std::fs::Permissions::from_mode(0o755))
                .unwrap();
        }
        let l = lease(&repo).await;
        let state = l.status().await.unwrap();
        let preview = l.selection_preview(&path(new)).await.unwrap();
        assert!(preview.selectable);
        assert_eq!(preview.preview.result.unwrap().blocks.len(), 3);
        let selection = Selection {
            rules: vec![Rule {
                id: "changes".into(),
                included: false,
            }],
            hunks: vec![Hunks {
                path: path(new),
                token: preview.token,
                blocks: vec![1],
            }],
        };
        #[cfg(unix)]
        {
            let before = repo.state().await;
            support::script(
                &repo.path(".git/hooks/pre-commit"),
                "echo metadata-hook-rejected >&2; exit 1",
            );
            assert!(l
                .commit_selection(&selection, "rejected rename", false, &state.fingerprint)
                .await
                .unwrap_err()
                .to_string()
                .contains("metadata-hook-rejected"));
            assert_eq!(before, repo.state().await);
            std::fs::remove_file(repo.path(".git/hooks/pre-commit")).unwrap();
        }
        assert!(matches!(
            l.commit_selection(&selection, "partial rename", false, &state.fingerprint)
                .await
                .unwrap(),
            ResultState::Committed { .. }
        ));
        assert_eq!(
            repo.read(R::Show, &[&format!("HEAD:{new}")]).await,
            base.replace("line 17\n", "SELECTED\n").as_bytes()
        );
        assert_eq!(
            repo.read(R::Show, &[&format!(":{new}")]).await,
            staged.replace("line 17\n", "SELECTED\n").as_bytes()
        );
        let expected_mode = if change_mode { "100755 " } else { "100644 " };
        assert!(repo
            .read(R::LsTree, &["HEAD", "--", new])
            .await
            .starts_with(expected_mode.as_bytes()));
        assert!(repo
            .read(R::LsFiles, &["--stage", "--", new])
            .await
            .starts_with(expected_mode.as_bytes()));
        assert!(!repo.path(old).exists());
        assert_eq!(std::fs::read(repo.path(new)).unwrap(), working.as_bytes());
        assert_eq!(
            repo.read(R::LsTree, &["--name-only", "HEAD", "--", old])
                .await,
            b""
        );
        let preview = l.selection_preview(&path(new)).await.unwrap();
        l.rollback_hunk(&Hunks {
            path: path(new),
            token: preview.token,
            blocks: vec![1],
        })
        .await
        .unwrap();
        assert_eq!(
            std::fs::read(repo.path(new)).unwrap(),
            staged.replace("line 17\n", "SELECTED\n").as_bytes()
        );
    }
}
#[cfg(unix)]
#[tokio::test]
async fn partial_mode_change_preserves_staged_text_and_remaining_worktree() {
    use std::os::unix::fs::PermissionsExt;
    for staged_mode in [false, true] {
        let repo = Repo::new().await;
        repo.git("config", &["core.filemode", "true"]).await;
        let base = (0..40).map(|i| format!("line {i}\n")).collect::<String>();
        repo.put("file", &base);
        std::fs::set_permissions(repo.path("file"), std::fs::Permissions::from_mode(0o644))
            .unwrap();
        repo.git("add", &["file"]).await;
        repo.git("commit", &["-qm", "base"]).await;
        let staged = base.replace("line 1\n", "STAGED\n");
        repo.put("file", &staged);
        if staged_mode {
            std::fs::set_permissions(repo.path("file"), std::fs::Permissions::from_mode(0o755))
                .unwrap();
        }
        repo.git("add", &["file"]).await;
        let working = staged
            .replace("line 17\n", "SELECTED\n")
            .replace("line 35\n", "WORKTREE\n");
        repo.put("file", &working);
        std::fs::set_permissions(repo.path("file"), std::fs::Permissions::from_mode(0o755))
            .unwrap();
        let l = lease(&repo).await;
        let state = l.status().await.unwrap();
        let preview = l.selection_preview(&path("file")).await.unwrap();
        assert!(preview.selectable);
        let selection = Selection {
            rules: vec![],
            hunks: vec![Hunks {
                path: path("file"),
                token: preview.token,
                blocks: vec![1],
            }],
        };
        assert!(matches!(
            l.commit_selection(&selection, "partial mode", false, &state.fingerprint)
                .await
                .unwrap(),
            ResultState::Committed { .. }
        ));
        assert_eq!(
            repo.read(R::Show, &["HEAD:file"]).await,
            base.replace("line 17\n", "SELECTED\n").as_bytes()
        );
        assert_eq!(
            repo.read(R::Show, &[":file"]).await,
            staged.replace("line 17\n", "SELECTED\n").as_bytes()
        );
        assert_eq!(
            std::fs::read(repo.path("file")).unwrap(),
            working.as_bytes()
        );
        assert!(repo
            .read(R::LsTree, &["HEAD", "--", "file"])
            .await
            .starts_with(b"100755 "));
        assert!(repo
            .read(R::LsFiles, &["--stage", "--", "file"])
            .await
            .starts_with(b"100755 "));
        assert_eq!(
            std::fs::metadata(repo.path("file"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o755
        );
    }
}

#[cfg(unix)]
#[tokio::test]
async fn partial_text_preserves_unselected_staged_mode_delta() {
    use std::os::unix::fs::PermissionsExt;
    for (head_mode, index_mode) in [(0o644, 0o755), (0o755, 0o644)] {
        let repo = Repo::new().await;
        repo.git("config", &["core.filemode", "true"]).await;
        let base = (0..40).map(|i| format!("line {i}\n")).collect::<String>();
        repo.put("file", &base);
        std::fs::set_permissions(
            repo.path("file"),
            std::fs::Permissions::from_mode(head_mode),
        )
        .unwrap();
        repo.git("add", &["file"]).await;
        repo.git("commit", &["-qm", "base"]).await;
        let staged = base.replace("line 1\n", "STAGED\n");
        repo.put("file", &staged);
        std::fs::set_permissions(
            repo.path("file"),
            std::fs::Permissions::from_mode(index_mode),
        )
        .unwrap();
        repo.git("add", &["file"]).await;
        let working = staged.replace("line 17\n", "SELECTED\n");
        repo.put("file", &working);
        std::fs::set_permissions(
            repo.path("file"),
            std::fs::Permissions::from_mode(head_mode),
        )
        .unwrap();
        let l = lease(&repo).await;
        let state = l.status().await.unwrap();
        let preview = l.selection_preview(&path("file")).await.unwrap();
        assert!(preview.selectable);
        let selection = Selection {
            rules: vec![],
            hunks: vec![Hunks {
                path: path("file"),
                token: preview.token,
                blocks: vec![1],
            }],
        };
        l.commit_selection(&selection, "text only", false, &state.fingerprint)
            .await
            .unwrap();
        let head_prefix = format!("100{head_mode:o} ");
        let index_prefix = format!("100{index_mode:o} ");
        assert!(repo
            .read(R::LsTree, &["HEAD", "--", "file"])
            .await
            .starts_with(head_prefix.as_bytes()));
        assert!(
            repo.read(R::LsFiles, &["--stage", "--", "file"])
                .await
                .starts_with(index_prefix.as_bytes()),
            "unselected index mode must survive"
        );
        assert_eq!(
            repo.read(R::Show, &[":file"]).await,
            staged.replace("line 17\n", "SELECTED\n").as_bytes()
        );
    }
}

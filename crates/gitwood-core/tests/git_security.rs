mod support;
use gitwood_core::git::{GitError, ReadCommand, ReadOperation};
use support::Repo;

// Executable shell fixtures are Unix-only. Windows CI runs the portable ownership,
// policy and filename tests; .bat execution of malicious Git helpers is not verified.
#[cfg(unix)]
#[tokio::test]
async fn passive_reads_never_execute_repository_helpers() {
    let repo = Repo::new().await;
    repo.initial().await;
    repo.put(".gitattributes", "B diff=evil\n");
    for (key, sentinel) in [
        ("core.fsmonitor", "fsmonitor"),
        ("diff.external", "external"),
        ("diff.evil.command", "driver-command"),
        ("diff.evil.textconv", "textconv"),
        ("core.pager", "pager"),
        ("core.sshCommand", "ssh"),
    ] {
        let path = repo.path(&format!("evil-{sentinel}"));
        support::script(
            &path,
            &format!(
                "touch '{}'\nexit 1",
                repo.path(&format!("pwned-{sentinel}")).display()
            ),
        );
        repo.git("config", &[key, path.to_str().unwrap()]).await;
    }
    repo.put("B", "changed\n");
    for (op, args) in [
        (ReadOperation::Status, vec!["--porcelain=v2", "-z"]),
        (ReadOperation::Diff, vec![]),
        (ReadOperation::Log, vec!["-p"]),
        (ReadOperation::Show, vec!["HEAD"]),
        (ReadOperation::RevParse, vec!["HEAD"]),
        (ReadOperation::LsFiles, vec!["--eol"]),
        (ReadOperation::LsTree, vec!["HEAD"]),
        (ReadOperation::DiffTree, vec!["--root", "-p", "HEAD"]),
    ] {
        repo.read(op, &args).await;
    }
    for name in [
        "fsmonitor",
        "external",
        "driver-command",
        "textconv",
        "pager",
        "ssh",
    ] {
        assert!(
            !repo.path(&format!("pwned-{name}")).exists(),
            "{name} executed"
        );
    }
}

#[cfg(unix)]
#[tokio::test]
async fn status_verbose_textconv_is_rejected() {
    let repo = Repo::new().await;
    repo.initial().await;
    let helper = repo.path("status-textconv");
    support::script(&helper, "touch pwned-status-textconv\ncat \"$1\"");
    repo.put(".gitattributes", "B diff=evil\n");
    repo.git("config", &["diff.evil.textconv", helper.to_str().unwrap()])
        .await;
    repo.put("B", "changed\n");
    repo.git("status", &["-vv"]).await;
    assert!(repo.path("pwned-status-textconv").exists());
    std::fs::remove_file(repo.path("pwned-status-textconv")).unwrap();
    for flag in ["-v", "-vv", "-bv", "--verbose", "--verb"] {
        assert!(matches!(
            repo.runner
                .run(
                    ReadCommand::new(ReadOperation::Status).args([flag]),
                    &repo.options
                )
                .await,
            Err(GitError::InvalidCommand(_))
        ));
    }
    repo.git("config", &["commit.verbose", "true"]).await;
    repo.git("config", &["status.verbose", "true"]).await;
    repo.read(ReadOperation::Status, &["--porcelain=v2"]).await;
    assert!(!repo.path("pwned-status-textconv").exists());
}

#[tokio::test]
async fn dubious_ownership_has_a_dedicated_error() {
    let mut repo = Repo::new().await;
    repo.options
        .env
        .insert("GIT_TEST_ASSUME_DIFFERENT_OWNER".into(), "1".into());
    let error = repo
        .runner
        .run(ReadCommand::new(ReadOperation::Status), &repo.options)
        .await
        .unwrap_err();
    match error {
        GitError::UnsafeRepository { path, stderr } => {
            assert_eq!(
                std::fs::canonicalize(path).unwrap(),
                std::fs::canonicalize(repo.dir.path()).unwrap()
            );
            assert!(stderr.contains("detected dubious ownership"));
        }
        other => panic!("expected ownership error, got {other:?}"),
    }
}

#[tokio::test]
async fn callers_cannot_reenable_unsafe_read_options() {
    let repo = Repo::new().await;
    for flag in [
        "--ext-diff",
        "--textconv",
        "--output=oops",
        "--open-files-in-pager",
        "--open-files",
        "--out=oops",
        "--textcon",
        "--show-signature",
        "--show-sign",
        "--alternate-refs",
        "--remerge-diff",
        "--diff-merges=remerge",
        "--diff-merges=on",
        "--ignore-submodules=none",
        "--no-ignore-submodules",
    ] {
        assert!(matches!(
            repo.runner
                .run(
                    ReadCommand::new(ReadOperation::Diff).args([flag]),
                    &repo.options
                )
                .await,
            Err(GitError::InvalidCommand(_))
        ));
    }
}

#[tokio::test]
async fn signature_formats_and_repository_pretty_aliases_are_rejected() {
    let repo = Repo::new().await;
    repo.initial().await;
    for op in [
        ReadOperation::Log,
        ReadOperation::Show,
        ReadOperation::DiffTree,
    ] {
        for format in [
            "%G?", "%GS", "%GK", "%GF", "%GP", "%GT", "%GG", "%+G?", "% G?", "%-G?",
        ] {
            for option in ["--format", "--pretty", "--form"] {
                for args in [
                    vec![format!("{option}={format}")],
                    vec![option.into(), format.into()],
                ] {
                    assert!(matches!(
                        repo.runner
                            .run(ReadCommand::new(op).args(args), &repo.options)
                            .await,
                        Err(GitError::InvalidCommand(_))
                    ));
                }
            }
        }
        assert!(matches!(
            repo.runner
                .run(
                    ReadCommand::new(op).args(["--pretty=repository-alias"]),
                    &repo.options
                )
                .await,
            Err(GitError::InvalidCommand(_))
        ));
    }
    // Signature-like paths after -- remain literal paths, and normal formats work.
    repo.read(ReadOperation::Log, &["--format=%H", "--", "%G?"])
        .await;
    assert!(!repo
        .read(ReadOperation::Log, &["--format=%H"])
        .await
        .is_empty());
}

#[cfg(unix)]
#[tokio::test]
async fn filter_scope_includes_and_configuration_changes_are_respected() {
    let mut repo = Repo::new().await;
    repo.initial().await;
    let helper = repo.path("trusted-filter");
    support::script(&helper, "touch pwned-scope\ncat");
    let config = format!("[filter \"scope\"]\nclean = {}\n", helper.display());
    repo.put("global-config", &config);
    repo.options.env.insert(
        "GIT_CONFIG_GLOBAL".into(),
        repo.path("global-config").into_os_string(),
    );
    repo.put(".gitattributes", "B filter=scope\n");
    repo.put("B", "changed worktree\n");
    repo.read(ReadOperation::Diff, &[]).await;
    assert!(
        repo.path("pwned-scope").exists(),
        "global filter must remain trusted"
    );
    std::fs::remove_file(repo.path("pwned-scope")).unwrap();
    // Even a local non-command property shadows the whole global driver.
    repo.git("config", &["filter.scope.required", "true"]).await;
    repo.read(ReadOperation::Diff, &[]).await;
    assert!(!repo.path("pwned-scope").exists());
    repo.git("config", &["--remove-section", "filter.scope"])
        .await;
    repo.read(ReadOperation::Diff, &[]).await;
    assert!(
        repo.path("pwned-scope").exists(),
        "config changes must take effect without a stale cache"
    );
    std::fs::remove_file(repo.path("pwned-scope")).unwrap();
    repo.put(".git/included-config", &config);
    let gitdir = std::fs::canonicalize(repo.path(".git")).unwrap();
    for key in [
        "include.path".to_owned(),
        format!("includeIf.gitdir:{}.path", gitdir.display()),
    ] {
        repo.git("config", &[&key, "included-config"]).await;
        repo.read(ReadOperation::Diff, &[]).await;
        assert!(
            !repo.path("pwned-scope").exists(),
            "local include escaped scope: {key}"
        );
        repo.git("config", &["--unset", &key]).await;
    }
}

#[cfg(unix)]
#[tokio::test]
async fn worktree_local_filters_are_disabled() {
    let mut repo = Repo::new().await;
    repo.initial().await;
    repo.git("config", &["extensions.worktreeConfig", "true"])
        .await;
    let linked = repo.path("linked");
    repo.git(
        "worktree",
        &["add", "-q", "-b", "linked", linked.to_str().unwrap()],
    )
    .await;
    let helper = repo.path("worktree-filter");
    let sentinel = repo.path("pwned-worktree");
    support::script(&helper, &format!("touch '{}'\ncat", sentinel.display()));
    repo.options.cwd = linked.clone();
    repo.git(
        "config",
        &[
            "--worktree",
            "filter.only-here.clean",
            helper.to_str().unwrap(),
        ],
    )
    .await;
    std::fs::write(linked.join(".gitattributes"), "B filter=only-here\n").unwrap();
    std::fs::write(linked.join("B"), "changed worktree\n").unwrap();
    repo.git("diff", &[]).await;
    assert!(sentinel.exists());
    std::fs::remove_file(&sentinel).unwrap();
    repo.read(ReadOperation::Diff, &[]).await;
    assert!(!sentinel.exists());
}

#[cfg(unix)]
#[tokio::test]
async fn trusted_system_and_command_filters_remain_enabled() {
    let mut repo = Repo::new().await;
    repo.initial().await;
    let helper = repo.path("trusted-filter");
    support::script(&helper, "touch pwned-trusted\ncat");
    repo.put(".gitattributes", "B filter=trusted\n");
    repo.put("B", "changed worktree\n");
    repo.put(
        "system-config",
        &format!("[filter \"trusted\"]\nclean = {}\n", helper.display()),
    );
    repo.options
        .env
        .remove(std::ffi::OsStr::new("GIT_CONFIG_NOSYSTEM"));
    repo.options.env.insert(
        "GIT_CONFIG_SYSTEM".into(),
        repo.path("system-config").into_os_string(),
    );
    repo.read(ReadOperation::Diff, &[]).await;
    assert!(repo.path("pwned-trusted").exists());
    std::fs::remove_file(repo.path("pwned-trusted")).unwrap();
    repo.options
        .env
        .insert("GIT_CONFIG_NOSYSTEM".into(), "1".into());
    repo.options
        .env
        .insert("GIT_CONFIG_COUNT".into(), "1".into());
    repo.options
        .env
        .insert("GIT_CONFIG_KEY_0".into(), "filter.trusted.clean".into());
    repo.options
        .env
        .insert("GIT_CONFIG_VALUE_0".into(), helper.into_os_string());
    repo.read(ReadOperation::Diff, &[]).await;
    assert!(repo.path("pwned-trusted").exists());
}

#[cfg(unix)]
#[tokio::test]
async fn alternate_refs_helper_is_blocked_before_execution() {
    let repo = Repo::new().await;
    let alternate = Repo::new().await;
    alternate.initial().await;
    let hash = String::from_utf8(alternate.read(ReadOperation::RevParse, &["HEAD"]).await).unwrap();
    std::fs::create_dir_all(repo.path(".git/objects/info")).unwrap();
    repo.put(
        ".git/objects/info/alternates",
        &format!("{}\n", alternate.path(".git/objects").display()),
    );
    let helper = repo.path("alternate-refs");
    support::script(
        &helper,
        &format!("touch pwned-alternates\necho {}", hash.trim()),
    );
    repo.git(
        "config",
        &["core.alternateRefsCommand", helper.to_str().unwrap()],
    )
    .await;
    repo.git("log", &["--alternate-refs", "--oneline"]).await;
    assert!(repo.path("pwned-alternates").exists());
    std::fs::remove_file(repo.path("pwned-alternates")).unwrap();
    assert!(matches!(
        repo.runner
            .run(
                ReadCommand::new(ReadOperation::Log).args(["--alternate-refs"]),
                &repo.options
            )
            .await,
        Err(GitError::InvalidCommand(_))
    ));
    assert!(!repo.path("pwned-alternates").exists());
}

#[cfg(unix)]
#[tokio::test]
async fn remerge_diff_cannot_execute_repository_merge_driver() {
    let repo = Repo::new().await;
    repo.initial().await;
    let branch = String::from_utf8(repo.git("branch", &["--show-current"]).await).unwrap();
    repo.git("checkout", &["-qb", "side"]).await;
    repo.put("B", "side change\n");
    repo.git("add", &["B"]).await;
    repo.git("commit", &["-qm", "side"]).await;
    repo.git("checkout", &["-q", branch.trim()]).await;
    repo.put("B", "main change\n");
    repo.git("add", &["B"]).await;
    repo.git("commit", &["-qm", "main"]).await;
    assert!(repo
        .runner
        .run(
            gitwood_core::git::WriteCommand::new("merge").args(["--no-ff", "side"]),
            &repo.options
        )
        .await
        .is_err());
    repo.put("B", "resolved\n");
    repo.git("add", &["B"]).await;
    repo.git("commit", &["-qm", "resolved merge"]).await;
    let helper = repo.path("evil-merge");
    support::script(&helper, "touch pwned-merge\nexit 1");
    std::fs::create_dir_all(repo.path(".git/info")).unwrap();
    repo.put(".git/info/attributes", "B merge=evil\n");
    repo.git("config", &["merge.evil.driver", helper.to_str().unwrap()])
        .await;
    repo.git("log", &["--remerge-diff", "-1"]).await;
    assert!(repo.path("pwned-merge").exists());
    std::fs::remove_file(repo.path("pwned-merge")).unwrap();
    repo.git("config", &["log.diffMerges", "remerge"]).await;
    for op in [ReadOperation::Log, ReadOperation::Show] {
        repo.read(op, &["-1", "-p", "-m"]).await;
        for option in [
            "--remerge-diff",
            "--diff-merges=remerge",
            "--diff-merges=r",
            "--diff-merges=on",
        ] {
            assert!(matches!(
                repo.runner
                    .run(ReadCommand::new(op).args([option]), &repo.options)
                    .await,
                Err(GitError::InvalidCommand(_))
            ));
        }
    }
    assert!(!repo.path("pwned-merge").exists());
}

#[cfg(unix)]
#[tokio::test]
async fn parent_reads_do_not_enter_submodule_filter_configuration() {
    let repo = Repo::new().await;
    let child = Repo::new().await;
    child.initial().await;
    repo.git("clone", &["-q", child.dir.path().to_str().unwrap(), "sub"])
        .await;
    repo.put(
        ".gitmodules",
        "[submodule \"sub\"]\npath = sub\nurl = ./unused\n",
    );
    repo.git("add", &[".gitmodules", "sub"]).await;
    repo.git("commit", &["-qm", "submodule"]).await;
    let helper = repo.path("submodule-filter");
    let sentinel = repo.path("pwned-submodule");
    support::script(&helper, &format!("touch '{}'\ncat", sentinel.display()));
    let mut sub_options = repo.options.clone();
    sub_options.cwd = repo.path("sub");
    repo.runner
        .run(
            gitwood_core::git::WriteCommand::new("config")
                .args(["filter.child-only.clean", helper.to_str().unwrap()]),
            &sub_options,
        )
        .await
        .unwrap();
    repo.put("sub/.gitattributes", "B filter=child-only\n");
    repo.put("sub/B", "modified inside child\n");
    repo.git("config", &["diff.submodule", "diff"]).await;
    repo.git("config", &["status.submoduleSummary", "true"])
        .await;
    repo.git("diff", &["--submodule=diff"]).await;
    assert!(
        sentinel.exists(),
        "native inline submodule diff must trigger fixture"
    );
    std::fs::remove_file(&sentinel).unwrap();
    for (op, args) in [
        (ReadOperation::Status, vec!["--porcelain=v2"]),
        (ReadOperation::Diff, vec!["--submodule=diff"]),
        (ReadOperation::Log, vec!["-p", "--submodule=diff"]),
        (ReadOperation::Show, vec!["HEAD", "--submodule=diff"]),
    ] {
        repo.read(op, &args).await;
        assert!(!sentinel.exists(), "submodule filter via {op:?}");
    }
}

#[cfg(unix)]
#[tokio::test]
async fn missing_promisor_objects_cannot_run_repository_ssh_command() {
    let repo = Repo::new().await;
    repo.initial().await;
    let blob = String::from_utf8(repo.read(ReadOperation::RevParse, &["HEAD:B"]).await).unwrap();
    let blob = blob.trim();
    std::fs::remove_file(repo.path(&format!(".git/objects/{}/{}", &blob[..2], &blob[2..])))
        .unwrap();
    let helper = repo.path("evil-ssh");
    support::script(
        &helper,
        &format!(
            "touch '{}'\nexit 1",
            repo.path("pwned-lazy-fetch").display()
        ),
    );
    for (key, value) in [
        ("core.sshCommand", helper.to_str().unwrap()),
        ("extensions.partialClone", "origin"),
        ("remote.origin.promisor", "true"),
        ("remote.origin.url", "ssh://example.invalid/repository"),
    ] {
        repo.git("config", &[key, value]).await;
    }
    let result = repo
        .runner
        .run(
            ReadCommand::new(ReadOperation::Show).args(["HEAD:B"]),
            &repo.options,
        )
        .await;
    assert!(matches!(result, Err(GitError::Exit { .. })));
    assert!(!repo.path("pwned-lazy-fetch").exists());
}

#[cfg(unix)]
#[tokio::test]
async fn local_clean_and_process_filters_are_disabled_for_reads() {
    for kind in ["clean", "process"] {
        let repo = Repo::new().await;
        repo.initial().await;
        let helper = repo.path("evil-filter");
        support::script(&helper, "touch pwned-filter\nexit 1");
        repo.git(
            "config",
            &[&format!("filter.Odd.Name.{kind}"), helper.to_str().unwrap()],
        )
        .await;
        std::fs::create_dir_all(repo.path(".git/info")).unwrap();
        repo.put(".git/info/attributes", "B filter=Odd.Name\n");
        repo.put("B", "changed and different length\n");
        // Positive control: native Git actually invokes this fixture.
        let mut control = repo.options.clone();
        control.timeout = Some(std::time::Duration::from_secs(2));
        let _ = repo
            .runner
            .run(
                gitwood_core::git::WriteCommand::new("diff")
                    .args(["--no-ext-diff", "--no-textconv"]),
                &control,
            )
            .await;
        assert!(repo.path("pwned-filter").exists());
        std::fs::remove_file(repo.path("pwned-filter")).unwrap();
        repo.git("config", &["filter.Odd.Name.required", "true"])
            .await;
        for (op, args) in [
            (ReadOperation::Status, vec!["--porcelain=v2"]),
            (ReadOperation::Diff, vec![]),
            (ReadOperation::Log, vec!["-p", "-1"]),
            (ReadOperation::Show, vec!["HEAD"]),
            (ReadOperation::DiffTree, vec!["-p", "HEAD"]),
        ] {
            repo.read(op, &args).await;
            assert!(!repo.path("pwned-filter").exists(), "{kind} via {op:?}");
        }
    }
}

#[cfg(unix)]
#[tokio::test]
async fn signed_log_and_show_do_not_execute_gpg() {
    let mut repo = Repo::new().await;
    repo.initial().await;
    // Git parses the signature header before invoking the verifier; no real key needed.
    let tree =
        String::from_utf8(repo.read(ReadOperation::RevParse, &["HEAD^{tree}"]).await).unwrap();
    repo.options.stdin = format!("tree {}\nauthor Test <test@example.invalid> 1700000000 +0000\ncommitter Test <test@example.invalid> 1700000000 +0000\ngpgsig -----BEGIN PGP SIGNATURE-----\n fake\n -----END PGP SIGNATURE-----\n\nsigned fixture\n", tree.trim()).into_bytes();
    let hash = String::from_utf8(
        repo.git("hash-object", &["-t", "commit", "-w", "--stdin"])
            .await,
    )
    .unwrap();
    repo.options.stdin.clear();
    repo.git("update-ref", &["HEAD", hash.trim()]).await;
    let helper = repo.path("evil-gpg");
    support::script(&helper, "touch pwned-gpg\nexit 1");
    repo.git("config", &["gpg.program", helper.to_str().unwrap()])
        .await;
    repo.git("config", &["log.showSignature", "true"]).await;
    for op in [ReadOperation::Log, ReadOperation::Show] {
        let name = if matches!(op, ReadOperation::Log) {
            "log"
        } else {
            "show"
        };
        repo.git(name, &["-1", "HEAD"]).await;
        assert!(repo.path("pwned-gpg").exists());
        std::fs::remove_file(repo.path("pwned-gpg")).unwrap();
        repo.read(op, &["-1", "HEAD"]).await;
        assert!(!repo.path("pwned-gpg").exists());
    }
    repo.git("config", &["log.showSignature", "false"]).await;
    repo.git("config", &["pretty.repository-format", "format:%G?"])
        .await;
    for format in ["%G?", "repository-format"] {
        repo.git("config", &["format.pretty", format]).await;
        repo.git("log", &["-1"]).await;
        assert!(
            repo.path("pwned-gpg").exists(),
            "format fixture must execute verifier"
        );
        std::fs::remove_file(repo.path("pwned-gpg")).unwrap();
        for op in [ReadOperation::Log, ReadOperation::Show] {
            repo.read(op, &["-1"]).await;
            assert!(!repo.path("pwned-gpg").exists());
        }
    }
}

#[cfg(unix)]
#[tokio::test]
async fn history_read_blocks_local_filters_and_signature_verification() {
    let mut repo = Repo::new().await;
    repo.initial().await;
    let tree =
        String::from_utf8(repo.read(ReadOperation::RevParse, &["HEAD^{tree}"]).await).unwrap();
    repo.options.stdin = format!("tree {}\nauthor Test <test@example.invalid> 1700000000 +0000\ncommitter Test <test@example.invalid> 1700000000 +0000\ngpgsig -----BEGIN PGP SIGNATURE-----\n fake\n -----END PGP SIGNATURE-----\n\nsigned fixture\n", tree.trim()).into_bytes();
    let hash = String::from_utf8(
        repo.git("hash-object", &["-t", "commit", "-w", "--stdin"])
            .await,
    )
    .unwrap();
    repo.options.stdin.clear();
    repo.git("update-ref", &["HEAD", hash.trim()]).await;
    let helper = repo.path("history-helper");
    support::script(&helper, "touch pwned-history\nexit 1");
    for key in ["filter.evil.clean", "filter.evil.process", "gpg.program"] {
        repo.git("config", &[key, helper.to_str().unwrap()]).await;
    }
    repo.git("config", &["log.showSignature", "true"]).await;
    repo.put(".gitattributes", "B filter=evil\n");
    repo.put("B", "modified\n");
    // Positive controls prove both repository helpers are executable.
    let _ = repo
        .runner
        .run(gitwood_core::git::WriteCommand::new("diff"), &repo.options)
        .await;
    assert!(repo.path("pwned-history").exists());
    std::fs::remove_file(repo.path("pwned-history")).unwrap();
    repo.git("log", &["-1"]).await;
    assert!(repo.path("pwned-history").exists());
    std::fs::remove_file(repo.path("pwned-history")).unwrap();
    let graph = gitwood_core::history::read::read_graph(&repo.runner, &repo.options)
        .await
        .unwrap();
    assert_eq!(graph.len(), 1);
    assert!(!repo.path("pwned-history").exists());
}

#[cfg(unix)]
#[tokio::test]
async fn for_each_ref_cannot_execute_repository_helpers() {
    let r = Repo::new().await;
    r.initial().await;
    let helper = r.path("evil-ref-helper");
    support::script(&helper, "touch pwned-refs\nexit 1");
    for key in [
        "core.fsmonitor",
        "core.pager",
        "core.alternateRefsCommand",
        "gpg.program",
        "diff.external",
        "filter.evil.clean",
    ] {
        r.git("config", &[key, helper.to_str().unwrap()]).await;
    }
    r.git("config", &["log.showSignature", "true"]).await;
    let refs = gitwood_core::repo::refs::read(&r.runner, &r.options)
        .await
        .unwrap();
    assert_eq!(refs.local.len(), 1);
    assert!(!r.path("pwned-refs").exists());
}

#[tokio::test]
async fn ref_signature_verification_formats_are_rejected() {
    let r = Repo::new().await;
    for args in [
        vec!["--format=%(signature:grade)"],
        vec!["--format", "%(signature:signer)"],
    ] {
        assert!(matches!(
            r.runner
                .run(
                    ReadCommand::new(ReadOperation::ForEachRef).args(args),
                    &r.options
                )
                .await,
            Err(GitError::InvalidCommand(_))
        ));
    }
}

#[cfg(unix)]
#[tokio::test]
async fn diff_blob_reads_cannot_execute_repository_helpers() {
    use gitwood_core::diff::{read_content, Content, ContentSource, ReadLimits};
    let repo = Repo::new().await;
    repo.initial().await;
    let helper = repo.path("blob-helper");
    support::script(&helper, "touch pwned-blob\nexit 1");
    repo.put(".gitattributes", "B diff=evil filter=evil\n");
    for key in [
        "core.fsmonitor",
        "core.pager",
        "diff.external",
        "diff.evil.textconv",
        "filter.evil.process",
        "core.alternateRefsCommand",
    ] {
        repo.git("config", &[key, helper.to_str().unwrap()]).await;
    }
    for source in [
        ContentSource::Commit {
            revision: "HEAD",
            path: std::path::Path::new("B"),
        },
        ContentSource::Index {
            path: std::path::Path::new("B"),
        },
    ] {
        assert!(
            matches!(read_content(&repo.runner, &repo.options, source, ReadLimits::default()).await.unwrap(), Content::Text {text, ..} if text == "B base\n")
        );
    }
    assert!(!repo.path("pwned-blob").exists());
    assert!(matches!(
        repo.runner
            .run(
                ReadCommand::new(ReadOperation::CatFile).args(["--filters", "HEAD:B"]),
                &repo.options
            )
            .await,
        Err(GitError::InvalidCommand(_))
    ));
}

mod support;
use gitwood_core::{
    git::{ReadOperation, WriteCommand},
    ops::{
        conflict::{self, Side},
        sequence::{self, SequenceAction, SequenceOutcome},
        snapshot,
    },
    repo::{self, RepoPath},
};
use support::Repo;

async fn token(r: &Repo) -> String {
    snapshot(&r.runner, &r.options).await.unwrap().fingerprint
}
async fn commit(r: &Repo, text: &[u8], message: &str) {
    std::fs::write(r.path("A"), text).unwrap();
    r.git("add", &["A"]).await;
    r.git("commit", &["-qm", message]).await;
}
async fn stop(r: &Repo, name: &str, args: &[&str]) {
    assert!(r
        .runner
        .run(
            WriteCommand::new(name).args(args.iter().copied()),
            &r.options
        )
        .await
        .is_err());
}
async fn conflict(base: &[u8], local: &[u8], incoming: &[u8]) -> Repo {
    let r = Repo::new().await;
    commit(&r, base, "base").await;
    r.git("branch", &["-M", "main"]).await;
    r.git("checkout", &["-qb", "topic"]).await;
    commit(&r, incoming, "incoming").await;
    r.git("checkout", &["main"]).await;
    commit(&r, local, "local").await;
    stop(&r, "merge", &["topic"]).await;
    r
}
fn a() -> RepoPath {
    RepoPath::new(b"A")
}
async fn accept(r: &Repo, side: Side) {
    conflict::resolve(
        &r.runner,
        &r.options,
        &a(),
        &token(r).await,
        Some(side),
        None,
    )
    .await
    .unwrap();
    assert!(repo::status::read(&r.runner, &r.options)
        .await
        .unwrap()
        .conflicts
        .is_empty());
}
#[tokio::test]
async fn text_merge_save_and_merge_commit_preserve_both_parents_and_auto_changes() {
    let r = conflict(
        b"start\nbase\nend\n",
        b"start\nlocal\nend\n",
        b"start\nincoming\nend\n",
    )
    .await;
    assert!(r.commit(&[], vec![], false).await.is_err());
    let content = conflict::read(&r.runner, &r.options, &a()).await.unwrap();
    assert_eq!(content.blocks.len(), 1);
    assert_eq!(content.left, "start\nlocal\nend\n");
    assert_eq!(content.right, "start\nincoming\nend\n");
    conflict::resolve(
        &r.runner,
        &r.options,
        &a(),
        &content.fingerprint,
        None,
        Some("start\nlocal\nincoming\nend"),
    )
    .await
    .unwrap();
    assert_eq!(
        std::fs::read(r.path("A")).unwrap(),
        b"start\nlocal\nincoming\nend"
    );
    r.commit(&["A"], vec![], false).await.unwrap();
    let parents = r.read(ReadOperation::RevParse, &["HEAD^1", "HEAD^2"]).await;
    assert_eq!(
        parents
            .split(|b| *b == b'\n')
            .filter(|s| !s.is_empty())
            .count(),
        2
    );
    assert!(r
        .read(ReadOperation::Status, &["--porcelain"])
        .await
        .is_empty());
}
#[tokio::test]
async fn accept_sides_and_external_changes_are_rejected_without_overwriting() {
    for side in [Side::Yours, Side::Theirs] {
        let r = conflict(b"base\n", b"ours\n", b"theirs\n").await;
        let content = conflict::read(&r.runner, &r.options, &a()).await.unwrap();
        r.put("A", "external\n");
        assert!(conflict::resolve(
            &r.runner,
            &r.options,
            &a(),
            &content.fingerprint,
            None,
            Some("result")
        )
        .await
        .is_err());
        assert_eq!(std::fs::read(r.path("A")).unwrap(), b"external\n");
        accept(&r, side).await;
        assert_eq!(
            std::fs::read(r.path("A")).unwrap(),
            if matches!(side, Side::Yours) {
                b"ours\n".as_slice()
            } else {
                b"theirs\n".as_slice()
            }
        );
    }
}
#[tokio::test]
async fn add_add_binary_and_modify_delete_conflicts() {
    for side in [Side::Yours, Side::Theirs] {
        let r = Repo::new().await;
        r.initial().await;
        r.git("branch", &["-M", "main"]).await;
        r.git("checkout", &["-qb", "topic"]).await;
        r.git("rm", &["A"]).await;
        r.git("commit", &["-qm", "delete"]).await;
        r.git("checkout", &["main"]).await;
        commit(&r, b"modified\n", "modify").await;
        stop(&r, "merge", &["topic"]).await;
        let status = repo::status::read(&r.runner, &r.options).await.unwrap();
        assert_eq!(status.conflicts[0].xy, "UD");
        accept(&r, side).await;
        assert_eq!(r.path("A").exists(), matches!(side, Side::Yours));
    }
    let r = Repo::new().await;
    r.initial().await;
    r.git("branch", &["-M", "main"]).await;
    r.git("checkout", &["-qb", "topic"]).await;
    r.put("new", "incoming");
    r.git("add", &["new"]).await;
    r.git("commit", &["-qm", "incoming"]).await;
    r.git("checkout", &["main"]).await;
    r.put("new", "ours");
    r.git("add", &["new"]).await;
    r.git("commit", &["-qm", "ours"]).await;
    stop(&r, "merge", &["topic"]).await;
    assert_eq!(
        repo::status::read(&r.runner, &r.options)
            .await
            .unwrap()
            .conflicts[0]
            .xy,
        "AA"
    );
    let content = conflict::read(&r.runner, &r.options, &RepoPath::new(b"new"))
        .await
        .unwrap();
    assert_eq!(content.blocks.len(), 1);
    let r = conflict(b"base\0", b"ours\0", b"theirs\0").await;
    assert_eq!(
        conflict::read(&r.runner, &r.options, &a())
            .await
            .unwrap()
            .read_only,
        Some("binaryOrLarge")
    );
    let files = conflict::files(
        &r.runner,
        &r.options,
        &repo::status::read(&r.runner, &r.options)
            .await
            .unwrap()
            .conflicts,
    )
    .await
    .unwrap();
    assert!(files[0].binary);
    accept(&r, Side::Theirs).await;
    assert_eq!(std::fs::read(r.path("A")).unwrap(), b"theirs\0");
}
#[tokio::test]
async fn rebase_reverses_sides_and_continues_through_two_conflicts_then_aborts_exactly() {
    let r = Repo::new().await;
    commit(&r, b"base\n", "base").await;
    r.git("branch", &["-M", "main"]).await;
    r.git("checkout", &["-qb", "topic"]).await;
    commit(&r, b"one\n", "one").await;
    commit(&r, b"two\n", "two").await;
    let original = r.read(ReadOperation::RevParse, &["HEAD"]).await;
    r.git("checkout", &["main"]).await;
    commit(&r, b"upstream\n", "upstream").await;
    r.git("checkout", &["topic"]).await;
    stop(&r, "rebase", &["main"]).await;
    let content = conflict::read(&r.runner, &r.options, &a()).await.unwrap();
    assert_eq!(content.left, "one\n");
    assert_eq!(content.right, "upstream\n");
    accept(&r, Side::Theirs).await;
    let next = sequence::run(
        &r.runner,
        &r.options,
        &token(&r).await,
        SequenceAction::Continue,
        None,
    )
    .await
    .unwrap();
    assert!(matches!(next, SequenceOutcome::Conflicts { .. }));
    accept(&r, Side::Yours).await;
    sequence::run(
        &r.runner,
        &r.options,
        &token(&r).await,
        SequenceAction::Abort,
        None,
    )
    .await
    .unwrap();
    assert_eq!(r.read(ReadOperation::RevParse, &["HEAD"]).await, original);
    assert_eq!(std::fs::read(r.path("A")).unwrap(), b"two\n");
    assert!(r
        .read(ReadOperation::Status, &["--porcelain"])
        .await
        .is_empty());
    stop(&r, "rebase", &["main"]).await;
    accept(&r, Side::Theirs).await;
    sequence::run(
        &r.runner,
        &r.options,
        &token(&r).await,
        SequenceAction::Continue,
        None,
    )
    .await
    .unwrap();
    accept(&r, Side::Yours).await;
    assert!(matches!(
        sequence::run(
            &r.runner,
            &r.options,
            &token(&r).await,
            SequenceAction::Continue,
            None
        )
        .await
        .unwrap(),
        SequenceOutcome::Completed
    ));
}
#[tokio::test]
async fn crlf_bom_and_missing_final_newline_survive_save() {
    for (base, left, right, result) in [
        (
            b"base\r\n".as_slice(),
            b"ours\r\n".as_slice(),
            b"theirs\r\n".as_slice(),
            b"resolved\r\n".as_slice(),
        ),
        (
            b"\xef\xbb\xbfbase".as_slice(),
            b"\xef\xbb\xbfours".as_slice(),
            b"\xef\xbb\xbftheirs".as_slice(),
            b"\xef\xbb\xbfresolved".as_slice(),
        ),
    ] {
        let r = conflict(base, left, right).await;
        let text = if result.ends_with(b"\r\n") {
            "resolved\r\n"
        } else {
            "resolved"
        };
        conflict::resolve(
            &r.runner,
            &r.options,
            &a(),
            &token(&r).await,
            None,
            Some(text),
        )
        .await
        .unwrap();
        assert_eq!(std::fs::read(r.path("A")).unwrap(), result);
    }
}

#[tokio::test]
async fn operation_commit_requires_auto_merged_files_and_preserves_their_contents() {
    let r = Repo::new().await;
    r.initial().await;
    r.git("branch", &["-M", "main"]).await;
    r.git("checkout", &["-qb", "topic"]).await;
    commit(&r, b"incoming\n", "incoming").await;
    r.put("new", "incoming file\n");
    r.git("add", &["new"]).await;
    r.git("commit", &["-qm", "new file"]).await;
    r.git("checkout", &["main"]).await;
    commit(&r, b"local\n", "local").await;
    stop(&r, "merge", &["topic"]).await;
    accept(&r, Side::Yours).await;
    assert!(r.commit(&["A"], vec![], false).await.is_err());
    r.commit(&["A", "new"], vec![], false).await.unwrap();
    assert_eq!(
        r.read(ReadOperation::Show, &["HEAD:new"]).await,
        b"incoming file\n"
    );
    assert!(!r
        .read(ReadOperation::RevParse, &["HEAD^2"])
        .await
        .is_empty());
}
#[tokio::test]
async fn cherry_pick_and_revert_commit_panel_paths_complete_the_operation() {
    for revert in [false, true] {
        let r = Repo::new().await;
        r.initial().await;
        r.git("branch", &["-M", "main"]).await;
        commit(&r, b"one\n", "one").await;
        let one = String::from_utf8(r.read(ReadOperation::RevParse, &["HEAD"]).await).unwrap();
        commit(&r, b"two\n", "two").await;
        if revert {
            stop(&r, "revert", &[one.trim()]).await;
        } else {
            r.git("checkout", &["-b", "topic", "HEAD~2"]).await;
            commit(&r, b"topic\n", "topic").await;
            stop(&r, "cherry-pick", &[one.trim()]).await;
        }
        assert!(r.path(".git/MERGE_MSG").exists());
        assert!(r.commit(&[], vec![], false).await.is_err());
        accept(&r, Side::Yours).await;
        r.put("A", "reviewed\n");
        r.git("add", &["A"]).await;
        assert!(r.commit(&["A"], vec![], true).await.is_err());
        r.commit(&["A"], vec![], false).await.unwrap();
        assert!(gitwood_core::ops::operation_state(&r.runner, &r.options)
            .await
            .unwrap()
            .is_none());
        assert!(r
            .read(ReadOperation::Status, &["--porcelain"])
            .await
            .is_empty());
    }
}
#[tokio::test]
async fn index_lock_and_large_file_do_not_allow_text_save() {
    let r = conflict(b"base", b"local", b"incoming").await;
    let expected = token(&r).await;
    let original = std::fs::read(r.path("A")).unwrap();
    r.put(".git/index.lock", "external lock");
    assert!(conflict::resolve(
        &r.runner,
        &r.options,
        &a(),
        &expected,
        Some(Side::Yours),
        None
    )
    .await
    .is_err());
    assert_eq!(std::fs::read(r.path("A")).unwrap(), original);
    std::fs::remove_file(r.path(".git/index.lock")).unwrap();
    std::fs::write(r.path("A"), vec![b'x'; 1024 * 1024 + 1]).unwrap();
    let content = conflict::read(&r.runner, &r.options, &a()).await.unwrap();
    assert_eq!(content.read_only, Some("binaryOrLarge"));
    assert!(conflict::resolve(
        &r.runner,
        &r.options,
        &a(),
        &content.fingerprint,
        None,
        Some("no")
    )
    .await
    .is_err());
}

#[tokio::test]
async fn utf16_text_merge_preserves_bom_endianness_and_no_final_newline() {
    for le in [true, false] {
        let encode = |text: &str| {
            let mut bytes = if le {
                vec![0xff, 0xfe]
            } else {
                vec![0xfe, 0xff]
            };
            for unit in text.encode_utf16() {
                bytes.extend(if le {
                    unit.to_le_bytes()
                } else {
                    unit.to_be_bytes()
                });
            }
            bytes
        };
        let r = conflict(
            &encode("base\r\n中文"),
            &encode("ours\r\n中文"),
            &encode("theirs\r\n中文"),
        )
        .await;
        let content = conflict::read(&r.runner, &r.options, &a()).await.unwrap();
        assert!(content.read_only.is_none());
        conflict::resolve(
            &r.runner,
            &r.options,
            &a(),
            &content.fingerprint,
            None,
            Some("resolved\r\n中文"),
        )
        .await
        .unwrap();
        assert_eq!(
            std::fs::read(r.path("A")).unwrap(),
            encode("resolved\r\n中文")
        );
    }
}
#[tokio::test]
async fn accepting_a_gitlink_uses_selected_stage_without_touching_nested_worktree() {
    let r = Repo::new().await;
    r.initial().await;
    r.git("branch", &["-M", "main"]).await;
    let base = String::from_utf8(r.read(ReadOperation::RevParse, &["HEAD"]).await).unwrap();
    r.git(
        "update-index",
        &["--add", "--cacheinfo", "160000", base.trim(), "module"],
    )
    .await;
    r.git("commit", &["-qm", "base link"]).await;
    r.git("checkout", &["-qb", "topic"]).await;
    commit(&r, b"incoming", "incoming").await;
    let incoming = String::from_utf8(r.read(ReadOperation::RevParse, &["HEAD"]).await).unwrap();
    r.git(
        "update-index",
        &["--cacheinfo", "160000", incoming.trim(), "module"],
    )
    .await;
    r.git("commit", &["-qm", "incoming link"]).await;
    r.git("checkout", &["main"]).await;
    commit(&r, b"ours", "ours").await;
    let ours = String::from_utf8(r.read(ReadOperation::RevParse, &["HEAD"]).await).unwrap();
    r.git(
        "update-index",
        &["--cacheinfo", "160000", ours.trim(), "module"],
    )
    .await;
    r.git("commit", &["-qm", "ours link"]).await;
    stop(&r, "merge", &["topic"]).await;
    let entries = repo::status::read(&r.runner, &r.options)
        .await
        .unwrap()
        .conflicts;
    let files = conflict::files(&r.runner, &r.options, &entries)
        .await
        .unwrap();
    assert!(files.iter().any(|f| f.submodule));
    std::fs::create_dir_all(r.path("module")).unwrap();
    r.put("module/keep", "nested data");
    conflict::resolve(
        &r.runner,
        &r.options,
        &RepoPath::new(b"module"),
        &token(&r).await,
        Some(Side::Theirs),
        None,
    )
    .await
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(r.path("module/keep")).unwrap(),
        "nested data"
    );
    let cached = r.read(ReadOperation::LsFiles, &["--stage", "module"]).await;
    assert!(String::from_utf8(cached).unwrap().contains(incoming.trim()));
}

#[tokio::test]
async fn conflict_list_batches_all_blob_reads_and_preserves_file_flags() {
    let r = Repo::new().await;
    r.initial().await;
    let data = [
        b"text\n".to_vec(),
        b"binary\0\nbody".to_vec(),
        vec![b'x'; 1024 * 1024 + 1],
        vec![0xff, 0xfe, b'a', 0],
    ];
    let mut oids = Vec::new();
    for (i, bytes) in data.iter().enumerate() {
        let path = format!("blob{i}");
        std::fs::write(r.path(&path), bytes).unwrap();
        oids.push(
            String::from_utf8(r.git("hash-object", &["-w", "--", &path]).await)
                .unwrap()
                .trim()
                .to_owned(),
        );
    }
    let mut input = String::new();
    for i in 0..30 {
        let stages = match i % 5 {
            0 => vec![(1, 0), (2, 0), (3, 0)],
            1 => vec![(1, 1), (2, 0), (3, 1)],
            2 => vec![(2, 2), (3, 0)],
            3 => vec![(2, 3), (3, 0)],
            _ => vec![(1, 0)],
        };
        for (stage, blob) in stages {
            input.push_str(&format!("100644 {} {stage}\tconflict{i}\n", oids[blob]));
        }
    }
    let head = String::from_utf8(r.read(ReadOperation::RevParse, &["HEAD"]).await).unwrap();
    for stage in 1..=3 {
        input.push_str(&format!("160000 {} {stage}\tmodule\n", head.trim()));
    }
    let mut options = r.options.clone();
    options.stdin = input.into_bytes();
    r.runner
        .run(
            WriteCommand::new("update-index").args(["--index-info"]),
            &options,
        )
        .await
        .unwrap();
    let entries = repo::status::read(&r.runner, &r.options)
        .await
        .unwrap()
        .conflicts;
    assert_eq!(entries.len(), 31);
    let log = tempfile::NamedTempFile::new().unwrap();
    let mut traced = r.options.clone();
    traced.env.insert("GIT_TRACE".into(), log.path().into());
    let files = conflict::files(&r.runner, &traced, &entries).await.unwrap();
    for file in &files {
        if file.path.display == "module" {
            assert!(file.submodule);
            continue;
        }
        let i = file
            .path
            .display
            .strip_prefix("conflict")
            .unwrap()
            .parse::<usize>()
            .unwrap();
        assert_eq!(file.binary, i % 5 == 1);
        assert_eq!(file.large, i % 5 == 2);
        assert!(!file.submodule);
        assert_eq!(
            file.conflict_type,
            match i % 5 {
                0 | 1 => "UU",
                2 | 3 => "AA",
                _ => "DD",
            }
        );
    }
    let trace = std::fs::read_to_string(log.path()).unwrap();
    let reads = trace
        .lines()
        .filter(|line| {
            line.contains("built-in: git")
                && (line.contains(" cat-file ") || line.contains(" show "))
        })
        .count();
    eprintln!("31 conflicts: {reads} blob-reading Git processes");
    assert_eq!(reads, 2);
    assert_eq!(
        trace
            .lines()
            .filter(|line| line.contains("built-in: git"))
            .count(),
        4
    );
    assert_eq!(trace.matches("--batch-check").count(), 1);
    assert_eq!(
        trace
            .lines()
            .filter(|line| line.ends_with("cat-file --batch"))
            .count(),
        1
    );
}

#[tokio::test]
async fn deleted_parent_directory_allows_dd_accept_and_rejects_text_save() {
    for side in [Side::Yours, Side::Theirs] {
        let r = Repo::new().await;
        r.initial().await;
        let oid = String::from_utf8(r.read(ReadOperation::RevParse, &["HEAD:A"]).await).unwrap();
        let mut options = r.options.clone();
        options.stdin = format!("100644 {} 1\tmissing/deep/file\n", oid.trim()).into_bytes();
        r.runner
            .run(
                WriteCommand::new("update-index").args(["--index-info"]),
                &options,
            )
            .await
            .unwrap();
        std::fs::create_dir_all(r.path("missing/deep")).unwrap();
        r.put("missing/deep/file", "old worktree content");
        std::fs::remove_dir_all(r.path("missing")).unwrap();
        let p = RepoPath::new(b"missing/deep/file");
        let content = conflict::read(&r.runner, &r.options, &p).await.unwrap();
        assert!(conflict::resolve(
            &r.runner,
            &r.options,
            &p,
            &content.fingerprint,
            None,
            Some("new")
        )
        .await
        .is_err());
        assert!(!r.path("missing").exists());
        conflict::resolve(
            &r.runner,
            &r.options,
            &p,
            &token(&r).await,
            Some(side),
            None,
        )
        .await
        .unwrap();
        assert!(repo::status::read(&r.runner, &r.options)
            .await
            .unwrap()
            .conflicts
            .is_empty());
        assert!(!r.path("missing").exists());
    }
}

#[tokio::test]
async fn accept_restores_a_selected_file_after_its_parent_was_removed() {
    let r = Repo::new().await;
    r.initial().await;
    std::fs::create_dir_all(r.path("removed/deep")).unwrap();
    r.put("removed/deep/file", "old worktree content");
    std::fs::remove_dir_all(r.path("removed")).unwrap();
    let oid = String::from_utf8(r.read(ReadOperation::RevParse, &["HEAD:A"]).await).unwrap();
    let mut options = r.options.clone();
    options.stdin = format!(
        "100644 {0} 1\tremoved/deep/file\n100644 {0} 2\tremoved/deep/file\n",
        oid.trim()
    )
    .into_bytes();
    r.runner
        .run(
            WriteCommand::new("update-index").args(["--index-info"]),
            &options,
        )
        .await
        .unwrap();
    conflict::resolve(
        &r.runner,
        &r.options,
        &RepoPath::new(b"removed/deep/file"),
        &token(&r).await,
        Some(Side::Yours),
        None,
    )
    .await
    .unwrap();
    assert_eq!(
        std::fs::read(r.path("removed/deep/file")).unwrap(),
        b"A base\n"
    );
    assert!(repo::status::read(&r.runner, &r.options)
        .await
        .unwrap()
        .conflicts
        .is_empty());
}

#[cfg(unix)]
#[tokio::test]
async fn missing_parent_below_a_symlink_cannot_escape_the_worktree() {
    let r = Repo::new().await;
    r.initial().await;
    let outside = tempfile::tempdir().unwrap();
    let oid = String::from_utf8(r.read(ReadOperation::RevParse, &["HEAD:A"]).await).unwrap();
    let mut options = r.options.clone();
    options.stdin = format!("100644 {} 1\tescape/missing/file\n", oid.trim()).into_bytes();
    r.runner
        .run(
            WriteCommand::new("update-index").args(["--index-info"]),
            &options,
        )
        .await
        .unwrap();
    std::os::unix::fs::symlink(outside.path(), r.path("escape")).unwrap();
    let error = conflict::read(
        &r.runner,
        &r.options,
        &RepoPath::new(b"escape/missing/file"),
    )
    .await
    .unwrap_err();
    assert!(
        error.to_string().contains("escapes worktree") || error.to_string().contains("invalidPath")
    );
    assert!(!outside.path().join("missing").exists());
}

mod support;
use gitwood_core::{
    git::{ReadOperation, WriteCommand},
    history::details::{self, Filter},
    repo::{
        self,
        session::Sessions,
        watch::{Refresh, RepositoryWatch},
        Repository,
    },
};
use support::Repo;
async fn status(r: &Repo) -> repo::status::Status {
    repo::status::read(&r.runner, &r.options).await.unwrap()
}
#[tokio::test]
async fn status_changes_and_byte_paths() {
    let r = Repo::new().await;
    assert_eq!(status(&r).await.branch.oid.as_deref(), Some("(initial)"));
    r.initial().await;
    r.put("A", "staged\n");
    r.git("add", &["A"]).await;
    r.put("A", "worktree\n");
    std::fs::remove_file(r.path("B")).unwrap();
    for name in ["中文 空格", "-leading"] {
        r.put(name, "new");
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        for name in [b"tab\tfile".to_vec(), b"line\nfile".to_vec()] {
            std::fs::write(r.dir.path().join(std::ffi::OsString::from_vec(name)), b"x").unwrap();
        }
    }
    r.put(".gitignore", "ignored\n");
    r.put("ignored", "x");
    let s = status(&r).await;
    assert!(s.staged.iter().any(|e| e.path.bytes == b"A"));
    assert!(s.unstaged.iter().any(|e| e.path.bytes == b"A"));
    assert!(s.unstaged.iter().any(|e| e.xy == ".D"));
    assert_eq!(s.ignored.len(), 1);
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        use std::os::unix::ffi::OsStringExt;
        std::fs::write(
            r.dir
                .path()
                .join(std::ffi::OsString::from_vec(vec![b'x', 255])),
            b"x",
        )
        .unwrap();
        assert!(status(&r)
            .await
            .untracked
            .iter()
            .any(|e| e.path.bytes == vec![b'x', 255]));
    }
    // APFS rejects invalid UTF-8 filenames; parser preservation is still verified.
    assert_eq!(
        repo::status::parse(b"? x\xff\0").unwrap().untracked[0]
            .path
            .bytes,
        vec![b'x', 255]
    );
    r.git("reset", &["--hard", "-q"]).await;
    r.git("mv", &["A", "renamed A"]).await;
    let s = status(&r).await;
    let e = &s.staged[0];
    assert_eq!(e.original.as_ref().unwrap().bytes, b"A");
    assert_eq!(e.similarity.as_deref(), Some("R100"));
    r.git("checkout", &["--detach", "-q"]).await;
    assert_eq!(status(&r).await.branch.head.as_deref(), Some("(detached)"));
}
#[tokio::test]
async fn real_copy_status_and_conflict() {
    let r = Repo::new().await;
    r.initial().await;
    r.git("config", &["status.renames", "copies"]).await;
    r.put("copy", "A base\n");
    r.put("A", "A modified\n");
    r.git("add", &["A", "copy"]).await;
    // Git only reports copies from modified sources; force a larger common body.
    r.git("reset", &["--hard", "-q"]).await;
    let body = "common line\n".repeat(100);
    r.put("A", &body);
    r.git("add", &["A"]).await;
    r.git("commit", &["-qm", "large"]).await;
    r.put("copy", &body);
    r.put("A", &(body + "tail\n"));
    r.git("add", &["A", "copy"]).await;
    assert!(status(&r)
        .await
        .staged
        .iter()
        .any(|e| e.similarity.as_deref() == Some("C100")));
    r.git("reset", &["--hard", "-q"]).await;
    r.git("branch", &["side"]).await;
    r.put("B", "main\n");
    r.git("commit", &["-qam", "main"]).await;
    let main = String::from_utf8(r.read(ReadOperation::RevParse, &["HEAD"]).await).unwrap();
    r.git("checkout", &["-q", "side"]).await;
    r.put("B", "side\n");
    r.git("commit", &["-qam", "side"]).await;
    assert!(r
        .runner
        .run(WriteCommand::new("merge").args([main.trim()]), &r.options)
        .await
        .is_err());
    assert_eq!(status(&r).await.conflicts.len(), 1);
}
#[tokio::test]
async fn repository_identification() {
    let r = Repo::new().await;
    r.initial().await;
    std::fs::create_dir(r.path("sub")).unwrap();
    let mut o = r.options.clone();
    o.cwd = r.path("sub");
    let repo = Repository::open(&r.runner, &o).await.unwrap();
    assert_eq!(repo.root, std::fs::canonicalize(r.dir.path()).unwrap());
    let linked = r.path("linked");
    r.git(
        "worktree",
        &["add", "-qb", "linked", linked.to_str().unwrap()],
    )
    .await;
    o.cwd = linked;
    assert!(Repository::open(&r.runner, &o).await.unwrap().linked);
    let bare = tempfile::tempdir().unwrap();
    o.cwd = bare.path().into();
    r.runner
        .run(
            WriteCommand::new("init").args(["--bare", "--template="]),
            &o,
        )
        .await
        .unwrap();
    assert!(Repository::open(&r.runner, &o)
        .await
        .unwrap_err()
        .to_string()
        .contains("bareUnsupported"));
    let empty = tempfile::tempdir().unwrap();
    o.cwd = empty.path().into();
    assert!(Repository::open(&r.runner, &o).await.is_err());
}
#[tokio::test]
async fn refs_tracking_tags_and_divergence() {
    let r = Repo::new().await;
    r.initial().await;
    let head = String::from_utf8(r.read(ReadOperation::RevParse, &["HEAD"]).await).unwrap();
    let branch = String::from_utf8(r.git("branch", &["--show-current"]).await).unwrap();
    for remote in ["origin", "second"] {
        r.git(
            "remote",
            &["add", remote, "https://example.invalid/no-network"],
        )
        .await;
        r.git(
            "update-ref",
            &[&format!("refs/remotes/{remote}/main"), head.trim()],
        )
        .await;
    }
    r.git(
        "symbolic-ref",
        &["refs/remotes/origin/HEAD", "refs/remotes/origin/main"],
    )
    .await;
    r.git("branch", &["--set-upstream-to=origin/main", branch.trim()])
        .await;
    r.git("tag", &["light"]).await;
    r.git("tag", &["-a", "annotated", "-m", "annotation"]).await;
    r.put("A", "ahead\n");
    r.git("commit", &["-qam", "ahead"]).await;
    let refs = repo::refs::read(&r.runner, &r.options).await.unwrap();
    assert_eq!(refs.remotes.len(), 2);
    assert_eq!(refs.remote_heads.len(), 1);
    assert_eq!(refs.remotes["origin"].len(), 1);
    assert_eq!(refs.tags.len(), 2);
    assert!(refs.tags.iter().any(|t| t.peeled.is_some()));
    assert!(refs.local[0].track.contains("ahead 1"));
    assert_eq!(refs.head.ahead, 1);
    r.git("branch", &["fork", head.trim()]).await;
    r.git("checkout", &["-q", "fork"]).await;
    r.put("B", "behind\n");
    r.git("commit", &["-qam", "behind"]).await;
    let fork = String::from_utf8(r.read(ReadOperation::RevParse, &["HEAD"]).await).unwrap();
    r.git("update-ref", &["refs/remotes/origin/main", fork.trim()])
        .await;
    r.git("checkout", &["-q", branch.trim()]).await;
    let s = status(&r).await;
    assert_eq!((s.branch.ahead, s.branch.behind), (1, 1));
}
#[tokio::test]
async fn history_paging_filtering_details_and_stale() {
    let r = Repo::new().await;
    let sessions = Sessions::default();
    let l = sessions
        .open(r.runner.clone(), r.options.clone())
        .await
        .unwrap();
    assert_eq!(l.layout(&Filter::default()).await.unwrap().total, 0);
    r.initial().await;
    r.put("-path", "body");
    r.git("add", &["--", "-path"]).await;
    r.git("commit", &["-qm", "--output=sentinel\n\nfull description"])
        .await;
    assert_eq!(l.layout(&Filter::default()).await.unwrap().total, 2);
    let rows = l.rows(0, 1).await.unwrap();
    assert_eq!(rows.commits.len(), 1);
    assert_eq!(l.rows(1, 2).await.unwrap().commits[0].subject, "base");
    let hash = &rows.commits[0].hash;
    assert!(l
        .details(hash)
        .await
        .unwrap()
        .message
        .contains("full description"));
    assert_eq!(l.files(hash).await.unwrap()[0].path.bytes, b"-path");
    assert!(l.rows(0, 201).await.is_err());
    for f in [
        Filter {
            text: Some("--output=sentinel".into()),
            ..Filter::default()
        },
        Filter {
            text: Some(hash[..8].into()),
            ..Filter::default()
        },
        Filter {
            path: Some(repo::RepoPath::new(b"-path")),
            ..Filter::default()
        },
    ] {
        assert_eq!(l.layout(&f).await.unwrap().total, 1);
    }
    for f in [
        Filter {
            author: Some("--output=sentinel".into()),
            ..Filter::default()
        },
        Filter {
            since: Some("--output=sentinel".into()),
            ..Filter::default()
        },
        Filter {
            until: Some("--output=sentinel".into()),
            ..Filter::default()
        },
    ] {
        let _ = l.layout(&f).await;
        assert!(!r.path("sentinel").exists());
    }
    assert!(details::validate_hash("--output=sentinel").is_err());
    assert!(l
        .layout(&Filter {
            branch: Some("--all".into()),
            ..Filter::default()
        })
        .await
        .is_err());
    let old = l.generation;
    let second = Repo::new().await;
    let next = sessions
        .open(second.runner.clone(), second.options.clone())
        .await
        .unwrap();
    assert!(sessions.check(&l).is_err());
    assert!(matches!(
        l.details(hash).await,
        Err(gitwood_core::git::GitError::Cancelled)
    ));
    assert!(sessions.lease(old).is_err());
    assert!(sessions.lease(next.generation).is_ok());
    let refreshed = sessions.refresh(next.generation).unwrap();
    assert!(refreshed > next.generation);
    assert!(sessions.check(&next).is_err());
    assert!(sessions.lease(refreshed).is_ok());
}
#[tokio::test]
async fn watcher_real_events() {
    let r = Repo::new().await;
    r.initial().await;
    let repository = Repository::open(&r.runner, &r.options).await.unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let watch = RepositoryWatch::start(repository, move |e| {
        tx.send(e).unwrap();
    })
    .unwrap();
    r.put("A", "changed\n");
    let first = rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
    assert!(first.status);
    r.git("commit", &["-qam", "external"]).await;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let e: Refresh = rx
            .recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
            .unwrap();
        if e.refs && e.history {
            break;
        }
    }
    // Drain preceding commit events before testing excluded object writes.
    while rx
        .recv_timeout(std::time::Duration::from_millis(700))
        .is_ok()
    {}
    std::fs::write(r.path(".git/objects/ignored-fixture"), b"ignored").unwrap();
    assert!(rx
        .recv_timeout(std::time::Duration::from_millis(900))
        .is_err());
    drop(watch);
}

#[tokio::test]
async fn history_filters_and_first_parent_files() {
    let r = Repo::new().await;
    r.initial().await;
    let root = String::from_utf8(r.read(ReadOperation::RevParse, &["HEAD"]).await).unwrap();
    assert_eq!(
        details::files(&r.runner, &r.options, root.trim())
            .await
            .unwrap()
            .len(),
        2
    );
    let branch = String::from_utf8(r.git("branch", &["--show-current"]).await).unwrap();
    r.git("checkout", &["-qb", "feature"]).await;
    r.put("feature-file", "side");
    r.git("add", &["feature-file"]).await;
    r.git("commit", &["-qm", "side body"]).await;
    let feature = String::from_utf8(r.read(ReadOperation::RevParse, &["HEAD"]).await).unwrap();
    r.git("checkout", &["-q", branch.trim()]).await;
    r.put("main-file", "main");
    r.git("add", &["main-file"]).await;
    r.git("commit", &["-qm", "main body"]).await;
    r.git("merge", &["--no-ff", "feature", "-m", "merge"]).await;
    let hash = String::from_utf8(r.read(ReadOperation::RevParse, &["HEAD"]).await).unwrap();
    let files = details::files(&r.runner, &r.options, hash.trim())
        .await
        .unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path.bytes, b"feature-file");
    for (filter, expected) in [
        (
            Filter {
                branch: Some("refs/heads/feature".into()),
                ..Filter::default()
            },
            2,
        ),
        (
            Filter {
                author: Some("Gitwood Test".into()),
                ..Filter::default()
            },
            4,
        ),
        (
            Filter {
                since: Some("2030-01-01T00:00:00Z".into()),
                ..Filter::default()
            },
            0,
        ),
        (
            Filter {
                until: Some("2000-01-01".into()),
                ..Filter::default()
            },
            0,
        ),
        (
            Filter {
                text: Some("side body".into()),
                ..Filter::default()
            },
            1,
        ),
    ] {
        assert_eq!(
            details::layout(&r.runner, &r.options, &filter)
                .await
                .unwrap()
                .graph()
                .len(),
            expected
        );
    }
    r.git("checkout", &["--detach", "-q", feature.trim()]).await;
    r.put("detached", "only here");
    r.git("add", &["detached"]).await;
    r.git("commit", &["-qm", "detached-only"]).await;
    assert_eq!(
        details::layout(&r.runner, &r.options, &Filter::default())
            .await
            .unwrap()
            .graph()
            .len(),
        5
    );
}

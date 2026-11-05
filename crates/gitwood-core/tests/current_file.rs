mod support;
use gitwood_core::repo::{session::Sessions, RepoPath};
use support::Repo;

#[tokio::test]
async fn file_history_tracks_rename_paths_and_pages_with_literal_special_names() {
    let repo = Repo::new().await;
    let first = "-wild * [a].txt";
    repo.put(first, "a line that remains the same\nanother stable line\n");
    repo.git("add", &["--", first]).await;
    repo.git("commit", &["-m", "first file"]).await;
    repo.git("mv", &["--", first, "space name -middle?.txt"])
        .await;
    repo.git("commit", &["-am", "rename one"]).await;
    repo.git(
        "mv",
        &["--", "space name -middle?.txt", "final [glob]*.txt"],
    )
    .await;
    repo.git("commit", &["-am", "rename two"]).await;

    let sessions = Sessions::default();
    let lease = sessions
        .open(repo.runner.clone(), repo.options.clone())
        .await
        .unwrap();
    let summary = lease
        .file_history_open(&RepoPath::new(b"final [glob]*.txt"), "HEAD")
        .await
        .unwrap();
    assert_eq!(summary.total, 3);
    let newest = lease.file_history_page(&summary.id, 0, 1).await.unwrap();
    assert_eq!(newest.rows[0].path.bytes, b"final [glob]*.txt");
    let middle = lease.file_history_page(&summary.id, 1, 2).await.unwrap();
    assert_eq!(middle.rows[0].path.bytes, b"space name -middle?.txt");
    let oldest = lease.file_history_page(&summary.id, 2, 3).await.unwrap();
    assert_eq!(oldest.rows[0].path.bytes, first.as_bytes());
    assert!(lease.file_history_page(&summary.id, 0, 201).await.is_err());
    assert!(lease
        .file_history_open(&RepoPath::new(b"final [glob]*.txt"), "--output=bad")
        .await
        .is_err());
    assert!(lease
        .file_history_open(&RepoPath::new(b"../outside"), "HEAD")
        .await
        .is_err());
}

#[tokio::test]
async fn annotate_does_not_replace_an_open_file_history_pages() {
    let repo = Repo::new().await;
    repo.put("history.txt", "first revision\n");
    repo.git("add", &["--", "history.txt"]).await;
    repo.git("commit", &["-m", "history one"]).await;
    repo.put("history.txt", "second revision\n");
    repo.git("commit", &["-am", "history two"]).await;
    repo.put("annotate.txt", "annotated file\n");
    repo.git("add", &["--", "annotate.txt"]).await;
    repo.git("commit", &["-m", "annotate file"]).await;

    let sessions = Sessions::default();
    let lease = sessions
        .open(repo.runner.clone(), repo.options.clone())
        .await
        .unwrap();
    let history = lease
        .file_history_open(&RepoPath::new(b"history.txt"), "HEAD")
        .await
        .unwrap();
    assert_eq!(history.total, 2);

    lease
        .annotate_open(&RepoPath::new(b"annotate.txt"), "HEAD")
        .await
        .unwrap();

    let first = lease.file_history_page(&history.id, 0, 1).await.unwrap();
    let second = lease.file_history_page(&history.id, 1, 2).await.unwrap();
    assert_eq!(first.total, 2);
    assert_eq!(second.rows[0].subject, "history one");
}

#[cfg(unix)]
#[tokio::test]
async fn file_history_passes_non_utf8_path_bytes_without_stringification() {
    let repo = Repo::new().await;
    repo.initial().await;
    let sessions = Sessions::default();
    let lease = sessions
        .open(repo.runner.clone(), repo.options.clone())
        .await
        .unwrap();
    let page = lease
        .file_history_open(&RepoPath::new(b"bad-\xff-name"), "HEAD")
        .await
        .unwrap();
    assert_eq!(page.total, 0);
}

#[tokio::test]
async fn annotate_incremental_matches_git_line_attribution_and_compares_single_file() {
    let repo = Repo::new().await;
    let original = (0..205)
        .map(|line| format!("line {line}\n"))
        .collect::<String>();
    repo.put("before.txt", &original);
    repo.git("add", &["--", "before.txt"]).await;
    repo.git("commit", &["-m", "original"]).await;
    repo.git("mv", &["--", "before.txt", "renamed * file.txt"])
        .await;
    repo.git("commit", &["-m", "renamed file"]).await;
    let edited = original
        .lines()
        .enumerate()
        .map(|(line, text)| if line == 1 { "changed line" } else { text })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    repo.put("renamed * file.txt", &edited);
    repo.git("add", &["--", "renamed * file.txt"]).await;
    repo.git("commit", &["-m", "edited after rename"]).await;
    repo.git("branch", &["-M", "main"]).await;

    let sessions = Sessions::default();
    let lease = sessions
        .open(repo.runner.clone(), repo.options.clone())
        .await
        .unwrap();
    let path = RepoPath::new(b"renamed * file.txt");
    let summary = lease.annotate_open(&path, "HEAD").await.unwrap();
    assert_eq!(summary.total, 205);
    let first_page = lease.annotate_page(&summary.id, 0, 200).await.unwrap();
    let last_page = lease.annotate_page(&summary.id, 200, 205).await.unwrap();
    let lines = [first_page.lines, last_page.lines].concat();
    assert_eq!(lines[0].text, "line 0");
    assert_eq!(lines[1].text, "changed line");
    assert_eq!(lines[204].text, "line 204");
    let original_hash = String::from_utf8_lossy(
        &repo
            .read(gitwood_core::git::ReadOperation::RevParse, &["HEAD~2"])
            .await,
    )
    .trim()
    .to_owned();
    assert_eq!(lines[0].hash, original_hash);
    assert_eq!(lines[1].hash, summary.revision);
    let blame = repo
        .read(
            gitwood_core::git::ReadOperation::Blame,
            &[
                "--porcelain",
                "--incremental",
                "HEAD",
                "--",
                "renamed * file.txt",
            ],
        )
        .await;
    let expected = String::from_utf8_lossy(&blame)
        .lines()
        .filter_map(|line| {
            let fields: Vec<_> = line.split_ascii_whitespace().collect();
            if fields.len() == 4 && fields[0].len() >= 40 {
                Some((
                    fields[2].parse::<usize>().ok()?,
                    fields[3].parse::<usize>().ok()?,
                    fields[0].to_owned(),
                ))
            } else {
                None
            }
        })
        .flat_map(|(start, count, hash)| {
            (start..start + count).map(move |line| (line - 1, hash.clone()))
        })
        .collect::<Vec<_>>();
    for (index, hash) in expected {
        assert_eq!(lines[index].hash, hash);
    }
    let previous_hash = lines[1].previous.as_deref().unwrap();
    let previous_path = lines[1].previous_path.as_ref().unwrap();
    assert_eq!(previous_path.bytes, b"renamed * file.txt");
    let old = lease
        .annotate_open(previous_path, previous_hash)
        .await
        .unwrap();
    assert_eq!(old.total, 205);
    let compared = lease
        .compare_file_with_revision(&path, previous_hash)
        .await
        .unwrap();
    assert!(compared.result.is_some());
    assert!(
        matches!(compared.left, gitwood_core::diff::Content::Text { ref text, .. } if text == &edited)
    );
    assert!(
        matches!(compared.right, gitwood_core::diff::Content::Text { ref text, .. } if text == &original)
    );
    let initial_hash = String::from_utf8_lossy(
        &repo
            .read(gitwood_core::git::ReadOperation::RevParse, &["HEAD~2"])
            .await,
    )
    .trim()
    .to_owned();
    let before_rename = lease
        .compare_file_with_revision(&path, &initial_hash)
        .await
        .unwrap();
    assert!(before_rename.result.is_some());
    assert!(
        matches!(before_rename.right, gitwood_core::diff::Content::Text { ref text, .. } if text == &original)
    );
    let branch_diff = lease
        .compare_file_with_revision(&path, "refs/heads/main")
        .await
        .unwrap();
    assert!(branch_diff.result.is_some());
    assert!(
        matches!(branch_diff.right, gitwood_core::diff::Content::Text { ref text, .. } if text == &edited)
    );
    std::fs::remove_file(repo.path("renamed * file.txt")).unwrap();
    let deleted = lease
        .compare_file_with_revision(&path, previous_hash)
        .await
        .unwrap();
    assert!(matches!(
        deleted.left,
        gitwood_core::diff::Content::Text { empty: true, .. }
    ));
    assert!(lease
        .compare_file_with_revision(&RepoPath::new(b"../outside"), "HEAD")
        .await
        .is_err());
    assert!(lease
        .annotate_open(&RepoPath::new(b"../outside"), "HEAD")
        .await
        .is_err());
    assert!(lease.annotate_page(&old.id, 0, 201).await.is_err());
    assert!(lease.annotate_open(&path, "--output=bad").await.is_err());
    lease.annotate_cancel().await;
    assert!(lease.annotate_page(&old.id, 0, 1).await.is_err());
}

#[cfg(unix)]
#[tokio::test]
async fn closing_annotation_cancels_and_terminates_the_running_git_process() {
    use gitwood_core::git::GitRunner;
    use std::time::Duration;
    use support::script;

    let repo = Repo::new().await;
    repo.put("large.txt", &"line\n".repeat(20_000));
    repo.git("add", &["--", "large.txt"]).await;
    repo.git("commit", &["-m", "annotate fixture"]).await;
    let marker = repo.path("blame.started");
    let wrapper = repo.path("slow-git");
    let git_path = repo.runner.path().display().to_string();
    let marker_path = marker.display().to_string();
    script(&wrapper, &format!(
        "for argument in \"$@\"; do if [ \"$argument\" = blame ]; then touch '{marker_path}'; sleep 30; break; fi; done\nexec '{git_path}' \"$@\""
    ));
    let runner = GitRunner::discover(Some(&wrapper)).await.unwrap();
    let sessions = Sessions::default();
    let lease = sessions.open(runner, repo.options.clone()).await.unwrap();
    let summary = lease
        .annotate_open(&RepoPath::new(b"large.txt"), "HEAD")
        .await
        .unwrap();
    let worker = lease.clone();
    let id = summary.id.clone();
    let task = tokio::spawn(async move { worker.annotate_page(&id, 0, 200).await });
    tokio::time::timeout(Duration::from_secs(5), async {
        while !marker.exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    lease.annotate_cancel().await;
    let result = tokio::time::timeout(Duration::from_secs(3), task)
        .await
        .unwrap()
        .unwrap();
    assert!(result.is_err());
}

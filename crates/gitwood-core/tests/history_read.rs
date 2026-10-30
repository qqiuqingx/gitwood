//! Exercise the production relationship reader against real isolated Git repos.

use gitwood_core::git::{GitError, GitRunner, RunOptions};
use gitwood_core::history::{read, CommitId, GraphLayout, Head};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let fixture = Self(std::env::temp_dir().join(format!(
            "gitwood history 中文 {}-{unique}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        )));
        fs::create_dir(&fixture.0).unwrap();
        fixture.git(&["init", "--quiet", "--template=", "--initial-branch=main"]);
        fixture.git(&["config", "user.name", "Gitwood Fixture"]);
        fixture.git(&["config", "user.email", "fixture@example.invalid"]);
        fixture.git(&["config", "commit.gpgsign", "false"]);
        fixture
    }

    fn git(&self, args: &[&str]) -> String {
        let result = Command::new("git")
            .args(args)
            .current_dir(&self.0)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env(
                "GIT_CONFIG_GLOBAL",
                if cfg!(windows) { "NUL" } else { "/dev/null" },
            )
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        String::from_utf8(result.stdout).unwrap().trim().to_owned()
    }

    fn commit(&self, message: &str, parents: &[&str]) -> String {
        let tree = self.git(&["write-tree"]);
        let mut args = vec!["commit-tree", &tree, "-m", message];
        for parent in parents {
            args.extend(["-p", *parent]);
        }
        self.git(&args)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[tokio::test]
async fn reads_all_refs_and_detached_head_with_closed_topological_relationships() {
    let fixture = Fixture::new();
    let root = fixture.commit("root", &[]);
    let main = fixture.commit("main", &[&root]);
    let detached = fixture.commit("detached", &[&root]);
    let other_root = fixture.commit("disconnected", &[]);
    fixture.git(&["update-ref", "refs/heads/main", &main]);
    fixture.git(&["update-ref", "refs/heads/other", &other_root]);
    fixture.git(&["checkout", "--quiet", "--detach", &detached]);
    let parsed = read::read_graph(
        &GitRunner::discover(None).await.unwrap(),
        &RunOptions::new(&fixture.0),
    )
    .await
    .unwrap();
    assert_eq!(parsed.len(), 4);
    assert_eq!(parsed.edge_count(), 2);
    let detached_id = parsed.find_commit(&detached).unwrap();
    assert!(parsed.find_commit(&other_root).is_some());
    let root_id = parsed.find_commit(&root).unwrap();
    assert_eq!(parsed.parents(detached_id).collect::<Vec<_>>(), [root_id]);
    let layout = GraphLayout::new(
        parsed,
        &[Head {
            commit: detached_id,
            reference: Some("HEAD"),
        }],
    )
    .unwrap();
    assert_eq!(layout.visible_rows(0, 4).unwrap().rows.len(), 4);
}

#[tokio::test]
async fn shallow_boundary_is_a_root_without_a_dangling_parent() {
    let fixture = Fixture::new();
    let root = fixture.commit("root", &[]);
    let tip = fixture.commit("tip", &[&root]);
    fixture.git(&["update-ref", "refs/heads/main", &tip]);
    fs::write(fixture.0.join(".git/shallow"), format!("{tip}\n")).unwrap();
    let graph = read::read_graph(
        &GitRunner::discover(None).await.unwrap(),
        &RunOptions::new(&fixture.0),
    )
    .await
    .unwrap();
    assert_eq!(graph.len(), 1);
    assert_eq!(graph.parents(CommitId(0)).count(), 0);
}

#[tokio::test]
async fn read_errors_retain_status_stderr_and_spawn_errors() {
    let fixture = Fixture::new();
    match read::read_graph(
        &GitRunner::discover(None).await.unwrap(),
        &RunOptions::new(&fixture.0),
    )
    .await
    .unwrap_err()
    {
        read::ReadError::Git(GitError::Exit { code, stderr }) => {
            assert!(code.is_some_and(|value| value != 0));
            assert!(!stderr.is_empty());
        }
        other => panic!("expected Git exit error, got {other}"),
    }
    let runner = GitRunner::discover(None).await.unwrap();
    let mut options = RunOptions::new(&fixture.0);
    options.cwd = fixture.0.join("nonexistent-directory");
    assert!(matches!(
        read::read_log(&runner, &options).await,
        Err(read::ReadError::Git(GitError::Spawn { .. }))
    ));
}

#[cfg(unix)]
#[tokio::test]
async fn background_read_does_not_invoke_repository_programs_or_signature_verifier() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    let root = fixture.commit("root", &[]);
    fixture.git(&["update-ref", "refs/heads/main", &root]);
    let program = fixture.0.join("untrusted.sh");
    fs::write(
        &program,
        b"#!/bin/sh\nprintf invoked > untrusted-invoked\nexit 1\n",
    )
    .unwrap();
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
    let executable = format!("'{}'", program.display());
    fixture.git(&["config", "core.fsmonitor", &executable]);
    fixture.git(&["config", "diff.external", &executable]);
    fixture.git(&["config", "diff.untrusted.textconv", &executable]);
    fixture.git(&["config", "gpg.program", program.to_str().unwrap()]);
    fixture.git(&["config", "log.showSignature", "true"]);
    fixture.git(&["config", "log.showNotes", "true"]);
    fixture.git(&["config", "color.ui", "always"]);
    // Invalid signature still causes Git to invoke a verifier unless disabled.
    let unsigned = fixture.git(&["cat-file", "commit", &root]);
    let mut signed = unsigned.replacen(
        "\n\n",
        "\ngpgsig -----BEGIN PGP SIGNATURE-----\n bogus\n -----END PGP SIGNATURE-----\n\n",
        1,
    );
    signed.push('\n');
    let commit_file = fixture.0.join("signed-commit");
    fs::write(&commit_file, signed).unwrap();
    let signed_id = fixture.git(&[
        "hash-object",
        "-t",
        "commit",
        "-w",
        commit_file.to_str().unwrap(),
    ]);
    fixture.git(&["update-ref", "refs/heads/main", &signed_id]);
    let graph = read::read_graph(
        &GitRunner::discover(None).await.unwrap(),
        &RunOptions::new(&fixture.0),
    )
    .await
    .unwrap();
    assert_eq!(graph.len(), 1);
    assert!(!fixture.0.join("untrusted-invoked").exists());
}

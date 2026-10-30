mod support;
use gitwood_core::{
    git::ReadOperation,
    ops::{
        branch::{branch, BranchOperation as Op, BranchOutcome as Out, CheckoutMode},
        snapshot,
    },
};
use support::Repo;
async fn run(r: &Repo, op: Op) -> Out {
    let token = snapshot(&r.runner, &r.options).await.unwrap().fingerprint;
    branch(&r.runner, &r.options, &op, &token).await.unwrap()
}
#[tokio::test]
async fn branch_lifecycle_and_validation() {
    let r = Repo::new().await;
    r.initial().await;
    let original = String::from_utf8(r.git("branch", &["--show-current"]).await)
        .unwrap()
        .trim()
        .to_owned();
    assert!(matches!(
        run(
            &r,
            Op::Create {
                name: "topic".into(),
                start: None,
                checkout: true
            }
        )
        .await,
        Out::Completed
    ));
    run(
        &r,
        Op::Rename {
            name: "topic".into(),
            new_name: "renamed".into(),
        },
    )
    .await;
    r.put("A", "new\n");
    r.git("commit", &["-am", "topic"]).await;
    run(
        &r,
        Op::Checkout {
            name: original,
            mode: CheckoutMode::Normal,
        },
    )
    .await;
    assert!(matches!(
        run(
            &r,
            Op::Delete {
                name: "renamed".into(),
                force: false
            }
        )
        .await,
        Out::NotMerged { .. }
    ));
    assert!(matches!(
        run(
            &r,
            Op::Delete {
                name: "renamed".into(),
                force: true
            }
        )
        .await,
        Out::Completed
    ));
    for name in ["-bad", "bad..name", "@{-1}"] {
        let token = snapshot(&r.runner, &r.options).await.unwrap().fingerprint;
        assert!(branch(
            &r.runner,
            &r.options,
            &Op::Create {
                name: name.into(),
                start: None,
                checkout: false
            },
            &token
        )
        .await
        .is_err());
    }
}
#[tokio::test]
async fn smart_checkout_preserves_stash_on_conflict() {
    let r = Repo::new().await;
    r.initial().await;
    let original = String::from_utf8(r.git("branch", &["--show-current"]).await)
        .unwrap()
        .trim()
        .to_owned();
    r.git("checkout", &["-b", "topic"]).await;
    r.put("A", "topic\n");
    r.git("commit", &["-am", "topic"]).await;
    r.git("checkout", &[&original]).await;
    r.put("A", "local\n");
    assert!(
        matches!(run(&r,Op::Checkout{name:"topic".into(),mode:CheckoutMode::Normal}).await,Out::LocalChanges{files} if !files.is_empty())
    );
    let out = run(
        &r,
        Op::Checkout {
            name: "topic".into(),
            mode: CheckoutMode::Smart,
        },
    )
    .await;
    assert!(matches!(out,Out::StashRecovery{files,..} if !files.is_empty()));
    assert!(!r
        .read(ReadOperation::RevParse, &["--verify", "refs/stash"])
        .await
        .is_empty());
}
#[tokio::test]
async fn smart_checkout_success_and_remote_tracking() {
    let r = Repo::new().await;
    r.initial().await;
    r.git("branch", &["topic"]).await;
    r.put("A", "local\n");
    assert!(matches!(
        run(
            &r,
            Op::Checkout {
                name: "topic".into(),
                mode: CheckoutMode::Smart
            }
        )
        .await,
        Out::Completed
    ));
    assert_eq!(std::fs::read_to_string(r.path("A")).unwrap(), "local\n");
    r.git("reset", &["--hard"]).await;
    r.git("remote", &["add", "origin", "/nonexistent/gitwood-test"])
        .await;
    r.git("update-ref", &["refs/remotes/origin/remote-topic", "HEAD"])
        .await;
    run(
        &r,
        Op::CheckoutRemote {
            remote: "origin".into(),
            branch: "remote-topic".into(),
            mode: CheckoutMode::Normal,
        },
    )
    .await;
    assert_eq!(
        r.git("config", &["branch.remote-topic.remote"]).await,
        b"origin\n"
    );
}
#[tokio::test]
async fn force_checkout_and_quoted_affected_paths() {
    let r = Repo::new().await;
    r.initial().await;
    let name = "tab\tfile";
    r.put(name, "base\n");
    r.git("add", &["--", name]).await;
    r.git("commit", &["-m", "file"]).await;
    let original = String::from_utf8(r.git("branch", &["--show-current"]).await)
        .unwrap()
        .trim()
        .to_owned();
    r.git("checkout", &["-b", "topic"]).await;
    r.put(name, "topic\n");
    r.git("commit", &["-am", "topic"]).await;
    r.git("checkout", &[&original]).await;
    r.put(name, "local\n");
    assert!(
        matches!(run(&r,Op::Checkout{name:"topic".into(),mode:CheckoutMode::Normal}).await,Out::LocalChanges{files} if files.iter().any(|p|p.bytes==name.as_bytes()))
    );
    assert!(matches!(
        run(
            &r,
            Op::Checkout {
                name: "topic".into(),
                mode: CheckoutMode::Force
            }
        )
        .await,
        Out::Completed
    ));
    assert_eq!(std::fs::read_to_string(r.path(name)).unwrap(), "topic\n");
    run(
        &r,
        Op::SetUpstream {
            name: "topic".into(),
            upstream: Some(format!("refs/heads/{original}")),
        },
    )
    .await;
    run(
        &r,
        Op::SetUpstream {
            name: "topic".into(),
            upstream: None,
        },
    )
    .await;
}

#[cfg(unix)]
#[tokio::test]
async fn smart_checkout_external_stash_is_preserved() {
    let r = Repo::new().await;
    r.initial().await;
    r.git("branch", &["topic"]).await;
    support::script(
        &r.path(".git/hooks/post-checkout"),
        "printf external > B\ngit stash push -qm external",
    );
    r.put("A", "local\n");
    assert!(matches!(
        run(
            &r,
            Op::Checkout {
                name: "topic".into(),
                mode: CheckoutMode::Smart
            }
        )
        .await,
        Out::Completed
    ));
    assert_eq!(std::fs::read_to_string(r.path("A")).unwrap(), "local\n");
    assert_eq!(
        r.git("stash", &["list", "--format=%s"])
            .await
            .split(|b| *b == b'\n')
            .filter(|s| !s.is_empty())
            .count(),
        1
    );
    assert!(String::from_utf8(r.git("stash", &["show", "-p"]).await)
        .unwrap()
        .contains("external"));
}

#[cfg(unix)]
#[tokio::test]
async fn smart_checkout_external_pop_returns_recovery_without_dropping_user_stash() {
    let r = Repo::new().await;
    r.initial().await;
    r.git("branch", &["topic"]).await;
    r.put("B", "user saved\n");
    r.git("stash", &["push", "-qm", "user"]).await;
    let user = r.git("rev-parse", &["refs/stash"]).await;
    support::script(&r.path(".git/hooks/post-checkout"), "git stash pop -q");
    r.put("A", "local\n");
    assert!(matches!(
        run(
            &r,
            Op::Checkout {
                name: "topic".into(),
                mode: CheckoutMode::Smart
            }
        )
        .await,
        Out::StashRecovery { .. }
    ));
    assert_eq!(r.git("rev-parse", &["refs/stash"]).await, user);
    assert_eq!(std::fs::read_to_string(r.path("A")).unwrap(), "local\n");
}

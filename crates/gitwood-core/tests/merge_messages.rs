mod support;
use gitwood_core::{
    git::ReadOperation,
    ops::{
        self,
        integrate::{self, Integration, MergeOptions},
    },
};
use support::Repo;
async fn value(r: &Repo, args: &[&str]) -> String {
    String::from_utf8(r.read(ReadOperation::Log, args).await).unwrap()
}
async fn setup() -> Repo {
    let r = Repo::new().await;
    r.initial().await;
    r.git("branch", &["-M", "main"]).await;
    r.git("checkout", &["-b", "feat"]).await;
    r.put("A", "feature\n");
    r.git("commit", &["-am", "feature subject"]).await;
    r.git("checkout", &["main"]).await;
    r.git("config", &["merge.log", "true"]).await;
    r
}
async fn integrate(r: &Repo, request: Integration) {
    let token = ops::snapshot(&r.runner, &r.options)
        .await
        .unwrap()
        .fingerprint;
    integrate::run(&r.runner, &r.options, &request, &token)
        .await
        .unwrap();
}
#[tokio::test]
async fn named_merge_messages_match_git_for_local_and_remote_tracking() {
    for (remote, log) in [
        (false, "false"),
        (false, "true"),
        (true, "false"),
        (true, "true"),
    ] {
        let r = setup().await;
        r.git("config", &["merge.log", log]).await;
        let (name, full) = if remote {
            r.git("update-ref", &["refs/remotes/origin/x", "feat"])
                .await;
            ("origin/x", "refs/remotes/origin/x")
        } else {
            ("feat", "refs/heads/feat")
        };
        let original = value(&r, &["-1", "--format=%H"]).await;
        r.git("merge", &["--no-ff", "--no-edit", name]).await;
        let expected = value(&r, &["-1", "--format=%B"]).await;
        r.git("reset", &["--hard", original.trim()]).await;
        integrate(
            &r,
            Integration::Merge {
                target: full.into(),
                options: MergeOptions {
                    no_ff: true,
                    ..Default::default()
                },
            },
        )
        .await;
        assert_eq!(value(&r, &["-1", "--format=%B"]).await, expected);
    }
}
#[tokio::test]
async fn pull_messages_match_git_for_single_and_multiple_branches() {
    for (multiple, log) in [
        (false, "false"),
        (false, "true"),
        (true, "false"),
        (true, "true"),
    ] {
        let r = setup().await;
        r.git("config", &["merge.log", log]).await;
        r.git("checkout", &["-b", "other", "main"]).await;
        r.put("B", "other\n");
        r.git("commit", &["-am", "other subject"]).await;
        r.git("checkout", &["main"]).await;
        let bare = tempfile::tempdir().unwrap();
        r.git("init", &["--bare", bare.path().to_str().unwrap()])
            .await;
        r.git("remote", &["add", "origin", bare.path().to_str().unwrap()])
            .await;
        r.git("push", &["origin", "feat", "other"]).await;
        let original = value(&r, &["-1", "--format=%H"]).await;
        let branches = if multiple {
            vec!["feat", "other"]
        } else {
            vec!["feat"]
        };
        let mut args = vec!["--no-rebase", "--no-ff", "--no-edit", "origin"];
        args.extend(&branches);
        r.git("pull", &args).await;
        let expected = value(&r, &["-1", "--format=%B"]).await;
        r.git("reset", &["--hard", original.trim()]).await;
        integrate(
            &r,
            Integration::Pull {
                remote: "origin".into(),
                branches: branches.iter().map(|s| (*s).into()).collect(),
                rebase: false,
                options: MergeOptions {
                    no_ff: true,
                    ..Default::default()
                },
            },
        )
        .await;
        assert_eq!(value(&r, &["-1", "--format=%B"]).await, expected);
    }
}
#[tokio::test]
async fn squash_message_matches_named_git_merge() {
    let r = setup().await;
    r.git("merge", &["--squash", "feat"]).await;
    let expected = std::fs::read(r.path(".git/SQUASH_MSG")).unwrap();
    r.git("reset", &["--hard", "HEAD"]).await;

    integrate(
        &r,
        Integration::Merge {
            target: "refs/heads/feat".into(),
            options: MergeOptions {
                squash: true,
                ..Default::default()
            },
        },
    )
    .await;
    assert_eq!(std::fs::read(r.path(".git/SQUASH_MSG")).unwrap(), expected);
}

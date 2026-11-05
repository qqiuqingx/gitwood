//! Credential prompts: socket protocol, cancellation, and remote operations that never ask.
mod support;

use gitwood_core::{
    git::askpass::{self, Askpass, Prompt, PromptKind},
    ops::{
        remote::{self, PushOptions, RemoteOutcome},
        snapshot,
    },
};
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use support::Repo;

fn answered(value: Option<&'static str>) -> askpass::Answer {
    Box::pin(std::future::ready(value.map(str::to_owned)))
}

/// Handler that records every prompt and answers from a fixed list, in order.
fn recorder(answers: Vec<Option<&'static str>>) -> (Arc<AtomicU64>, askpass::Handler) {
    let asked = Arc::new(AtomicU64::new(0));
    let counter = Arc::clone(&asked);
    let answers = Arc::new(std::sync::Mutex::new(answers));
    let handler: askpass::Handler = Arc::new(move |_, _prompt: Prompt| {
        counter.fetch_add(1, Ordering::SeqCst);
        let next = {
            let mut queued = answers.lock().unwrap();
            (!queued.is_empty()).then(|| queued.remove(0)).flatten()
        };
        answered(next)
    });
    (asked, handler)
}

async fn run_helper(client: askpass::Client, token: String, prompt: &'static str) -> (i32, String) {
    // The helper is a separate process in production; blocking I/O keeps it faithful here.
    tokio::task::spawn_blocking(move || {
        let mut out = Vec::new();
        let code = askpass::helper(&client.socket, &token, prompt, &mut out);
        (code, String::from_utf8_lossy(&out).into_owned())
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn helper_answers_once_and_reports_cancellation() {
    let (asked, handler) = recorder(vec![Some("alice"), None]);
    let askpass = Arc::new(Askpass::start(handler).unwrap());
    let client = askpass.client();

    let (code, out) = run_helper(
        client.clone(),
        client.token.clone(),
        "Username for 'https://example.com': ",
    )
    .await;
    assert_eq!(code, 0);
    assert_eq!(out, "alice\n");
    assert!(!askpass.cancelled());

    let (code, out) = run_helper(
        client.clone(),
        client.token.clone(),
        "Password for 'https://alice@example.com': ",
    )
    .await;
    assert_eq!(code, 1, "cancellation must fail the prompt");
    assert!(out.is_empty());
    assert!(askpass.cancelled());
    assert_eq!(asked.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn helper_rejects_a_foreign_token_without_answering() {
    let (asked, handler) = recorder(vec![Some("alice")]);
    let askpass = Askpass::start(handler).unwrap();
    let client = askpass.client();
    let (code, out) = run_helper(
        client.clone(),
        "not-the-token".to_string(),
        "Username for 'https://example.com': ",
    )
    .await;
    assert_eq!(code, 1);
    assert!(out.is_empty());
    assert_eq!(asked.load(Ordering::SeqCst), 0);
    assert!(!askpass.cancelled());
}

#[tokio::test]
async fn helper_fails_when_the_socket_is_gone() {
    let (_, handler) = recorder(vec![]);
    let socket = {
        let askpass = Askpass::start(handler).unwrap();
        let socket = askpass.client().socket;
        drop(askpass);
        socket
    };
    let mut out = Vec::new();
    assert_eq!(askpass::helper(&socket, "token", "prompt", &mut out), 1);
    assert!(out.is_empty());
}

#[tokio::test]
async fn prompts_are_classified_before_reaching_the_window() {
    let seen = Arc::new(std::sync::Mutex::new(Vec::<PromptKind>::new()));
    let recorded = Arc::clone(&seen);
    let handler: askpass::Handler = Arc::new(move |_, prompt: Prompt| {
        recorded.lock().unwrap().push(prompt.kind);
        answered(Some("value"))
    });
    let askpass = Askpass::start(handler).unwrap();
    let client = askpass.client();
    for text in [
        "Username for 'https://example.com': ",
        "Password for 'https://example.com': ",
        "Enter passphrase for key '/tmp/key': ",
        "Are you sure you want to continue connecting (yes/no/[fingerprint])?",
        "Unrecognised prompt: ",
    ] {
        let (code, _) = run_helper(client.clone(), client.token.clone(), text).await;
        assert_eq!(code, 0);
    }
    assert_eq!(
        *seen.lock().unwrap(),
        vec![
            PromptKind::Username,
            PromptKind::Password,
            PromptKind::Passphrase,
            PromptKind::HostKey,
            PromptKind::Generic,
        ]
    );
}

async fn fixture() -> (Repo, Repo, tempfile::TempDir) {
    let bare = tempfile::tempdir().unwrap();
    let a = Repo::new().await;
    let b = Repo::new().await;
    a.git("init", &["--bare", bare.path().to_str().unwrap()])
        .await;
    for repo in [&a, &b] {
        repo.git("remote", &["add", "origin", bare.path().to_str().unwrap()])
            .await;
    }
    a.initial().await;
    a.git("branch", &["-M", "main"]).await;
    (a, b, bare)
}

#[tokio::test]
async fn enabled_askpass_leaves_local_transports_working_without_prompts() {
    let (a, b, _bare) = fixture().await;
    let (asked, handler) = recorder(vec![]);
    let mut options = a.options.clone();
    options.askpass = Some(Arc::new(Askpass::start(handler).unwrap()));
    let mut target = b.options.clone();
    target.askpass = Some(Arc::new(Askpass::start(recorder(vec![]).1).unwrap()));

    let push = PushOptions {
        remote: Some("origin".into()),
        branch: Some("main".into()),
        force_with_lease: false,
        tags: false,
        up_to: None,
    };
    let fingerprint = snapshot(&a.runner, &a.options).await.unwrap().fingerprint;
    let outcome = remote::push(&a.runner, &options, &push, &fingerprint)
        .await
        .unwrap();
    assert!(matches!(outcome, RemoteOutcome::Push { success: true, .. }));
    assert_eq!(
        asked.load(Ordering::SeqCst),
        0,
        "no credentials were needed"
    );

    let fingerprint = snapshot(&b.runner, &b.options).await.unwrap().fingerprint;
    let fetched = remote::fetch(&b.runner, &target, Some("origin"), false, &fingerprint)
        .await
        .unwrap();
    assert!(matches!(fetched, RemoteOutcome::Completed));
}

#[tokio::test]
async fn a_cancelled_prompt_ends_the_operation_instead_of_waiting() {
    let (_, handler) = recorder(vec![None]);
    let askpass = Askpass::start(handler).unwrap();
    let client = askpass.client();
    let (code, out) = tokio::time::timeout(
        Duration::from_secs(10),
        run_helper(
            client.clone(),
            client.token.clone(),
            "Password for 'https://user@example.com': ",
        ),
    )
    .await
    .expect("a cancelled prompt must not hang the helper");
    assert_eq!(code, 1);
    assert!(out.is_empty());
}

#[cfg(unix)]
#[tokio::test]
async fn only_the_push_receives_the_helper_and_as_a_bare_path() {
    use gitwood_core::repo::{
        inclusion::{Rule, Selection},
        session::Sessions,
        write::ResultState,
    };
    let (a, _b, _bare) = fixture().await;
    let seen = tempfile::tempdir().unwrap();
    let record = |name: &str| {
        format!(
            "printf '%s|%s' \"${{GIT_ASKPASS-unset}}\" \"${{GITWOOD_ASKPASS_TOKEN-unset}}\" > '{}'",
            seen.path().join(name).display()
        )
    };
    support::script(&a.path(".git/hooks/pre-commit"), &record("commit"));
    support::script(&a.path(".git/hooks/pre-push"), &record("push"));
    let askpass = Arc::new(Askpass::start(recorder(vec![]).1).unwrap());
    let client = askpass.client();
    let lease = Sessions::default()
        .open(a.runner.clone(), a.options.clone())
        .await
        .unwrap();
    a.put("A", "new commit\n");
    let changes = Selection {
        rules: vec![Rule {
            id: "changes".into(),
            included: true,
        }],
        hunks: vec![],
    };
    let push = PushOptions {
        remote: Some("origin".into()),
        branch: Some("main".into()),
        force_with_lease: false,
        tags: false,
        up_to: None,
    };
    let fingerprint = snapshot(&a.runner, &a.options).await.unwrap().fingerprint;
    let result = lease
        .commit_and_push(
            &changes,
            "local",
            false,
            &fingerprint,
            &push,
            None,
            Some(askpass),
        )
        .await
        .unwrap();
    assert!(matches!(result.commit, ResultState::Committed { .. }));
    assert!(matches!(
        result.push,
        Some(Ok(RemoteOutcome::Push { success: true, .. }))
    ));

    let read = |name: &str| std::fs::read_to_string(seen.path().join(name)).unwrap();
    // Commit hooks are local: neither the helper nor the per-operation token reaches them.
    assert_eq!(read("commit"), "unset|unset");
    // Git executes GIT_ASKPASS without a shell, so it must be the bare, existing program.
    let pushed = read("push");
    let (program, token) = pushed.split_once('|').unwrap();
    assert_eq!(std::path::Path::new(program), client.program);
    assert!(client.program.is_file());
    assert_eq!(token, client.token);
}

#[cfg(unix)]
#[tokio::test]
async fn an_endless_request_is_dropped_at_the_size_limit() {
    use std::io::{Read, Write};
    let (asked, handler) = recorder(vec![]);
    let askpass = Askpass::start(handler).unwrap();
    let socket = askpass.client().socket;
    let closed = tokio::task::spawn_blocking(move || {
        let mut stream = std::os::unix::net::UnixStream::connect(socket).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        // No newline ever arrives; the listener must stop reading instead of buffering it.
        let _ = stream.write_all(&vec![b'x'; 64 * 1024]);
        let mut rest = Vec::new();
        stream.read_to_end(&mut rest).map(|_| rest)
    })
    .await
    .unwrap();
    assert!(closed
        .expect("the listener closes the connection")
        .is_empty());
    assert_eq!(asked.load(Ordering::SeqCst), 0);
}

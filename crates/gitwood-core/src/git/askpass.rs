//! Credential prompts from Git and SSH, relayed to the window that started the operation.
//!
//! A remote operation creates one private directory holding a Unix socket. Git and SSH run
//! this same executable as their askpass helper; the helper carries one prompt and one
//! answer, and Gitwood stores neither. The directory is 0700 and the per-operation token is
//! random, so another local process cannot answer a prompt it was not handed.

use super::redact;
use serde::Serialize;
use std::{
    fmt, io,
    io::Write,
    path::{Path, PathBuf},
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio_util::sync::CancellationToken;

/// Environment carrying the per-operation socket and token to the helper process.
pub const SOCKET_ENV: &str = "GITWOOD_ASKPASS_SOCKET";
pub const TOKEN_ENV: &str = "GITWOOD_ASKPASS_TOKEN";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PromptKind {
    Username,
    Password,
    Passphrase,
    HostKey,
    Generic,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Prompt {
    pub kind: PromptKind,
    /// Prompt text with any embedded URL credentials removed.
    pub text: String,
}

/// Classify a provider prompt. Unknown wording stays a masked generic input: Git and
/// OpenSSH versions word their prompts differently, and a mismatch must not fail the
/// operation, only reduce the dialog's help.
pub fn prompt(raw: &str) -> Prompt {
    let kind = if raw.starts_with("Username for ") {
        PromptKind::Username
    } else if raw.starts_with("Password for ") {
        PromptKind::Password
    } else if raw.contains("passphrase for key ") {
        PromptKind::Passphrase
    } else if raw.contains("continue connecting") {
        PromptKind::HostKey
    } else {
        PromptKind::Generic
    };
    Prompt {
        kind,
        text: redact(raw.trim_end()),
    }
}

/// The environment Git and SSH need to reach the running helper.
#[derive(Clone, Debug)]
pub struct Client {
    /// Git and SSH both execute their askpass program directly, without a shell, so this
    /// is a bare path for `GIT_ASKPASS` and `SSH_ASKPASS` alike.
    pub program: PathBuf,
    pub socket: PathBuf,
    pub token: String,
}

pub type Answer = Pin<Box<dyn std::future::Future<Output = Option<String>> + Send>>;
/// Receives the request ordinal and the classified prompt; `None` cancels the operation.
pub type Handler = Arc<dyn Fn(u64, Prompt) -> Answer + Send + Sync>;

/// One operation's prompt listener. Dropping it stops the listener and removes the socket.
pub struct Askpass {
    socket: PathBuf,
    token: String,
    program: PathBuf,
    cancelled: Arc<AtomicBool>,
    shutdown: CancellationToken,
    task: tokio::task::JoinHandle<()>,
    _directory: tempfile::TempDir,
}

impl fmt::Debug for Askpass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Askpass")
            .field("socket", &self.socket)
            .finish_non_exhaustive()
    }
}

impl Drop for Askpass {
    fn drop(&mut self) {
        // A helper blocked on an abandoned prompt sees EOF and reports cancellation.
        self.shutdown.cancel();
        self.task.abort();
    }
}

impl Askpass {
    /// Non-Unix platforms keep `GIT_TERMINAL_PROMPT=0` without a dialog (v0.2 adds them).
    pub fn start(handler: Handler) -> io::Result<Self> {
        #[cfg(unix)]
        {
            unix::start(handler)
        }
        #[cfg(not(unix))]
        {
            let _ = handler;
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "credential prompts are not available on this platform",
            ))
        }
    }

    pub fn client(&self) -> Client {
        Client {
            program: self.program.clone(),
            socket: self.socket.clone(),
            token: self.token.clone(),
        }
    }

    /// True once the user declined a prompt; an abandoned request does not count.
    pub fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

/// Answer one prompt through the socket Git or SSH was given. Returns the helper exit code.
pub fn helper(socket: &Path, token: &str, prompt: &str, out: &mut impl Write) -> i32 {
    #[cfg(unix)]
    {
        unix::helper(socket, token, prompt, out)
    }
    #[cfg(not(unix))]
    {
        let _ = (socket, token, prompt, out);
        1
    }
}

/// Helper mode entry point. Returns `Some(exit code)` when this process was started by
/// Git or SSH as its askpass program, so the caller must not initialize the application.
pub fn run_as_helper() -> Option<i32> {
    let socket = std::env::var_os(SOCKET_ENV)?;
    let token = std::env::var(TOKEN_ENV).ok()?;
    let prompt = std::env::args().nth(1).unwrap_or_default();
    let mut out = io::stdout();
    Some(helper(Path::new(&socket), &token, &prompt, &mut out))
}

#[cfg(unix)]
mod unix {
    use super::{Askpass, Handler};
    use serde::{Deserialize, Serialize};
    use std::{
        io::{self, BufRead, BufReader, Read, Write},
        path::{Path, PathBuf},
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
    };
    use tokio::{
        io::{AsyncBufReadExt, AsyncReadExt, AsyncWrite, AsyncWriteExt},
        net::{UnixListener, UnixStream},
    };
    use tokio_util::sync::CancellationToken;

    /// A prompt is a single line; anything longer is not a prompt Git would send.
    const MAX_REQUEST: usize = 8 * 1024;

    #[derive(Deserialize, Serialize)]
    struct Request {
        token: String,
        prompt: String,
    }

    #[derive(Deserialize, Serialize)]
    struct Response {
        answer: Option<String>,
    }

    pub(super) fn start(handler: Handler) -> io::Result<Askpass> {
        let directory = tempfile::Builder::new()
            .prefix("gitwood-askpass-")
            .tempdir_in(base())?;
        let socket = directory.path().join("s");
        let listener = UnixListener::bind(&socket)?;
        let program = std::env::current_exe()?;
        let token = random_token()?;
        let cancelled = Arc::new(AtomicBool::new(false));
        let shutdown = CancellationToken::new();
        let task = tokio::spawn(serve(
            listener,
            handler,
            token.clone(),
            cancelled.clone(),
            shutdown.clone(),
        ));
        Ok(Askpass {
            socket,
            token,
            program,
            cancelled,
            shutdown,
            task,
            _directory: directory,
        })
    }

    /// `sun_path` is 104 bytes on macOS; keep the socket out of long per-user temp paths.
    fn base() -> PathBuf {
        let temp = std::env::temp_dir();
        if temp.as_os_str().len() + 32 <= 100 {
            temp
        } else {
            PathBuf::from("/tmp")
        }
    }

    /// `getrandom` is only a transitive dependency, so read the kernel source directly.
    fn random_token() -> io::Result<String> {
        let mut bytes = [0u8; 16];
        std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
        let mut token = String::with_capacity(32);
        for byte in bytes {
            token.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('0'));
            token.push(char::from_digit(u32::from(byte & 0xf), 16).unwrap_or('0'));
        }
        Ok(token)
    }

    async fn serve(
        listener: UnixListener,
        handler: Handler,
        token: String,
        cancelled: Arc<AtomicBool>,
        shutdown: CancellationToken,
    ) {
        let mut next = 0u64;
        loop {
            let accepted = tokio::select! {
                _ = shutdown.cancelled() => return,
                accepted = listener.accept() => accepted,
            };
            let Ok((stream, _)) = accepted else { return };
            next += 1;
            let (handler, token, cancelled, shutdown) = (
                handler.clone(),
                token.clone(),
                cancelled.clone(),
                shutdown.clone(),
            );
            tokio::spawn(async move {
                answer(stream, next, handler, token, cancelled, shutdown).await;
            });
        }
    }

    async fn answer(
        mut stream: UnixStream,
        id: u64,
        handler: Handler,
        token: String,
        cancelled: Arc<AtomicBool>,
        shutdown: CancellationToken,
    ) {
        let mut line = String::new();
        {
            // Bound the read itself: a peer that never sends a newline must not grow memory.
            let mut reader = tokio::io::BufReader::new((&mut stream).take(MAX_REQUEST as u64 + 1));
            let read = tokio::select! {
                _ = shutdown.cancelled() => return,
                read = reader.read_line(&mut line) => read,
            };
            if read.is_err() || line.len() > MAX_REQUEST {
                return;
            }
        }
        let Ok(request) = serde_json::from_str::<Request>(&line) else {
            return;
        };
        if request.token != token {
            // Wrong token: reveal nothing, not even whether a prompt is pending.
            let _ = respond(&mut stream, None).await;
            return;
        }
        let answered = tokio::select! {
            _ = shutdown.cancelled() => None,
            answer = handler(id, super::prompt(&request.prompt)) => Some(answer),
        };
        // A dropped request is not a user cancellation: close the socket instead, which the
        // helper reports as a failed prompt without claiming the user declined.
        let Some(answer) = answered else { return };
        if answer.is_none() {
            cancelled.store(true, Ordering::SeqCst);
        }
        let _ = respond(&mut stream, answer).await;
    }

    async fn respond(
        stream: &mut (impl AsyncWrite + Unpin),
        answer: Option<String>,
    ) -> io::Result<()> {
        let body = serde_json::to_vec(&Response { answer })
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        stream.write_all(&body).await?;
        stream.write_all(b"\n").await?;
        stream.flush().await
    }

    pub(super) fn helper(socket: &Path, token: &str, prompt: &str, out: &mut impl Write) -> i32 {
        let Ok(mut stream) = std::os::unix::net::UnixStream::connect(socket) else {
            return 1;
        };
        let request = Request {
            token: token.into(),
            prompt: prompt.into(),
        };
        let Ok(body) = serde_json::to_vec(&request) else {
            return 1;
        };
        if stream.write_all(&body).is_err() || stream.write_all(b"\n").is_err() {
            return 1;
        }
        let mut line = String::new();
        if BufReader::new(stream).read_line(&mut line).is_err() {
            return 1;
        }
        let Ok(response) = serde_json::from_str::<Response>(&line) else {
            return 1;
        };
        match response.answer {
            Some(answer) if writeln!(out, "{answer}").is_ok() => 0,
            _ => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_the_prompts_git_and_ssh_emit() {
        assert_eq!(
            prompt("Username for 'https://github.com': ").kind,
            PromptKind::Username
        );
        assert_eq!(
            prompt("Password for 'https://user@example.com': ").kind,
            PromptKind::Password
        );
        assert_eq!(
            prompt("Enter passphrase for key '/Users/me/.ssh/id_ed25519': ").kind,
            PromptKind::Passphrase
        );
        assert_eq!(
            prompt("Are you sure you want to continue connecting (yes/no/[fingerprint])?").kind,
            PromptKind::HostKey
        );
        assert_eq!(prompt("Something new: ").kind, PromptKind::Generic);
    }

    #[test]
    fn strips_trailing_space_and_embedded_credentials() {
        assert_eq!(
            prompt("Username for 'https://github.com': ").text,
            "Username for 'https://github.com':"
        );
        let password = prompt("Password for 'https://user:secret@example.com': ");
        assert_eq!(password.kind, PromptKind::Password);
        assert!(!password.text.contains("secret"));
        assert!(password.text.contains("[redacted]"));
    }
}

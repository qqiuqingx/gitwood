use super::{
    askpass, command::read_config_arguments, filters, GitCommand, GitError, GitVersion, RunOptions,
};
use std::{
    path::{Path, PathBuf},
    process::{ExitStatus, Stdio},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
};

#[derive(Clone, Debug)]
pub struct GitRunner {
    pub(super) path: PathBuf,
    pub(super) version: GitVersion,
}
#[derive(Debug)]
pub struct GitOutput {
    pub code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: String,
}

impl GitRunner {
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn version(&self) -> GitVersion {
        self.version
    }

    pub async fn run(
        &self,
        command: impl Into<GitCommand>,
        options: &RunOptions,
    ) -> Result<GitOutput, GitError> {
        let output = self.run_report(command, options).await?;
        if output.code != Some(0) {
            return Err(GitError::Exit {
                code: output.code,
                stderr: output.stderr,
            });
        }
        Ok(output)
    }

    /// Preserve machine-readable stdout even when Git reports a failed ref update.
    pub async fn run_report(
        &self,
        command: impl Into<GitCommand>,
        options: &RunOptions,
    ) -> Result<GitOutput, GitError> {
        let command = command.into();
        let mut args = command.arguments()?;
        let read = matches!(command, GitCommand::Read(_));
        let transport = matches!(&command, GitCommand::Write(write) if write.is_transport());
        let timeout = options
            .timeout
            .or_else(|| read.then(|| Duration::from_secs(30)));
        if read {
            let mut query = self.process(options, true, false);
            query.arg("--no-pager").args(read_config_arguments()).args([
                "config",
                "--show-scope",
                "--null",
                "--name-only",
                "--get-regexp",
                "^filter\\.",
            ]);
            // config returns 1 for no matching keys, not a failed safety query.
            let output = match execute_report(query, options, &self.path, timeout).await {
                Ok(output) if output.code == Some(0) || output.code == Some(1) => output.stdout,
                Ok(output) => {
                    return Err(GitError::Exit {
                        code: output.code,
                        stderr: output.stderr,
                    })
                }
                Err(GitError::Exit { code: Some(1), .. }) => Vec::new(),
                Err(error) => return Err(error),
            };
            args.splice(1..1, filters::overrides(&output)?);
        }
        let mut process = self.process(options, read, transport);
        process.args(args);
        execute_report(process, options, &self.path, timeout).await
    }

    fn process(&self, options: &RunOptions, read: bool, transport: bool) -> Command {
        let mut process = Command::new(&self.path);
        process
            .current_dir(&options.cwd)
            .envs(&options.env)
            .env("LC_ALL", "C")
            .env("GIT_OPTIONAL_LOCKS", "0")
            // No operation may block on a terminal: scans run without one, and a write
            // started from Finder would otherwise wait for input nobody can type.
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if read {
            process.env("GIT_ALLOW_PROTOCOL", "");
        } else if let Some(client) = options
            .askpass
            .as_deref()
            .filter(|_| transport)
            .map(|a| a.client())
        {
            // Only fetch and push reach a remote. Local steps of the same operation (commit
            // hooks, stash, merge, rebase) never see the helper or its per-operation token.
            // Git asks the credential helper first and only falls back to GIT_ASKPASS, so
            // keychain and agent configuration keep precedence; nothing here edits them.
            process
                .env("GIT_ASKPASS", &client.program)
                .env("SSH_ASKPASS", &client.program)
                .env("SSH_ASKPASS_REQUIRE", "force")
                .env(askpass::SOCKET_ENV, client.socket)
                .env(askpass::TOKEN_ENV, client.token);
            // OpenSSH before 8.4 refuses a helper without DISPLAY. macOS has no X server and
            // ships a newer OpenSSH, so a fake display there would only reach X-aware tools.
            #[cfg(not(target_os = "macos"))]
            if !options.env.contains_key(std::ffi::OsStr::new("DISPLAY"))
                && std::env::var_os("DISPLAY").is_none()
            {
                process.env("DISPLAY", ":0");
            }
        }
        process
    }
}

pub(super) async fn execute(
    command: Command,
    options: &RunOptions,
    path: &Path,
    timeout: Option<Duration>,
) -> Result<GitOutput, GitError> {
    let output = execute_report(command, options, path, timeout).await?;
    if output.code != Some(0) {
        return Err(GitError::Exit {
            code: output.code,
            stderr: output.stderr,
        });
    }
    Ok(output)
}

async fn execute_report(
    mut command: Command,
    options: &RunOptions,
    path: &Path,
    timeout: Option<Duration>,
) -> Result<GitOutput, GitError> {
    if options.cancellation.is_cancelled() {
        return Err(GitError::Cancelled);
    }
    #[cfg(unix)]
    command.process_group(0);
    let mut child = command.spawn().map_err(|source| GitError::Spawn {
        path: path.into(),
        source,
    })?;
    #[cfg(unix)]
    let group = child.id().and_then(|id| i32::try_from(id).ok());
    let mut stdin = child.stdin.take().expect("piped stdin");
    let mut stdout = child.stdout.take().expect("piped stdout");
    let mut stderr = child.stderr.take().expect("piped stderr");
    let mut out = Vec::new();
    let mut err = Vec::new();
    let result = tokio::select! {
        biased;
        _ = options.cancellation.cancelled() => Err(GitError::Cancelled),
        limit = async {
            match timeout {
                Some(limit) => { tokio::time::sleep(limit).await; limit }
                None => std::future::pending().await,
            }
        } => Err(GitError::Timeout(limit)),
        result = async {
            let input = async {
                // Git can reject input before consuming it; its exit status is authoritative.
                match stdin.write_all(&options.stdin).await {
                    Err(e) if e.kind() != std::io::ErrorKind::BrokenPipe => return Err(e),
                    _ => {}
                }
                drop(stdin); Ok(())
            };
            let (status, _, _, _) = tokio::try_join!(child.wait(), input,
                stdout.read_to_end(&mut out), read_progress(&mut stderr, &mut err, options))?;
            Ok::<ExitStatus, std::io::Error>(status)
        } => result.map_err(GitError::Io),
    };
    if result.is_err() {
        #[cfg(unix)]
        if let Some(group) = group {
            signal_group(group, 15); // SIGTERM: give hooks/signers a cleanup window.
            tokio::time::sleep(Duration::from_millis(200)).await;
            signal_group(group, 9); // SIGKILL also reaches descendants ignoring SIGTERM.
        }
        let _ = child.kill().await;
    }
    classify(result?, out, err, options)
}

#[cfg(unix)]
fn signal_group(group: i32, signal: i32) {
    unsafe extern "C" {
        fn kill(pid: i32, signal: i32) -> i32;
    }
    // SAFETY: process_group(0) made this child's PID its private PGID. A negative
    // PID targets that group only. ESRCH means it already exited. No new dependency.
    unsafe {
        kill(-group, signal);
    }
}

fn classify(
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    options: &RunOptions,
) -> Result<GitOutput, GitError> {
    let stderr = super::redact(&String::from_utf8_lossy(&stderr));
    if stderr.contains("detected dubious ownership") {
        let path = stderr
            .split("repository at '")
            .nth(1)
            .and_then(|s| s.split("'\n").next())
            .map(PathBuf::from)
            .unwrap_or_else(|| options.cwd.clone());
        return Err(GitError::UnsafeRepository { path, stderr });
    }
    Ok(GitOutput {
        code: status.code(),
        stdout,
        stderr,
    })
}

async fn read_progress(
    reader: &mut (impl tokio::io::AsyncRead + Unpin),
    bytes: &mut Vec<u8>,
    options: &RunOptions,
) -> std::io::Result<usize> {
    let mut buffer = [0u8; 4096];
    loop {
        tokio::select! {
            count=reader.read(&mut buffer)=>{
                let count=count?;
                if count==0 {return Ok(bytes.len());}
                bytes.extend_from_slice(&buffer[..count]);
                // Emit complete CR/LF-delimited records only: credentials split
                // across pipe reads must never escape redaction.
                if let Some(sender)=&options.progress {
                    if let Some(end)=bytes.iter().rposition(|b|matches!(b,b'\r'|b'\n')) {
                        let start=bytes[..bytes.len()-count].iter().rposition(|b|matches!(b,b'\r'|b'\n')).map_or(0,|n|n+1);
                        if end>=start {let _=sender.send(super::redact(&String::from_utf8_lossy(&bytes[start..=end])));}
                    }
                }
            }
            _=tokio::time::sleep(options.heartbeat_interval.max(Duration::from_millis(1))), if options.progress.is_some()=>{
                if let Some(sender)=&options.progress {let _=sender.send("remote.noOutput".into());}
            }
        }
    }
}

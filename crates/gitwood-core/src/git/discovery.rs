use super::{runner::execute, GitError, GitRunner, GitVersion, RunOptions};
use std::{
    env,
    path::{Path, PathBuf},
    process::Stdio,
};
use tokio::process::Command;

impl GitRunner {
    /// An explicit executable is authoritative: invalid/old overrides are reported,
    /// rather than silently selecting a different installation.
    pub async fn discover(preferred: Option<&Path>) -> Result<Self, GitError> {
        if let Some(path) = preferred {
            return probe(path).await;
        }
        for path in candidates() {
            if !path.is_file() {
                continue;
            }
            match probe(&path).await {
                Err(GitError::NotFound(_) | GitError::Spawn { .. }) => continue,
                result => return result,
            }
        }
        Err(GitError::NotFound(None))
    }
}

async fn probe(path: &Path) -> Result<GitRunner, GitError> {
    if !path.is_file() || !platform_usable(path).await {
        return Err(GitError::NotFound(Some(path.into())));
    }
    let path = std::fs::canonicalize(path)?;
    let options = RunOptions::new(env::current_dir()?);
    let mut command = Command::new(&path);
    command
        .args(["--version"])
        .env("LC_ALL", "C")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let output = execute(
        command,
        &options,
        &path,
        Some(std::time::Duration::from_secs(30)),
    )
    .await?;
    let version = GitVersion::parse(&output.stdout)?.require_supported()?;
    Ok(GitRunner { path, version })
}

fn candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    #[cfg(target_os = "macos")]
    paths.extend(["/opt/homebrew/bin/git".into(), "/usr/local/bin/git".into()]);
    #[cfg(all(unix, not(target_os = "macos")))]
    paths.extend([
        "/usr/local/bin/git".into(),
        "/usr/bin/git".into(),
        "/bin/git".into(),
    ]);
    #[cfg(windows)]
    for key in ["ProgramFiles", "ProgramFiles(x86)", "LocalAppData"] {
        if let Some(root) = env::var_os(key) {
            for suffix in [
                "Git/cmd/git.exe",
                "Git/bin/git.exe",
                "Programs/Git/cmd/git.exe",
            ] {
                paths.push(PathBuf::from(&root).join(suffix));
            }
        }
    }
    if let Some(path) = env::var_os("PATH") {
        paths.extend(
            env::split_paths(&path).map(|p| p.join(if cfg!(windows) { "git.exe" } else { "git" })),
        );
    }
    #[cfg(target_os = "macos")]
    {
        // Keep the Apple stub last, even when /usr/bin was earlier in PATH.
        paths.retain(|p| p != Path::new("/usr/bin/git"));
        paths.push("/usr/bin/git".into());
    }
    paths
}

#[cfg(target_os = "macos")]
async fn platform_usable(path: &Path) -> bool {
    let resolved = std::fs::canonicalize(path).unwrap_or_else(|_| path.into());
    if resolved != Path::new("/usr/bin/git") {
        return true;
    }
    let mut command = Command::new("/usr/bin/xcode-select");
    command.arg("-p").kill_on_drop(true);
    matches!(tokio::time::timeout(std::time::Duration::from_secs(5), command.output()).await,
        Ok(Ok(output)) if output.status.success())
}

#[cfg(not(target_os = "macos"))]
async fn platform_usable(_: &Path) -> bool {
    true
}

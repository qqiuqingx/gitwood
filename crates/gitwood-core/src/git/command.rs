use super::{askpass::Askpass, CancellationToken, GitError};
use std::{collections::BTreeMap, ffi::OsString, path::PathBuf, sync::Arc, time::Duration};

/// Local, non-networking operations only. Aliases, config mutation and remote commands
/// cannot be represented as a background read, except argument-free remote-name enumeration.
#[derive(Clone, Copy, Debug)]
pub enum ReadOperation {
    Status,
    Diff,
    Log,
    Show,
    RevParse,
    LsFiles,
    LsTree,
    DiffTree,
    ForEachRef,
    CatFile,
    CheckRefFormat,
    RemoteNames,
    RemoteUrl,
    HostingContains,
    HostingRef,
    MergeBase,
    StashShow,
    StashList,
    Blame,
}

impl ReadOperation {
    fn name(self) -> &'static str {
        match self {
            Self::Status => "status",
            Self::Diff => "diff",
            Self::Log => "log",
            Self::Show => "show",
            Self::RevParse => "rev-parse",
            Self::LsFiles => "ls-files",
            Self::LsTree => "ls-tree",
            Self::DiffTree => "diff-tree",
            Self::ForEachRef => "for-each-ref",
            Self::CatFile => "cat-file",
            Self::CheckRefFormat => "check-ref-format",
            Self::RemoteNames => "remote",
            Self::RemoteUrl => "remote",
            Self::HostingContains | Self::HostingRef => "for-each-ref",
            Self::MergeBase => "merge-base",
            Self::StashShow => "stash",
            Self::StashList => "stash",
            Self::Blame => "blame",
        }
    }
    fn is_diff(self) -> bool {
        matches!(self, Self::Diff | Self::Log | Self::Show | Self::DiffTree)
    }
}

#[derive(Clone, Debug)]
pub struct ReadCommand {
    operation: ReadOperation,
    args: Vec<OsString>,
}
#[derive(Clone, Debug)]
pub struct WriteCommand {
    name: OsString,
    args: Vec<OsString>,
}
#[derive(Clone, Debug)]
pub enum GitCommand {
    Read(ReadCommand),
    Write(WriteCommand),
}

impl ReadCommand {
    pub fn new(operation: ReadOperation) -> Self {
        Self {
            operation,
            args: Vec::new(),
        }
    }
    pub fn args(mut self, args: impl IntoIterator<Item = impl Into<OsString>>) -> Self {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }
}
impl WriteCommand {
    /// Commands that contact a remote and may need credential prompts.
    pub(super) fn is_transport(&self) -> bool {
        self.name == "fetch" || self.name == "push"
    }
    pub fn new(name: impl Into<OsString>) -> Self {
        Self {
            name: name.into(),
            args: Vec::new(),
        }
    }
    pub fn args(mut self, args: impl IntoIterator<Item = impl Into<OsString>>) -> Self {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }
}
impl From<ReadCommand> for GitCommand {
    fn from(value: ReadCommand) -> Self {
        Self::Read(value)
    }
}
impl From<WriteCommand> for GitCommand {
    fn from(value: WriteCommand) -> Self {
        Self::Write(value)
    }
}

impl GitCommand {
    pub(super) fn arguments(&self) -> Result<Vec<OsString>, GitError> {
        let mut result: Vec<OsString> = vec!["--no-pager".into()];
        match self {
            Self::Read(read) => {
                validate_read(read)?;
                result.extend(read_config_arguments());
                result.push(read.operation.name().into());
                if read.operation.is_diff() {
                    result.extend(["--no-ext-diff".into(), "--no-textconv".into()]);
                }
                if matches!(read.operation, ReadOperation::Blame) {
                    result.push("--no-textconv".into());
                }
                if read.operation.is_diff() || matches!(read.operation, ReadOperation::Status) {
                    result.push("--ignore-submodules=all".into());
                }
                result.extend(read.args.clone());
            }
            Self::Write(write) => {
                if write.name.is_empty() || write.name.to_string_lossy().starts_with('-') {
                    return Err(GitError::InvalidCommand("expected a Git subcommand".into()));
                }
                result.push(write.name.clone());
                result.extend(write.args.clone());
            }
        }
        Ok(result)
    }
}

pub(super) fn read_config_arguments() -> Vec<OsString> {
    [
        "--no-optional-locks",
        "-c",
        "core.fsmonitor=false",
        "-c",
        "core.untrackedCache=false",
        "-c",
        "core.sshCommand=",
        "-c",
        "submodule.recurse=false",
        "-c",
        "log.showSignature=false",
        "-c",
        "format.pretty=medium",
        "-c",
        "log.diffMerges=separate",
    ]
    .map(Into::into)
    .into()
}

fn validate_read(read: &ReadCommand) -> Result<(), GitError> {
    if matches!(read.operation, ReadOperation::StashList) {
        let args: Vec<_> = read.args.iter().map(|arg| arg.to_string_lossy()).collect();
        if args.len() != 2 || args[0] != "list" || args[1] != "--format=%gd%x00%H%x00%gs" {
            return Err(GitError::InvalidCommand(
                "invalid read-only stash list".into(),
            ));
        }
        return Ok(());
    }
    if matches!(read.operation, ReadOperation::StashShow) {
        let args: Vec<_> = read.args.iter().map(|arg| arg.to_string_lossy()).collect();
        if args.len() != 9
            || args[0] != "show"
            || args[1] != "--patch"
            || args[2] != "--binary"
            || args[3] != "--include-untracked"
            || args[4] != "--no-color"
            || args[5] != "--no-ext-diff"
            || args[6] != "--no-textconv"
            || args[7] != "--ignore-submodules=all"
            || !args[8].bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(GitError::InvalidCommand(
                "invalid read-only stash show".into(),
            ));
        }
        return Ok(());
    }
    if matches!(read.operation, ReadOperation::RemoteNames) && !read.args.is_empty() {
        return Err(GitError::InvalidCommand(
            "remote names read accepts no arguments".into(),
        ));
    }
    if matches!(read.operation, ReadOperation::RemoteUrl) {
        let args: Vec<_> = read.args.iter().map(|arg| arg.to_string_lossy()).collect();
        if args.len() != 2 || args[0] != "get-url" || !valid_remote_name(&args[1]) {
            return Err(GitError::InvalidCommand("invalid remote URL lookup".into()));
        }
    }
    if matches!(read.operation, ReadOperation::HostingContains) {
        return validate_hosting_contains(read);
    }
    if matches!(read.operation, ReadOperation::HostingRef) {
        return validate_hosting_ref(read);
    }
    let mut format_value = false;
    for arg in read.args.iter().take_while(|a| *a != "--") {
        let arg = arg.to_string_lossy();
        let option = arg.split('=').next().unwrap_or_default();
        let unsafe_option = [
            "--ext-diff",
            "--textconv",
            "--output",
            "--open-files-in-pager",
            "--recurse-submodules",
            "--filters",
            "--show-signature",
            "--alternate-refs",
            "--remerge-diff",
            "--no-ignore-submodules",
        ]
        .iter()
        .any(|flag| option.starts_with("--") && flag.starts_with(option));
        let unsafe_submodules = option.starts_with("--")
            && "--ignore-submodules".starts_with(option)
            && arg.split_once('=').map(|(_, v)| v) != Some("all");
        // status has no --no-textconv switch; its verbose diff can run textconv.
        // Normal status/porcelain reads do not enable that internal diff path.
        let unsafe_status_verbose = matches!(read.operation, ReadOperation::Status)
            && ((option.starts_with("--") && "--verbose".starts_with(option))
                || (arg.starts_with('-') && !arg.starts_with("--") && arg.contains('v')));
        let unsafe_merges = option.starts_with("--")
            && "--diff-merges".starts_with(option)
            && !matches!(
                arg.split_once('=').map(|(_, v)| v),
                Some("off" | "none" | "first-parent" | "separate" | "combined" | "dense-combined")
            );
        let is_format = option.starts_with("--")
            && ["--format", "--pretty"]
                .iter()
                .any(|flag| flag.starts_with(option));
        let value = if format_value {
            Some(arg.as_ref())
        } else if is_format {
            arg.split_once('=').map(|(_, v)| v)
        } else {
            None
        };
        if unsafe_option
            || unsafe_submodules
            || unsafe_status_verbose
            || unsafe_merges
            || value.is_some_and(|v| !safe_format(v))
            || (matches!(read.operation, ReadOperation::ForEachRef)
                && value.is_some_and(|v| v.contains("%(signature")))
        {
            return Err(GitError::InvalidCommand(format!(
                "unsafe background option: {arg}"
            )));
        }
        format_value = is_format && !arg.contains('=');
    }
    Ok(())
}

fn valid_remote_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && !name.starts_with('-')
        && !name.starts_with('.')
        && !name.ends_with('.')
        && !name.contains("..")
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
}

fn validate_hosting_contains(read: &ReadCommand) -> Result<(), GitError> {
    let args: Vec<_> = read.args.iter().map(|arg| arg.to_string_lossy()).collect();
    if args.len() != 4
        || args[0] != "--count=1"
        || args[1] != "--format=%(refname)"
        || !args[2]
            .strip_prefix("--contains=")
            .is_some_and(valid_object_id)
        || !valid_remote_prefix(&args[3])
    {
        return Err(GitError::InvalidCommand(
            "invalid published revision lookup".into(),
        ));
    }
    Ok(())
}

fn validate_hosting_ref(read: &ReadCommand) -> Result<(), GitError> {
    let args: Vec<_> = read.args.iter().map(|arg| arg.to_string_lossy()).collect();
    if args.len() != 3
        || args[0] != "--count=1"
        || args[1] != "--format=%(objectname)"
        || !valid_hosting_refname(&args[2])
    {
        return Err(GitError::InvalidCommand(
            "invalid hosting ref lookup".into(),
        ));
    }
    Ok(())
}

fn valid_remote_prefix(value: &str) -> bool {
    value
        .strip_prefix("refs/remotes/")
        .and_then(|rest| rest.strip_suffix('/'))
        .is_some_and(valid_remote_name)
}

fn valid_hosting_refname(value: &str) -> bool {
    if let Some(branch) = value.strip_prefix("refs/heads/") {
        return valid_branch_name(branch);
    }
    let Some((remote, branch)) = value
        .strip_prefix("refs/remotes/")
        .and_then(|rest| rest.split_once('/'))
    else {
        return false;
    };
    valid_remote_name(remote) && valid_branch_name(branch)
}

fn valid_branch_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 1024
        && !value.contains("..")
        && !value.contains("@{")
        && !value.ends_with('.')
        && !value
            .chars()
            .any(|ch| ch.is_control() || ch.is_whitespace() || "~^:?*[\\".contains(ch))
        && value.split('/').all(|part| {
            !part.is_empty() && !part.starts_with('.') && !part.ends_with(".lock") && part != "@"
        })
}

fn valid_object_id(value: &str) -> bool {
    [40, 64].contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn safe_format(value: &str) -> bool {
    // Named repository pretty aliases can hide signature placeholders. Only exact
    // builtins or inline formats are accepted. Include Git's whitespace modifiers.
    if ["%G", "%+G", "%-G", "% G"]
        .iter()
        .any(|token| value.contains(token))
    {
        return false;
    }
    value.contains('%')
        || value.starts_with("format:")
        || value.starts_with("tformat:")
        || matches!(
            value,
            "oneline"
                | "short"
                | "medium"
                | "full"
                | "fuller"
                | "reference"
                | "email"
                | "mboxrd"
                | "raw"
        )
}

/// Extra environment is applied before mandatory safety values. No global configuration
/// is erased. Callers must not supply repository-routing variables from untrusted input.
#[derive(Clone, Debug)]
pub struct RunOptions {
    pub cwd: PathBuf,
    pub env: BTreeMap<OsString, OsString>,
    pub stdin: Vec<u8>,
    /// None selects the policy default: reads 30 seconds, writes unlimited.
    /// Some applies the explicit limit to either kind of operation.
    pub timeout: Option<Duration>,
    pub cancellation: CancellationToken,
    pub heartbeat_interval: std::time::Duration,
    pub progress: Option<tokio::sync::mpsc::UnboundedSender<String>>,
    /// Answers Git/SSH credential prompts in the project window. The runner hands it only
    /// to `fetch` and `push`; local commands run with the same options never see it.
    pub askpass: Option<Arc<Askpass>>,
}
impl RunOptions {
    pub fn new(cwd: impl Into<PathBuf>) -> Self {
        Self {
            cwd: cwd.into(),
            env: BTreeMap::new(),
            stdin: Vec::new(),
            timeout: None,
            cancellation: CancellationToken::new(),
            heartbeat_interval: std::time::Duration::from_secs(15),
            progress: None,
            askpass: None,
        }
    }
}

#[cfg(test)]
mod hosting_read_tests {
    use super::{GitCommand, ReadCommand, ReadOperation};

    #[test]
    fn hosting_ref_reads_are_fixed_and_revision_cannot_become_an_option() {
        let revision = "a".repeat(40);
        let contains = format!("--contains={revision}");
        let command = ReadCommand::new(ReadOperation::HostingContains).args([
            "--count=1",
            "--format=%(refname)",
            contains.as_str(),
            "refs/remotes/origin/",
        ]);
        let args = GitCommand::from(command).arguments().unwrap();
        let tail: Vec<String> = args[args.len() - 5..]
            .iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            tail,
            vec![
                "for-each-ref".to_owned(),
                "--count=1".to_owned(),
                "--format=%(refname)".to_owned(),
                contains,
                "refs/remotes/origin/".to_owned(),
            ]
        );

        let invalid = ReadCommand::new(ReadOperation::HostingContains).args([
            "--count=1",
            "--format=%(refname)",
            "--contains=--help",
            "refs/remotes/origin/",
        ]);
        assert!(GitCommand::from(invalid).arguments().is_err());
    }

    #[test]
    fn hosting_ref_lookup_rejects_patterns_and_path_injection() {
        let valid = ReadCommand::new(ReadOperation::HostingRef).args([
            "--count=1",
            "--format=%(objectname)",
            "refs/remotes/origin/release/one",
        ]);
        assert!(GitCommand::from(valid).arguments().is_ok());
        for value in [
            "refs/remotes/origin/*",
            "refs/remotes/origin/../other",
            "refs/remotes/-origin/main",
        ] {
            let read = ReadCommand::new(ReadOperation::HostingRef).args([
                "--count=1",
                "--format=%(objectname)",
                value,
            ]);
            assert!(GitCommand::from(read).arguments().is_err(), "{value}");
        }
    }
}

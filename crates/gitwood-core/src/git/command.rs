use super::{CancellationToken, GitError};
use std::{collections::BTreeMap, ffi::OsString, path::PathBuf, time::Duration};

/// Local, non-networking operations only. Aliases, config mutation and remote commands
/// cannot be represented as a background read.
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
        }
    }
}

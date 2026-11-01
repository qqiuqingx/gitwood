//! Bounded detail reads and literal filter argument construction.
use super::{CommitGraph, CommitId, EdgeId, GraphLayout};
use crate::{
    git::{GitError, GitRunner, ReadCommand, ReadOperation, RunOptions},
    repo::{invalid, text, RepoPath},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
};
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Filter {
    pub text: Option<String>,
    pub branch: Option<String>,
    pub author: Option<String>,
    pub since: Option<String>,
    pub until: Option<String>,
    pub path: Option<RepoPath>,
    /// IDEA's `.*` toggle: `--extended-regexp` instead of `--fixed-strings`.
    #[serde(default)]
    pub regex: bool,
    /// IDEA's `Cc` toggle; off adds `--regexp-ignore-case` (GitLogProvider).
    #[serde(default)]
    pub match_case: bool,
}
impl Filter {
    pub fn arguments(&self) -> Result<Vec<OsString>, GitError> {
        let mut args: Vec<OsString> = [
            "--format=%H %P",
            "--topo-order",
            "--no-decorate",
            "--no-notes",
            "--no-color",
            "--full-history",
        ]
        .map(Into::into)
        .into();
        for (key, value) in [
            ("author", &self.author),
            ("since", &self.since),
            ("until", &self.until),
        ] {
            if let Some(value) = value {
                args.push(format!("--{key}={value}").into());
            }
        }
        if let Some(branch) = &self.branch {
            if branch != "HEAD"
                && !branch.starts_with("refs/heads/")
                && !branch.starts_with("refs/remotes/")
                && !branch.starts_with("refs/tags/")
            {
                return Err(invalid("expected full branch ref"));
            }
            args.push(branch.into());
        } else {
            args.push("--all".into());
        }
        args.push("--".into());
        if let Some(path) = &self.path {
            args.push(crate::repo::os(path.bytes.clone()));
        }
        Ok(args)
    }
}
/// Filter visibility on the complete DAG, retaining nearest matching ancestors.
/// IDEA calls these collapsed-fragment edges DOTTED and paints them DASHED:
/// FilteredController.kt, DottedFilterEdgesGenerator.kt, EdgePrintElementImpl.kt
/// (platform/vcs-log at 0698a5fe297dfa08069f015cc423a6a85ed7fc60).
pub async fn layout(
    runner: &GitRunner,
    options: &RunOptions,
    filter: &Filter,
) -> Result<GraphLayout, GitError> {
    let mut options = options.clone();
    options
        .env
        .insert("GIT_LITERAL_PATHSPECS".into(), "1".into());
    let base = Filter {
        branch: filter.branch.clone(),
        ..Filter::default()
    };
    let arguments = log_arguments(runner, &options, &base).await?;
    let bytes = read_log(runner, &options, arguments).await?;
    let graph = CommitGraph::parse_log(&bytes).map_err(|error| invalid(&error.to_string()))?;
    if filter.text.is_none()
        && filter.author.is_none()
        && filter.since.is_none()
        && filter.until.is_none()
        && filter.path.is_none()
    {
        return GraphLayout::new(graph, &[]).map_err(|error| invalid(&error.to_string()));
    }
    let selected = matching_hashes(runner, &options, filter).await?;
    collapse_graph(&graph, &selected)
}

/// Rows contained in the current branch, for IDEA's CurrentBranchHighlighter.
/// Empty (no highlighting) when HEAD is detached or unborn, or when the filter
/// already shows only HEAD or the current branch (CurrentBranchHighlighter.update).
pub async fn current_branch_rows(
    runner: &GitRunner,
    options: &RunOptions,
    filter: &Filter,
    layout: &GraphLayout,
) -> Result<Vec<bool>, GitError> {
    let branch = crate::repo::status::read(runner, options).await?.branch;
    let Some(head) = branch
        .head
        .filter(|head| head != "(detached)" && branch.oid.as_deref() != Some("(initial)"))
    else {
        return Ok(Vec::new());
    };
    if let Some(filtered) = &filter.branch {
        if filtered == "HEAD" || *filtered == format!("refs/heads/{head}") {
            return Ok(Vec::new());
        }
    }
    let output = runner
        .run(
            ReadCommand::new(ReadOperation::Log).args(["--format=%H", "HEAD", "--"]),
            options,
        )
        .await?;
    let reachable = hashes(&output.stdout);
    Ok((0..layout.graph().len())
        .map(|row| {
            layout
                .graph()
                .hash(CommitId(row as u32))
                .is_some_and(|hash| reachable.contains(&hash))
        })
        .collect())
}

async fn log_arguments(
    runner: &GitRunner,
    options: &RunOptions,
    filter: &Filter,
) -> Result<Vec<OsString>, GitError> {
    let mut arguments = filter.arguments()?;
    // Path-limited %P is rewritten only with --parents. Visibility uses the Git
    // path walk; collapsed connectivity always uses the original complete DAG.
    arguments.insert(0, "--parents".into());
    if filter.branch.is_none() {
        let branch = crate::repo::status::read(runner, options).await?.branch;
        if let Some(oid) = branch.oid.filter(|oid| oid != "(initial)") {
            let separator = arguments.iter().position(|arg| arg == "--").unwrap();
            arguments.insert(separator, oid.into());
        }
    }
    Ok(arguments)
}

async fn read_log(
    runner: &GitRunner,
    options: &RunOptions,
    arguments: Vec<OsString>,
) -> Result<Vec<u8>, GitError> {
    Ok(runner
        .run(
            ReadCommand::new(ReadOperation::Log).args(arguments),
            options,
        )
        .await?
        .stdout)
}

fn hashes(bytes: &[u8]) -> HashSet<String> {
    text(bytes)
        .lines()
        .filter_map(|line| line.split_whitespace().next().map(String::from))
        .collect()
}

async fn matching_hashes(
    runner: &GitRunner,
    options: &RunOptions,
    filter: &Filter,
) -> Result<HashSet<String>, GitError> {
    let arguments = log_arguments(runner, options, filter).await?;
    let candidates = hashes(&read_log(runner, options, arguments.clone()).await?);
    let Some(query) = &filter.text else {
        return Ok(candidates);
    };
    let mut grep_arguments = arguments;
    grep_arguments.insert(0, format!("--grep={query}").into());
    grep_arguments.insert(
        0,
        if filter.regex {
            "--extended-regexp"
        } else {
            "--fixed-strings"
        }
        .into(),
    );
    if !filter.match_case {
        grep_arguments.insert(0, "--regexp-ignore-case".into());
    }
    let mut selected = hashes(&read_log(runner, options, grep_arguments).await?);
    // Hashes are lowercase hex; a hex-looking query also matches by hash prefix/infix.
    let needle = if filter.match_case {
        query.clone()
    } else {
        query.to_ascii_lowercase()
    };
    if !needle.is_empty() && needle.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        selected.extend(candidates.into_iter().filter(|hash| hash.contains(&needle)));
    }
    Ok(selected)
}

fn nearest_ancestors(
    graph: &CommitGraph,
    child: CommitId,
    selected: &HashSet<CommitId>,
) -> Vec<(CommitId, bool)> {
    let direct: HashSet<_> = graph.parents(child).collect();
    let mut stack: Vec<_> = direct.iter().copied().collect();
    let mut visited = HashSet::new();
    let mut matches = Vec::new();
    while let Some(ancestor) = stack.pop() {
        if !visited.insert(ancestor) {
            continue;
        }
        if selected.contains(&ancestor) {
            matches.push((ancestor, !direct.contains(&ancestor)));
        } else {
            stack.extend(graph.parents(ancestor));
        }
    }
    matches.sort_unstable_by_key(|(ancestor, _)| *ancestor);
    matches
}

fn collapse_graph(graph: &CommitGraph, hashes: &HashSet<String>) -> Result<GraphLayout, GitError> {
    let selected: HashSet<_> = (0..graph.len())
        .map(|row| CommitId(row as u32))
        .filter(|&id| graph.hash(id).is_some_and(|hash| hashes.contains(&hash)))
        .collect();
    let mut closed = String::new();
    let mut dotted = HashMap::new();
    for row in 0..graph.len() {
        let child = CommitId(row as u32);
        if !selected.contains(&child) {
            continue;
        }
        let child_hash = graph.hash(child).unwrap();
        closed.push_str(&child_hash);
        for (ancestor, skipped) in nearest_ancestors(graph, child, &selected) {
            let parent_hash = graph.hash(ancestor).unwrap();
            closed.push(' ');
            closed.push_str(&parent_hash);
            dotted.insert((child_hash.clone(), parent_hash), skipped);
        }
        closed.push('\n');
    }
    let collapsed =
        CommitGraph::parse_log(closed.as_bytes()).map_err(|error| invalid(&error.to_string()))?;
    let mut layout =
        GraphLayout::new(collapsed, &[]).map_err(|error| invalid(&error.to_string()))?;
    for index in 0..layout.graph().edge_count() {
        let edge_id = EdgeId(index as u32);
        let edge = layout.graph().edge(edge_id).unwrap();
        let pair = (
            layout.graph().hash(edge.child).unwrap(),
            layout.graph().hash(edge.parent).unwrap(),
        );
        if dotted.get(&pair) == Some(&true) {
            layout.dotted_edges.insert(edge_id);
        }
    }
    Ok(layout)
}
pub fn validate_hash(hash: &str) -> Result<(), GitError> {
    if !matches!(hash.len(), 40 | 64) || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(invalid("expected full object ID"));
    }
    Ok(())
}
#[derive(Debug, Clone, Serialize)]
pub struct Commit {
    pub hash: String,
    pub parents: Vec<String>,
    pub author: String,
    pub email: String,
    pub author_time: i64,
    pub commit_time: i64,
    pub subject: String,
    pub refs: String,
    pub message: String,
}
pub async fn page(
    r: &GitRunner,
    o: &RunOptions,
    hashes: &[String],
    full: bool,
) -> Result<Vec<Commit>, GitError> {
    if hashes.len() > 200 {
        return Err(invalid("page exceeds 200 commits"));
    }
    if hashes.is_empty() {
        return Ok(vec![]);
    }
    for h in hashes {
        validate_hash(h)?;
    }
    let mut options = o.clone();
    options.stdin = hashes.join("\n").into_bytes();
    options.stdin.push(b'\n');
    let format = if full {
        "--format=%H%x00%P%x00%an%x00%ae%x00%at%x00%ct%x00%s%x00%D%x00%B%x00"
    } else {
        "--format=%H%x00%P%x00%an%x00%ae%x00%at%x00%ct%x00%s%x00%D%x00%x00"
    };
    let b = r
        .run(
            ReadCommand::new(ReadOperation::Log).args([
                "--no-walk=unsorted",
                "--stdin",
                "--no-notes",
                "--no-color",
                format,
            ]),
            &options,
        )
        .await?
        .stdout;
    let mut fields = b.split(|b| *b == 0);
    let mut commits = vec![];
    while let Some(hash) = fields.next() {
        let hash = hash.strip_prefix(b"\n").unwrap_or(hash);
        if hash.is_empty() {
            continue;
        }
        let values: Vec<_> = fields.by_ref().take(8).map(text).collect();
        if values.len() != 8 {
            return Err(invalid("truncated commit"));
        }
        commits.push(Commit {
            hash: text(hash),
            parents: values[0].split_whitespace().map(String::from).collect(),
            author: values[1].clone(),
            email: values[2].clone(),
            author_time: values[3]
                .parse()
                .map_err(|_| invalid("invalid author time"))?,
            commit_time: values[4]
                .parse()
                .map_err(|_| invalid("invalid commit time"))?,
            subject: values[5].clone(),
            refs: values[6].clone(),
            message: values[7].clone(),
        });
    }
    Ok(commits)
}
#[derive(Debug, Clone, Serialize)]
pub struct FileChange {
    pub status: String,
    pub path: RepoPath,
    pub original: Option<RepoPath>,
}
pub async fn files(r: &GitRunner, o: &RunOptions, hash: &str) -> Result<Vec<FileChange>, GitError> {
    validate_hash(hash)?;
    let c = page(r, o, &[hash.into()], false)
        .await?
        .pop()
        .ok_or_else(|| invalid("missing commit"))?;
    let mut args = vec![
        "--no-commit-id",
        "-r",
        "-z",
        "--name-status",
        "-M",
        "--root",
    ];
    if let Some(parent) = c.parents.first() {
        args.push(parent);
    }
    args.push(hash);
    args.push("--");
    let b = r
        .run(ReadCommand::new(ReadOperation::DiffTree).args(args), o)
        .await?
        .stdout;
    let mut fields = b.split(|b| *b == 0);
    let mut out = vec![];
    while let Some(s) = fields.next() {
        if s.is_empty() {
            continue;
        }
        let first = fields.next().ok_or_else(|| invalid("missing file path"))?;
        let (path, original) = if s[0] == b'R' || s[0] == b'C' {
            (
                fields
                    .next()
                    .ok_or_else(|| invalid("missing rename path"))?,
                Some(RepoPath::new(first)),
            )
        } else {
            (first, None)
        };
        out.push(FileChange {
            status: text(s),
            path: RepoPath::new(path),
            original,
        });
    }
    Ok(out)
}

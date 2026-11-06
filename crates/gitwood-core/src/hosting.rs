//! Safe hosting URL parsing and link generation. No network or credentials are handled here.
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Github,
    Gitlab,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostedRemote {
    pub platform: Platform,
    pub host: String,
    pub path: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinkTarget<'a> {
    File {
        revision: &'a str,
        path: &'a str,
        start: Option<u32>,
        end: Option<u32>,
    },
    Commit {
        revision: &'a str,
    },
    Branch {
        branch: &'a str,
    },
    CreateRequest {
        base: Option<&'a str>,
        head: &'a str,
    },
}

/// A configured host is exactly a DNS name and optional numeric port.
pub fn validate_host(value: &str) -> bool {
    if value.is_empty()
        || value.len() > 253
        || value.bytes().any(|b| !b.is_ascii() || b.is_ascii_control())
    {
        return false;
    }
    let host = match value.rsplit_once(':') {
        Some((host, port))
            if !host.is_empty() && !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) =>
        {
            if !port.parse::<u16>().is_ok_and(|port| port > 0) {
                return false;
            }
            host
        }
        Some(_) => return false,
        None => value,
    };
    host.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && label.as_bytes()[0].is_ascii_alphanumeric()
            && label.as_bytes()[label.len() - 1].is_ascii_alphanumeric()
            && label
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    })
}

pub fn parse_remote(
    url: &str,
    github_hosts: &[String],
    gitlab_hosts: &[String],
) -> Option<HostedRemote> {
    if url.is_empty()
        || url
            .bytes()
            .any(|b| b.is_ascii_control() || b.is_ascii_whitespace())
    {
        return None;
    }
    let (authority, raw_path) = split_remote(url)?;
    if authority.bytes().filter(|byte| *byte == b'@').count() > 1 {
        return None;
    }
    let authority = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    if authority.contains('@') || !validate_host(authority) {
        return None;
    }
    let host = authority.to_ascii_lowercase();
    let hostname = host
        .rsplit_once(':')
        .map_or(host.as_str(), |(name, _)| name);
    let platform = if hostname == "github.com" || contains_host(github_hosts, &host) {
        Platform::Github
    } else if hostname == "gitlab.com" || contains_host(gitlab_hosts, &host) {
        Platform::Gitlab
    } else {
        return None;
    };
    let path = raw_path
        .trim_matches('/')
        .strip_suffix(".git")
        .unwrap_or(raw_path.trim_matches('/'));
    if path.is_empty() || path.contains(['?', '#', '\\']) {
        return None;
    }
    let parts: Vec<String> = path.split('/').map(str::to_owned).collect();
    if parts.len() < 2 || parts.iter().any(|p| p.is_empty() || p == "." || p == "..") {
        return None;
    }
    Some(HostedRemote {
        platform,
        host,
        path: parts,
    })
}

fn contains_host(hosts: &[String], host: &str) -> bool {
    hosts.iter().any(|value| value.eq_ignore_ascii_case(host))
}

fn split_remote(url: &str) -> Option<(&str, &str)> {
    if let Some(rest) = url.strip_prefix("https://") {
        let (authority, path) = rest.split_once('/')?;
        return Some((authority, path));
    }
    if let Some(rest) = url.strip_prefix("ssh://") {
        let (authority, path) = rest.split_once('/')?;
        if authority.bytes().filter(|byte| *byte == b'@').count() > 1 {
            return None;
        }
        let host = authority
            .rsplit_once('@')
            .map_or(authority, |(_, value)| value);
        let host = if let Some((host, port)) = host.rsplit_once(':') {
            if !port.is_empty() && port.parse::<u16>().is_ok_and(|value| value > 0) {
                host
            } else {
                return None;
            }
        } else {
            host
        };
        return Some((host, path));
    }
    if url.contains("://") {
        return None;
    }
    let (authority, path) = url.split_once(':')?;
    if authority.contains('/') || authority.is_empty() {
        return None;
    }
    Some((authority, path))
}

pub fn build_link(remote: &HostedRemote, target: LinkTarget<'_>) -> Option<String> {
    match target {
        LinkTarget::File {
            revision,
            path,
            start,
            end,
        } => build_file_link(remote, revision, path, start, end),
        LinkTarget::Commit { revision } => build_commit_link(remote, revision),
        LinkTarget::Branch { branch } => build_branch_link(remote, branch),
        LinkTarget::CreateRequest { base, head } => build_request_link(remote, base, head),
    }
}

fn repository_url(remote: &HostedRemote) -> String {
    format!(
        "https://{}/{}",
        remote.host,
        remote
            .path
            .iter()
            .map(|part| encode(part))
            .collect::<Vec<_>>()
            .join("/")
    )
}

fn build_file_link(
    remote: &HostedRemote,
    revision: &str,
    path: &str,
    start: Option<u32>,
    end: Option<u32>,
) -> Option<String> {
    if !valid_revision(revision) {
        return None;
    }
    let file_path = encode_path(path, true)?;
    let line = line_fragment(remote.platform, start, end)?;
    let blob = if remote.platform == Platform::Github {
        "blob"
    } else {
        "-/blob"
    };
    Some(format!(
        "{}/{blob}/{revision}/{file_path}{line}",
        repository_url(remote)
    ))
}

fn line_fragment(platform: Platform, start: Option<u32>, end: Option<u32>) -> Option<String> {
    match (start, end) {
        (None, None) => Some(String::new()),
        (Some(first), None) if first > 0 => Some(format!("#L{first}")),
        (Some(first), Some(last)) if first > 0 && last >= first => {
            let range = if first == last {
                String::new()
            } else if platform == Platform::Github {
                format!("-L{last}")
            } else {
                format!("-{last}")
            };
            Some(format!("#L{first}{range}"))
        }
        _ => None,
    }
}

fn build_commit_link(remote: &HostedRemote, revision: &str) -> Option<String> {
    if !valid_revision(revision) {
        return None;
    }
    let segment = if remote.platform == Platform::Github {
        "commit"
    } else {
        "-/commit"
    };
    Some(format!("{}/{segment}/{revision}", repository_url(remote)))
}

fn build_branch_link(remote: &HostedRemote, branch: &str) -> Option<String> {
    let branch = encode_path(branch, true)?;
    let segment = if remote.platform == Platform::Github {
        "tree"
    } else {
        "-/tree"
    };
    Some(format!("{}/{segment}/{branch}", repository_url(remote)))
}

fn build_request_link(remote: &HostedRemote, base: Option<&str>, head: &str) -> Option<String> {
    let base = encode_path(base?, remote.platform == Platform::Github)?;
    let head = encode_path(head, remote.platform == Platform::Github)?;
    let repository = repository_url(remote);
    match remote.platform {
        Platform::Github => Some(format!("{repository}/compare/{base}...{head}?expand=1")),
        Platform::Gitlab => Some(format!("{repository}/-/merge_requests/new?merge_request%5Bsource_branch%5D={head}&merge_request%5Btarget_branch%5D={base}")),
    }
}

/// Resolve only an existing ordinary file contained by the canonical worktree root.
pub fn resolve_workspace_file(root: &Path, relative: &Path) -> Result<PathBuf, &'static str> {
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err("expected a relative worktree file path");
    }
    let root = root.canonicalize().map_err(|_| "worktree is unavailable")?;
    let file = root
        .join(relative)
        .canonicalize()
        .map_err(|_| "file is unavailable")?;
    if !file.starts_with(&root) {
        return Err("file escapes worktree");
    }
    if !file
        .metadata()
        .map_err(|_| "file is unavailable")?
        .is_file()
    {
        return Err("expected an ordinary file");
    }
    Ok(file)
}

fn valid_revision(value: &str) -> bool {
    [40, 64].contains(&value.len()) && value.bytes().all(|b| b.is_ascii_hexdigit())
}

fn encode_path(value: &str, allow_slashes: bool) -> Option<String> {
    if value.is_empty()
        || value.bytes().any(|b| b.is_ascii_control())
        || value
            .split('/')
            .any(|part| part.is_empty() || part == ".." || part == ".")
    {
        return None;
    }
    Some(
        value
            .split('/')
            .map(encode)
            .collect::<Vec<_>>()
            .join(if allow_slashes { "/" } else { "%2F" }),
    )
}

fn encode(value: &str) -> String {
    let mut output = String::new();
    for byte in value.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'.' | b'_' | b'~') {
            output.push(*byte as char);
        } else {
            output.push_str(&format!("%{byte:02X}"));
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hosted(platform: Platform) -> HostedRemote {
        HostedRemote {
            platform,
            host: if platform == Platform::Github {
                "github.com"
            } else {
                "gitlab.com"
            }
            .into(),
            path: vec!["team name".into(), "repo#?é".into()],
        }
    }

    #[test]
    fn parses_supported_remote_forms_and_strips_credentials_and_ssh_ports() {
        for url in [
            "https://user:secret@github.com/team/repo.git/",
            "git@github.com:team/repo.git",
            "ssh://git@github.com:2222/team/repo.git",
            "https://github.com:8443/team/repo.git",
        ] {
            let remote = parse_remote(url, &[], &[]).unwrap_or_else(|| panic!("failed: {url}"));
            assert_eq!(remote.platform, Platform::Github);
            assert_eq!(
                remote.host,
                if url.contains("8443") {
                    "github.com:8443"
                } else {
                    "github.com"
                }
            );
            assert!(!format!("{remote:?}").contains("secret"));
        }
        let gitlab = parse_remote(
            "git@gitlab.example:group/sub/repo.git",
            &[],
            &["gitlab.example".into()],
        )
        .unwrap();
        assert_eq!(gitlab.path, ["group", "sub", "repo"]);
    }

    #[test]
    fn accepts_ssh_without_port_https_without_git_and_rejects_http() {
        let ssh = parse_remote("ssh://git@github.com/group/repo", &[], &[]).unwrap();
        assert_eq!(ssh.host, "github.com");
        assert_eq!(ssh.path, ["group", "repo"]);
        let https = parse_remote("https://github.com/group/repo/", &[], &[]).unwrap();
        assert_eq!(https.path, ["group", "repo"]);
        assert!(parse_remote("http://github.com/group/repo.git", &[], &[]).is_none());
    }

    #[test]
    fn recognizes_self_hosted_github_and_builds_gitlab_multigroup_link() {
        let custom = parse_remote(
            "https://github.corp:8443/org/repo.git",
            &["github.corp:8443".into()],
            &[],
        )
        .unwrap();
        let revision = "b".repeat(40);
        assert_eq!(
            build_link(
                &custom,
                LinkTarget::Commit {
                    revision: &revision
                }
            )
            .unwrap(),
            format!("https://github.corp:8443/org/repo/commit/{revision}")
        );
        let gitlab = parse_remote(
            "https://gitlab.corp/group/subgroup/repo/",
            &[],
            &["gitlab.corp".into()],
        )
        .unwrap();
        assert_eq!(
            build_link(
                &gitlab,
                LinkTarget::Branch {
                    branch: "release/β"
                }
            )
            .unwrap(),
            "https://gitlab.corp/group/subgroup/repo/-/tree/release/%CE%B2"
        );
    }

    #[test]
    fn rejects_unsafe_urls_and_unconfigured_hosts() {
        for value in [
            "javascript:alert(1)",
            "https://user@evil-github.com/g/r",
            "https://github.com@evil.test/g/r",
            "https://user@github.com@evil.test/g/r",
            "https://github.com/g/r\n",
        ] {
            assert!(parse_remote(value, &[], &[]).is_none());
        }
        assert!(parse_remote("https://gitlab.corp/g/r", &[], &[]).is_none());
        assert!(validate_host("gitlab.corp:9443"));
        for value in [
            "https://gitlab.corp/x",
            "a@b.com",
            "host:abc",
            "-bad.com",
            "bad..com",
            "host:99999",
        ] {
            assert!(!validate_host(value));
        }
    }

    #[test]
    fn github_file_and_branch_links_encode_each_path_segment() {
        let revision = "a".repeat(40);
        let gh = build_link(
            &hosted(Platform::Github),
            LinkTarget::File {
                revision: &revision,
                path: "dir/a #?é.rs",
                start: Some(10),
                end: Some(20),
            },
        )
        .unwrap();
        assert_eq!(gh, format!("https://github.com/team%20name/repo%23%3F%C3%A9/blob/{revision}/dir/a%20%23%3F%C3%A9.rs#L10-L20"));
        let branch = build_link(
            &hosted(Platform::Github),
            LinkTarget::Branch {
                branch: "feature/a b",
            },
        )
        .unwrap();
        assert!(branch.ends_with("/tree/feature/a%20b"));
    }

    #[test]
    fn gitlab_commit_branch_file_and_merge_request_use_gitlab_routes() {
        let revision = "c".repeat(40);
        let remote = hosted(Platform::Gitlab);
        assert_eq!(
            build_link(
                &remote,
                LinkTarget::Commit {
                    revision: &revision
                }
            )
            .unwrap(),
            format!("https://gitlab.com/team%20name/repo%23%3F%C3%A9/-/commit/{revision}")
        );
        assert_eq!(
            build_link(
                &remote,
                LinkTarget::Branch {
                    branch: "feature/ready"
                }
            )
            .unwrap(),
            "https://gitlab.com/team%20name/repo%23%3F%C3%A9/-/tree/feature/ready"
        );
        let file = build_link(
            &remote,
            LinkTarget::File {
                revision: &revision,
                path: "src/file.rs",
                start: Some(10),
                end: Some(20),
            },
        )
        .unwrap();
        assert!(file.contains(&format!("/-/blob/{revision}/src/file.rs#L10-20")));
    }

    #[test]
    fn create_request_links_encode_branch_values_and_require_base() {
        let request = build_link(
            &hosted(Platform::Github),
            LinkTarget::CreateRequest {
                base: Some("release/β"),
                head: "feature #1",
            },
        )
        .unwrap();
        assert!(request.contains("/compare/release/%CE%B2...feature%20%231?expand=1"));
        let merge = build_link(
            &hosted(Platform::Gitlab),
            LinkTarget::CreateRequest {
                base: Some("main"),
                head: "feature/a b",
            },
        )
        .unwrap();
        assert!(merge.contains("feature%2Fa%20b"));
        assert!(build_link(
            &hosted(Platform::Github),
            LinkTarget::CreateRequest {
                base: None,
                head: "main"
            }
        )
        .is_none());
        assert!(build_link(
            &hosted(Platform::Gitlab),
            LinkTarget::CreateRequest {
                base: None,
                head: "main"
            }
        )
        .is_none());
    }

    #[test]
    fn workspace_path_requires_existing_regular_file_inside_root() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        std::fs::write(dir.path().join("sub/file"), "ok").unwrap();
        assert!(resolve_workspace_file(dir.path(), Path::new("sub/file")).is_ok());
        for path in ["../outside", "/etc/passwd", "sub", "missing"] {
            assert!(resolve_workspace_file(dir.path(), Path::new(path)).is_err());
        }
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("escape"), "no").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(outside.path().join("escape"), dir.path().join("linked"))
            .unwrap();
        #[cfg(unix)]
        assert_eq!(
            resolve_workspace_file(dir.path(), Path::new("linked")),
            Err("file escapes worktree")
        );
    }
}

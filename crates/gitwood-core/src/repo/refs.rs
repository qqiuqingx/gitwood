use super::{invalid, status, text};
use crate::git::{GitError, GitRunner, ReadCommand, ReadOperation, RunOptions};
use serde::Serialize;
use std::collections::BTreeMap;
#[derive(Debug, Clone, Serialize)]
pub struct Reference {
    pub name: String,
    pub hash: String,
    pub peeled: Option<String>,
    pub upstream: Option<String>,
    pub track: String,
}
#[derive(Debug, Serialize)]
pub struct References {
    pub local: Vec<Reference>,
    pub remotes: BTreeMap<String, Vec<Reference>>,
    pub tags: Vec<Reference>,
    pub remote_heads: BTreeMap<String, String>,
    pub head: status::Branch,
}
pub async fn read(r: &GitRunner, o: &RunOptions) -> Result<References, GitError> {
    let bytes=r.run(ReadCommand::new(ReadOperation::ForEachRef).args(["--format=%(refname)%00%(objectname)%00%(*objectname)%00%(upstream)%00%(upstream:track,nobracket)%00%(symref)%00","refs/heads","refs/remotes","refs/tags"]),o).await?.stdout;
    let names = r
        .run(ReadCommand::new(ReadOperation::RemoteNames), o)
        .await?
        .stdout;
    let remotes = text(&names)
        .lines()
        .map(|name| (name.to_owned(), vec![]))
        .collect();
    let mut result = References {
        local: vec![],
        remotes,
        tags: vec![],
        remote_heads: BTreeMap::new(),
        head: status::read(r, o).await?.branch,
    };
    // Git ref names cannot contain LF/NUL; record LF and field NUL are unambiguous.
    for line in bytes.split(|b| *b == b'\n').filter(|v| !v.is_empty()) {
        let f: Vec<_> = line.split(|b| *b == 0).collect();
        if f.len() != 7 {
            return Err(invalid("invalid ref record"));
        }
        let name = text(f[0]);
        if !f[5].is_empty() && name.starts_with("refs/remotes/") {
            result.remote_heads.insert(name, text(f[5]));
            continue;
        }
        let reference = Reference {
            name: name.clone(),
            hash: text(f[1]),
            peeled: (!f[2].is_empty()).then(|| text(f[2])),
            upstream: (!f[3].is_empty()).then(|| text(f[3])),
            track: text(f[4]),
        };
        if name.starts_with("refs/heads/") {
            result.local.push(reference);
        } else if let Some(remote) = name.strip_prefix("refs/remotes/") {
            let owner = result
                .remotes
                .keys()
                .filter(|name| remote.starts_with(&format!("{name}/")))
                .max_by_key(|name| name.len())
                .cloned()
                .unwrap_or_else(|| remote.split('/').next().unwrap_or_default().into());
            result.remotes.entry(owner).or_default().push(reference);
        } else {
            result.tags.push(reference);
        }
    }
    Ok(result)
}

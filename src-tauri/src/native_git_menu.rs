//! Git menu shape and localized Tauri menu construction.
use crate::native_menu::{MenuLabels, NativeMenuActions};
use tauri::{
    menu::{MenuItem, PredefinedMenuItem, Submenu},
    AppHandle, Wry,
};

#[derive(Debug, PartialEq, Eq)]
enum GitMenuNode {
    Action {
        id: &'static str,
        label: &'static str,
        accelerator: Option<&'static str>,
    },
    Separator,
    Submenu {
        id: &'static str,
        label: &'static str,
        children: Vec<GitMenuNode>,
    },
}

use GitMenuNode::{Action, Separator, Submenu as ChildMenu};

fn action(id: &'static str, label: &'static str) -> GitMenuNode {
    Action {
        id,
        label,
        accelerator: None,
    }
}

fn shortcut_action(
    id: &'static str,
    label: &'static str,
    accelerator: &'static str,
) -> GitMenuNode {
    Action {
        id,
        label,
        accelerator: Some(accelerator),
    }
}

fn main_operations() -> Vec<GitMenuNode> {
    vec![
        shortcut_action("CheckinProject", "commitAction", "Cmd+K"),
        shortcut_action("Vcs.Push", "push", "Cmd+Shift+K"),
        shortcut_action("Vcs.UpdateProject", "update", "Cmd+T"),
        action("Git.Pull", "pull"),
        action("Git.Fetch", "fetch"),
        Separator,
        action("Git.Merge", "merge"),
        action("Git.Merge.Commit", "commitMerge"),
        action("Git.Merge.Abort", "abortMerge"),
        action("Git.Rebase", "rebase"),
        action("Git.Rebase.Abort", "abortRebase"),
        action("Git.Rebase.Continue", "continueRebase"),
        action("Git.Rebase.Skip", "skipCommit"),
        Separator,
    ]
}

fn branch_and_log_actions() -> Vec<GitMenuNode> {
    vec![
        action("Git.Branches", "branches"),
        action("Git.CreateNewBranch", "newBranch"),
        action("Git.Tag", "newTag"),
        action("Git.Reset", "resetHead"),
        Separator,
        action("Vcs.Show.Log", "showGitLog"),
    ]
}

fn local_changes_menu() -> GitMenuNode {
    ChildMenu {
        id: "Git.MainMenu.LocalChanges",
        label: "uncommittedChanges",
        children: vec![
            action("Git.StashChanges", "stash"),
            action("Git.UnstashChanges", "unstash"),
            action("ChangesView.Revert", "rollback"),
        ],
    }
}

fn current_file_menu() -> GitMenuNode {
    ChildMenu {
        id: "Git.MainMenu.FileActions",
        label: "currentFile",
        children: vec![
            action("ChangesView.CommitFile", "commitFile"),
            action("ChangesView.AddUnversioned", "addToVcs"),
            Separator,
            action("Git.CurrentFile.Annotate", "annotate"),
            action("Git.CurrentFile.CompareRevision", "compareRevision"),
            action("Git.CurrentFile.CompareBranch", "compareBranch"),
            action("Git.CurrentFile.ShowHistory", "showHistory"),
            hosting_menu(
                "Git.CurrentFile.GitHub",
                "github",
                [
                    ("Git.CurrentFile.Github.Open", "openHosted"),
                    ("Git.CurrentFile.Github.Copy", "copyLink"),
                ],
            ),
            hosting_menu(
                "Git.CurrentFile.GitLab",
                "gitlab",
                [
                    ("Git.CurrentFile.GitLab.Open", "openHosted"),
                    ("Git.CurrentFile.GitLab.Copy", "copyLink"),
                ],
            ),
        ],
    }
}

fn hosting_menu(
    platform: &'static str,
    label: &'static str,
    actions: impl IntoIterator<Item = (&'static str, &'static str)>,
) -> GitMenuNode {
    ChildMenu {
        id: platform,
        label,
        children: actions
            .into_iter()
            .map(|(id, text)| action(id, text))
            .collect(),
    }
}

fn git_menu_spec() -> Vec<GitMenuNode> {
    let mut nodes = main_operations();
    nodes.extend(branch_and_log_actions());
    nodes.push(local_changes_menu());
    nodes.push(current_file_menu());
    nodes.push(Separator);
    nodes.push(hosting_menu(
        "Git.MainMenu.GitHub",
        "github",
        [
            ("Github.Open.In.Browser", "openHosted"),
            ("GitHub.Copy.Link", "copyLink"),
            ("Github.Create.Pull.Request", "createPullRequest"),
        ],
    ));
    nodes.push(hosting_menu(
        "Git.MainMenu.GitLab",
        "gitlab",
        [
            ("GitLab.Open.In.Browser", "openHosted"),
            ("GitLab.Copy.Link", "copyLink"),
            ("GitLab.Merge.Request.Create", "createMergeRequest"),
        ],
    ));
    nodes.push(Separator);
    nodes.push(shortcut_action(
        "Vcs.QuickListPopupAction",
        "vcsOperations",
        "Ctrl+V",
    ));
    nodes
}

pub(crate) fn build(
    app: &AppHandle,
    labels: &MenuLabels,
    state: &NativeMenuActions,
) -> tauri::Result<Submenu<Wry>> {
    let git = Submenu::with_id(app, "git-menu", labels.text("git"), state.has_repository)?;
    append_nodes(app, &git, &git_menu_spec(), labels, state)?;
    Ok(git)
}

fn append_nodes(
    app: &AppHandle,
    parent: &Submenu<Wry>,
    nodes: &[GitMenuNode],
    labels: &MenuLabels,
    state: &NativeMenuActions,
) -> tauri::Result<()> {
    for node in nodes {
        match node {
            Action {
                id,
                label,
                accelerator,
            } => {
                let item = MenuItem::with_id(
                    app,
                    *id,
                    labels.text(label),
                    state.is_enabled(id),
                    *accelerator,
                )?;
                parent.append(&item)?;
            }
            Separator => parent.append(&PredefinedMenuItem::separator(app)?)?,
            ChildMenu {
                id,
                label,
                children,
            } => {
                let submenu = Submenu::with_id(app, *id, labels.text(label), true)?;
                append_nodes(app, &submenu, children, labels, state)?;
                submenu.set_enabled(
                    children
                        .iter()
                        .any(|child| matches!(child, Action { id, .. } if state.is_enabled(id))),
                )?;
                parent.append(&submenu)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{git_menu_spec, GitMenuNode};

    fn ids(nodes: &[GitMenuNode]) -> Vec<String> {
        nodes
            .iter()
            .map(|node| match node {
                GitMenuNode::Action { id, .. } => (*id).to_owned(),
                GitMenuNode::Separator => "|".into(),
                GitMenuNode::Submenu { id, .. } => (*id).to_owned(),
            })
            .collect()
    }

    #[test]
    fn git_menu_follows_idea_xml_order_for_the_implemented_subset() {
        let menu = git_menu_spec();
        assert_eq!(
            ids(&menu),
            [
                "CheckinProject",
                "Vcs.Push",
                "Vcs.UpdateProject",
                "Git.Pull",
                "Git.Fetch",
                "|",
                "Git.Merge",
                "Git.Merge.Commit",
                "Git.Merge.Abort",
                "Git.Rebase",
                "Git.Rebase.Abort",
                "Git.Rebase.Continue",
                "Git.Rebase.Skip",
                "|",
                "Git.Branches",
                "Git.CreateNewBranch",
                "Git.Tag",
                "Git.Reset",
                "|",
                "Vcs.Show.Log",
                "Git.MainMenu.LocalChanges",
                "Git.MainMenu.FileActions",
                "|",
                "Git.MainMenu.GitHub",
                "Git.MainMenu.GitLab",
                "|",
                "Vcs.QuickListPopupAction",
            ]
            .map(str::to_owned)
        );
        let GitMenuNode::Submenu { children, .. } = &menu[20] else {
            panic!("local changes submenu")
        };
        assert_eq!(
            ids(children),
            [
                "Git.StashChanges",
                "Git.UnstashChanges",
                "ChangesView.Revert"
            ]
        );
        let GitMenuNode::Submenu { children, .. } = &menu[23] else {
            panic!("GitHub submenu")
        };
        assert_eq!(
            ids(children),
            [
                "Github.Open.In.Browser",
                "GitHub.Copy.Link",
                "Github.Create.Pull.Request"
            ]
            .map(str::to_owned)
        );
        let GitMenuNode::Submenu { children, .. } = &menu[24] else {
            panic!("GitLab submenu")
        };
        assert_eq!(
            ids(children),
            [
                "GitLab.Open.In.Browser",
                "GitLab.Copy.Link",
                "GitLab.Merge.Request.Create"
            ]
            .map(str::to_owned)
        );
        let GitMenuNode::Submenu { children, .. } = &menu[21] else {
            panic!("current file submenu")
        };
        assert_eq!(
            ids(children),
            [
                "ChangesView.CommitFile",
                "ChangesView.AddUnversioned",
                "|",
                "Git.CurrentFile.Annotate",
                "Git.CurrentFile.CompareRevision",
                "Git.CurrentFile.CompareBranch",
                "Git.CurrentFile.ShowHistory",
                "Git.CurrentFile.GitHub",
                "Git.CurrentFile.GitLab",
            ]
        );
    }

    #[test]
    fn native_accelerators_are_limited_to_idea_macos_git_shortcuts() {
        let menu = git_menu_spec();
        let accelerators = menu
            .iter()
            .filter_map(|node| match node {
                GitMenuNode::Action {
                    id,
                    accelerator: Some(value),
                    ..
                } => Some((*id, *value)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            accelerators,
            [
                ("CheckinProject", "Cmd+K"),
                ("Vcs.Push", "Cmd+Shift+K"),
                ("Vcs.UpdateProject", "Cmd+T"),
                ("Vcs.QuickListPopupAction", "Ctrl+V"),
            ]
        );
    }
}

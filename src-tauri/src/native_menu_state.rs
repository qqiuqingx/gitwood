//! Per-window repository and selection state used to compute native Git menu enablement.
use gitwood_core::repo::{operation::OperationKind, refs::References, session::StatusSummary};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum NativeFileSource {
    Changes,
    Commit,
}

#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeFileSelection {
    pub source: NativeFileSource,
    pub versioned: bool,
    pub untracked: bool,
    pub conflicted: bool,
}

#[derive(Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeMenuSelection {
    pub current_file: Option<NativeFileSelection>,
    pub selected_change_count: usize,
    pub selected_commit_count: usize,
    pub busy: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct MenuRepositoryStatus {
    has_head: bool,
    attached: bool,
    has_upstream: bool,
    changes: usize,
    untracked: usize,
    conflicts: usize,
    operation: Option<OperationKind>,
    update_stash: bool,
}

impl From<&StatusSummary> for MenuRepositoryStatus {
    fn from(status: &StatusSummary) -> Self {
        Self {
            has_head: status
                .branch
                .oid
                .as_deref()
                .is_some_and(|oid| oid != "(initial)"),
            attached: status
                .branch
                .head
                .as_deref()
                .is_some_and(|head| head != "(detached)"),
            has_upstream: status.branch.upstream.is_some(),
            changes: status.changes,
            untracked: status.untracked,
            conflicts: status.conflicts,
            operation: status.operation.as_ref().map(|operation| operation.kind),
            update_stash: status.update_stash.is_some(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct MenuReferences {
    remotes: usize,
    local_branches: usize,
    remote_branches: usize,
}

impl From<&References> for MenuReferences {
    fn from(references: &References) -> Self {
        Self {
            remotes: references.remotes.len(),
            local_branches: references.local.len(),
            remote_branches: references.remotes.values().map(Vec::len).sum(),
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeMenuActions {
    pub has_repository: bool,
    pub actions: BTreeMap<String, bool>,
}

impl NativeMenuActions {
    pub fn is_enabled(&self, id: &str) -> bool {
        self.actions.get(id).copied().unwrap_or(false)
    }
}

impl Default for NativeMenuActions {
    fn default() -> Self {
        calculate_actions(None, None, NativeMenuSelection::default())
    }
}

/// Cached per project window. The process menu displays the focused window's result.
#[derive(Default)]
pub(crate) struct ProjectMenuContext {
    status: Option<MenuRepositoryStatus>,
    references: Option<MenuReferences>,
    selection: NativeMenuSelection,
    presentation: NativeMenuActions,
}

impl ProjectMenuContext {
    pub(crate) fn set_status(&mut self, status: &StatusSummary) -> bool {
        let next = MenuRepositoryStatus::from(status);
        if self.status.as_ref() == Some(&next) {
            return false;
        }
        self.status = Some(next);
        self.recompute()
    }

    pub(crate) fn set_references(&mut self, references: &References) -> bool {
        let next = MenuReferences::from(references);
        if self.references.as_ref() == Some(&next) {
            return false;
        }
        self.references = Some(next);
        self.recompute()
    }

    pub(crate) fn set_selection(&mut self, selection: NativeMenuSelection) -> bool {
        if self.selection == selection {
            return false;
        }
        self.selection = selection;
        self.recompute()
    }

    pub(crate) fn clear_repository(&mut self) -> bool {
        self.status = None;
        self.references = None;
        self.selection.current_file = None;
        self.selection.selected_change_count = 0;
        self.selection.selected_commit_count = 0;
        self.selection.busy = false;
        self.recompute()
    }

    fn recompute(&mut self) -> bool {
        let next = calculate_actions(
            self.status.as_ref(),
            self.references.as_ref(),
            self.selection,
        );
        if self.presentation == next {
            return false;
        }
        self.presentation = next;
        true
    }

    pub(crate) fn presentation(&self) -> NativeMenuActions {
        self.presentation.clone()
    }
}

fn calculate_actions(
    status: Option<&MenuRepositoryStatus>,
    references: Option<&MenuReferences>,
    selection: NativeMenuSelection,
) -> NativeMenuActions {
    let has_repository = status.is_some();
    let mut actions = disabled_actions();
    set_repository_actions(&mut actions, status, references, selection);
    set_operation_actions(&mut actions, status, selection);
    set_file_actions(&mut actions, status, references, selection);
    set(&mut actions, "Vcs.Show.Log", has_repository);
    set(&mut actions, "ChangesView.ToggleCommitUi", has_repository);
    set(&mut actions, "Vcs.QuickListPopupAction", has_repository);
    NativeMenuActions {
        has_repository,
        actions,
    }
}

fn disabled_actions() -> BTreeMap<String, bool> {
    [
        "CheckinProject",
        "Vcs.Push",
        "Vcs.UpdateProject",
        "Git.Pull",
        "Git.Fetch",
        "Git.Merge",
        "Git.Merge.Commit",
        "Git.Merge.Abort",
        "Git.Rebase",
        "Git.Rebase.Continue",
        "Git.Rebase.Abort",
        "Git.Rebase.Skip",
        "Git.Branches",
        "Git.CreateNewBranch",
        "Git.Tag",
        "Git.Reset",
        "Vcs.Show.Log",
        "Git.StashChanges",
        "Git.UnstashChanges",
        "ChangesView.Revert",
        "ChangesView.CommitFile",
        "ChangesView.AddUnversioned",
        "Git.CurrentFile.ShowHistory",
        "Git.CurrentFile.Annotate",
        "Git.CurrentFile.CompareRevision",
        "Git.CurrentFile.CompareBranch",
        "Git.ResolveConflicts",
        "ChangesView.ToggleCommitUi",
        "Vcs.QuickListPopupAction",
    ]
    .into_iter()
    .map(|id| (id.to_owned(), false))
    .collect()
}

fn set(actions: &mut BTreeMap<String, bool>, id: &str, enabled: bool) {
    actions.insert(id.to_owned(), enabled);
}

fn set_repository_actions(
    actions: &mut BTreeMap<String, bool>,
    status: Option<&MenuRepositoryStatus>,
    references: Option<&MenuReferences>,
    selection: NativeMenuSelection,
) {
    set_remote_actions(actions, status, references, selection);
    set_branch_actions(actions, status, references, selection);
    set_local_actions(actions, status, selection);
}

fn set_remote_actions(
    actions: &mut BTreeMap<String, bool>,
    status: Option<&MenuRepositoryStatus>,
    references: Option<&MenuReferences>,
    selection: NativeMenuSelection,
) {
    let has_repository = status.is_some();
    let has_head = status.is_some_and(|state| state.has_head);
    let attached = status.is_some_and(|state| state.attached);
    let no_conflicts = status.is_some_and(|state| state.conflicts == 0);
    let no_operation = status.is_some_and(|state| state.operation.is_none());
    let no_recovery = status.is_some_and(|state| !state.update_stash);
    let idle = !selection.busy;
    let ready = has_repository && idle && no_operation && no_conflicts && no_recovery;
    let can_integrate = ready && has_head && attached && references.is_some();
    let has_remotes = references.is_some_and(|refs| refs.remotes > 0);
    set(
        actions,
        "Vcs.Push",
        has_head && attached && idle && has_remotes,
    );
    set(
        actions,
        "Vcs.UpdateProject",
        can_integrate && status.is_some_and(|state| state.has_upstream),
    );
    set(actions, "Git.Pull", can_integrate && has_remotes);
    set(actions, "Git.Fetch", has_repository && idle && has_remotes);
    set(actions, "Git.Merge", can_integrate);
    set(actions, "Git.Rebase", can_integrate);
}

fn set_branch_actions(
    actions: &mut BTreeMap<String, bool>,
    status: Option<&MenuRepositoryStatus>,
    references: Option<&MenuReferences>,
    selection: NativeMenuSelection,
) {
    let has_repository = status.is_some();
    let has_head = status.is_some_and(|state| state.has_head);
    let attached = status.is_some_and(|state| state.attached);
    let no_conflicts = status.is_some_and(|state| state.conflicts == 0);
    let no_operation = status.is_some_and(|state| state.operation.is_none());
    let no_recovery = status.is_some_and(|state| !state.update_stash);
    let idle = !selection.busy;
    let ready = has_repository && idle && no_operation && no_conflicts && no_recovery;
    let can_integrate = ready && has_head && attached && references.is_some();
    set(
        actions,
        "Git.Branches",
        has_repository && references.is_some(),
    );
    set(
        actions,
        "Git.CreateNewBranch",
        ready && has_head && references.is_some(),
    );
    set(
        actions,
        "Git.Tag",
        ready && has_head && references.is_some(),
    );
    set(
        actions,
        "Git.Reset",
        can_integrate && selection.selected_commit_count > 0,
    );
}

fn set_local_actions(
    actions: &mut BTreeMap<String, bool>,
    status: Option<&MenuRepositoryStatus>,
    selection: NativeMenuSelection,
) {
    let has_repository = status.is_some();
    let idle = !selection.busy;
    let no_conflicts = status.is_some_and(|state| state.conflicts == 0);
    let no_operation = status.is_some_and(|state| state.operation.is_none());
    let no_recovery = status.is_some_and(|state| !state.update_stash);
    set(actions, "CheckinProject", has_repository && idle);
    let ready = has_repository && idle && no_operation && no_conflicts && no_recovery;
    let local_changes = status.is_some_and(|state| state.changes + state.untracked > 0);
    set(actions, "Git.StashChanges", ready && local_changes);
    set(actions, "Git.UnstashChanges", has_repository && idle);
}

fn set_operation_actions(
    actions: &mut BTreeMap<String, bool>,
    status: Option<&MenuRepositoryStatus>,
    selection: NativeMenuSelection,
) {
    let idle = !selection.busy;
    let no_conflicts = status.is_some_and(|state| state.conflicts == 0);
    let merge = status.is_some_and(|state| state.operation == Some(OperationKind::Merge));
    let rebase = status.is_some_and(|state| state.operation == Some(OperationKind::Rebase));
    let has_repository = status.is_some();
    set(actions, "Git.Merge.Commit", merge && idle && no_conflicts);
    set(actions, "Git.Merge.Abort", merge && idle);
    set(
        actions,
        "Git.Rebase.Continue",
        rebase && idle && no_conflicts,
    );
    set(actions, "Git.Rebase.Abort", rebase && idle);
    set(actions, "Git.Rebase.Skip", rebase && idle);
    set(
        actions,
        "Git.ResolveConflicts",
        has_repository && idle && !no_conflicts,
    );
}

fn set_file_actions(
    actions: &mut BTreeMap<String, bool>,
    status: Option<&MenuRepositoryStatus>,
    references: Option<&MenuReferences>,
    selection: NativeMenuSelection,
) {
    let idle = !selection.busy;
    let has_head = status.is_some_and(|state| state.has_head);
    let current_file = selection.current_file;
    let tracked = current_file.is_some_and(|file| file.versioned && !file.conflicted);
    let from_changes = current_file.is_some_and(|file| file.source == NativeFileSource::Changes);
    let untracked = current_file.is_some_and(|file| file.untracked && !file.conflicted);
    set(
        actions,
        "ChangesView.Revert",
        idle && selection.selected_change_count > 0
            && status.is_some_and(|state| state.conflicts == 0),
    );
    set(
        actions,
        "ChangesView.CommitFile",
        idle && from_changes && !current_file.is_some_and(|file| file.conflicted),
    );
    set(
        actions,
        "ChangesView.AddUnversioned",
        idle && from_changes && untracked,
    );
    let can_use = idle && tracked && has_head;
    set(actions, "Git.CurrentFile.ShowHistory", can_use);
    set(actions, "Git.CurrentFile.Annotate", can_use);
    set(
        actions,
        "Git.CurrentFile.CompareRevision",
        can_use && references.is_some(),
    );
    set(
        actions,
        "Git.CurrentFile.CompareBranch",
        can_use && references.is_some_and(|refs| refs.local_branches + refs.remote_branches > 0),
    );
}

#[cfg(test)]
#[path = "native_menu_tests.rs"]
mod tests;

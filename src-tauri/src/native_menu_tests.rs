use super::{
    calculate_actions, MenuReferences, MenuRepositoryStatus, NativeFileSelection, NativeFileSource,
    NativeMenuSelection, ProjectMenuContext,
};
use crate::{project_windows::ProjectWindows, settings::Layout};
use gitwood_core::repo::{operation::OperationKind, session::StatusSummary};

fn summary(changes: usize) -> StatusSummary {
    StatusSummary {
        branch: gitwood_core::repo::status::Branch {
            oid: Some("a".repeat(40)),
            head: Some("main".into()),
            upstream: None,
            ahead: 0,
            behind: 0,
        },
        changes,
        staged: 0,
        untracked: 0,
        conflicts: 0,
        conflict_files: vec![],
        merge_message: None,
        update_stash: None,
        operation: None,
        fingerprint: "state".into(),
    }
}

fn base() -> (MenuRepositoryStatus, MenuReferences) {
    (
        MenuRepositoryStatus {
            has_head: true,
            attached: true,
            has_upstream: true,
            changes: 0,
            untracked: 0,
            conflicts: 0,
            operation: None,
            update_stash: false,
        },
        MenuReferences {
            remotes: 1,
            local_branches: 1,
            remote_branches: 1,
        },
    )
}

#[test]
fn clean_repository_enables_commit_read_and_remote_actions() {
    let (status, refs) = base();
    let actions = calculate_actions(Some(&status), Some(&refs), NativeMenuSelection::default());
    assert!(actions.has_repository);
    assert!(actions.is_enabled("CheckinProject"));
    for id in [
        "Vcs.Push",
        "Vcs.UpdateProject",
        "Git.Pull",
        "Git.Fetch",
        "Git.Merge",
        "Git.Rebase",
        "Git.Branches",
        "Git.Tag",
        "Vcs.Show.Log",
        "Git.UnstashChanges",
        "Vcs.QuickListPopupAction",
    ] {
        assert!(actions.is_enabled(id), "{id}");
    }
    for id in [
        "Git.StashChanges",
        "ChangesView.Revert",
        "Git.Reset",
        "Git.CurrentFile.Annotate",
    ] {
        assert!(!actions.is_enabled(id), "{id}");
    }
}

#[test]
fn local_changes_enable_commit_stash_and_selected_change_actions() {
    let (mut status, refs) = base();
    status.changes = 2;
    status.untracked = 1;
    let selection = NativeMenuSelection {
        current_file: Some(NativeFileSelection {
            source: NativeFileSource::Changes,
            versioned: false,
            untracked: true,
            conflicted: false,
        }),
        selected_change_count: 1,
        ..NativeMenuSelection::default()
    };
    let actions = calculate_actions(Some(&status), Some(&refs), selection);
    for id in [
        "CheckinProject",
        "Git.StashChanges",
        "ChangesView.Revert",
        "ChangesView.CommitFile",
        "ChangesView.AddUnversioned",
    ] {
        assert!(actions.is_enabled(id), "{id}");
    }
    for id in ["Git.CurrentFile.ShowHistory", "Git.CurrentFile.Annotate"] {
        assert!(!actions.is_enabled(id), "{id}");
    }
}

#[test]
fn conflicts_and_merge_or_rebase_operations_gate_the_expected_actions() {
    let (mut status, refs) = base();
    status.conflicts = 1;
    status.operation = Some(OperationKind::Merge);
    let actions = calculate_actions(Some(&status), Some(&refs), NativeMenuSelection::default());
    assert!(actions.is_enabled("Git.Merge.Abort"));
    assert!(!actions.is_enabled("Git.Merge.Commit"));
    assert!(actions.is_enabled("Git.ResolveConflicts"));
    assert!(actions.is_enabled("CheckinProject"));
    for id in [
        "Git.Merge",
        "Git.Rebase",
        "Git.StashChanges",
        "Git.CurrentFile.Annotate",
    ] {
        assert!(!actions.is_enabled(id), "{id}");
    }

    status.conflicts = 0;
    let actions = calculate_actions(Some(&status), Some(&refs), NativeMenuSelection::default());
    assert!(actions.is_enabled("Git.Merge.Commit"));
    assert!(actions.is_enabled("Git.Merge.Abort"));
    assert!(actions.is_enabled("CheckinProject"));

    status.operation = Some(OperationKind::Rebase);
    let actions = calculate_actions(Some(&status), Some(&refs), NativeMenuSelection::default());
    assert!(actions.is_enabled("Git.Rebase.Continue"));
    assert!(actions.is_enabled("Git.Rebase.Abort"));
    assert!(actions.is_enabled("Git.Rebase.Skip"));
    assert!(actions.is_enabled("CheckinProject"));
    assert!(!actions.is_enabled("Git.Merge"));
}

#[test]
fn detached_unborn_and_busy_states_gate_branch_writes() {
    let (mut status, refs) = base();
    status.attached = false;
    let detached = calculate_actions(Some(&status), Some(&refs), NativeMenuSelection::default());
    for id in [
        "Vcs.Push",
        "Vcs.UpdateProject",
        "Git.Pull",
        "Git.Merge",
        "Git.Rebase",
        "Git.Reset",
    ] {
        assert!(!detached.is_enabled(id), "{id}");
    }
    assert!(detached.is_enabled("Git.Tag"));

    status.has_head = false;
    status.attached = true;
    let unborn = calculate_actions(Some(&status), Some(&refs), NativeMenuSelection::default());
    for id in [
        "Vcs.Push",
        "Vcs.UpdateProject",
        "Git.Merge",
        "Git.Rebase",
        "Git.CreateNewBranch",
        "Git.Tag",
        "Git.Reset",
    ] {
        assert!(!unborn.is_enabled(id), "{id}");
    }
    assert!(unborn.is_enabled("Git.Branches"));

    status.has_head = true;
    let busy = calculate_actions(
        Some(&status),
        Some(&refs),
        NativeMenuSelection {
            busy: true,
            ..NativeMenuSelection::default()
        },
    );
    assert!(!busy.is_enabled("Git.Pull"));
    assert!(!busy.is_enabled("CheckinProject"));
}

#[test]
fn current_file_actions_follow_file_kind_and_selected_commit() {
    let (status, refs) = base();
    let selection = NativeMenuSelection {
        current_file: Some(NativeFileSelection {
            source: NativeFileSource::Changes,
            versioned: true,
            untracked: false,
            conflicted: false,
        }),
        selected_commit_count: 1,
        ..NativeMenuSelection::default()
    };
    let actions = calculate_actions(Some(&status), Some(&refs), selection);
    assert!(actions.is_enabled("Git.CurrentFile.ShowHistory"));
    assert!(actions.is_enabled("Git.CurrentFile.Annotate"));
    assert!(actions.is_enabled("Git.CurrentFile.CompareRevision"));
    assert!(actions.is_enabled("Git.CurrentFile.CompareBranch"));
    assert!(actions.is_enabled("Git.Reset"));

    let conflict = NativeMenuSelection {
        current_file: Some(NativeFileSelection {
            conflicted: true,
            ..selection.current_file.unwrap()
        }),
        ..selection
    };
    let actions = calculate_actions(Some(&status), Some(&refs), conflict);
    assert!(!actions.is_enabled("Git.CurrentFile.ShowHistory"));
}

#[test]
fn identical_menu_state_is_deduplicated() {
    let mut context = ProjectMenuContext::default();
    let status = summary(1);
    assert!(context.set_status(&status));
    assert!(!context.set_status(&status));
    let references = gitwood_core::repo::refs::References {
        local: vec![],
        remotes: Default::default(),
        tags: vec![],
        remote_heads: Default::default(),
        head: Default::default(),
    };
    assert!(context.set_references(&references));
    assert!(!context.set_references(&references));
    assert!(!context.set_selection(NativeMenuSelection::default()));
    let selected = NativeMenuSelection {
        selected_commit_count: 1,
        ..NativeMenuSelection::default()
    };
    assert!(context.set_selection(selected));
    assert!(!context.set_selection(selected));
}

#[test]
fn project_windows_keep_native_menu_contexts_independent() {
    let windows = ProjectWindows::default();
    windows.insert("main".into(), Layout::default(), None);
    windows.insert("project-1".into(), Layout::default(), None);
    let first = windows.get("main").unwrap();
    let second = windows.get("project-1").unwrap();
    first.native_menu.lock().unwrap().set_status(&summary(1));

    assert!(first
        .native_menu
        .lock()
        .unwrap()
        .presentation()
        .is_enabled("CheckinProject"));
    assert!(!second
        .native_menu
        .lock()
        .unwrap()
        .presentation()
        .is_enabled("CheckinProject"));
}

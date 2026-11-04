import type { Operation, OperationKind, RepoPath } from '../browse/types';
import type { RemoteError } from './model';
import { operationText, type OperationMessage } from '../../i18n/operation';

export type SequenceAction = 'continue' | 'abort' | 'skip';
export type SequenceOutcome =
  | { kind: 'completed' }
  | { kind: 'conflicts'; operation: Operation; files: RepoPath[] }
  | { kind: 'stopped'; operation: Operation; detail: string };
/** `resolve` / `abort` match IDEA GitMergeRebaseWidget button colors. */
export interface OperationAction { action: SequenceAction; label: string; tone: 'resolve' | 'abort' | 'plain' }

const names: Record<OperationKind, OperationMessage> = {
  merge: 'mergeName', rebase: 'rebaseName', applyMailbox: 'applyMailboxName', cherryPick: 'cherryPickName', revert: 'revertName', bisect: 'bisectName',
};
export const operationName = (kind: OperationKind) => operationText(names[kind]);

/** IDEA GitBranchUtil.getDisplayableBranchText: "Rebasing main", "Cherry-picking in main". */
export function operationLabel(operation: Operation, currentBranch: string | null): string {
  const branch = operation.branch ?? currentBranch ?? 'HEAD';
  const text = operationText(operation.kind, { branch });
  return operation.step && operation.total ? `${text} ${operationText('step', { step: operation.step, total: operation.total })}` : text;
}

/**
 * IDEA Git.Ongoing.Rebase.Actions / GitMergeRebaseWidgetGroup, in widget order. Revert offers only
 * Abort, as in IDEA; bisect has no Git menu actions.
 */
export function operationActions(operation: Operation | null): OperationAction[] {
  const item = (action: SequenceAction, key: OperationMessage): OperationAction =>
    ({ action, label: operationText(key), tone: action === 'continue' ? 'resolve' : action === 'abort' ? 'abort' : 'plain' });
  switch (operation?.kind) {
    case 'rebase': return [item('continue', 'continueRebase'), item('abort', 'abortRebase'), item('skip', 'skipCommit')];
    case 'applyMailbox': return [item('continue', 'continueApply'), item('abort', 'abortApply'), item('skip', 'skipPatch')];
    case 'merge': return [item('continue', 'commitMerge'), item('abort', 'abortMerge')];
    case 'cherryPick': return [item('continue', 'continueCherryPick'), item('abort', 'abortCherryPick')];
    case 'revert': return [item('abort', 'abortRevert')];
    default: return [];
  }
}

/** IDEA abort.operation.dialog.*: confirmation before any abort. */
export function abortConfirmation(kind: OperationKind, project: string): { title: string; message: string } {
  const name = operationName(kind);
  return { title: operationText('abortTitle', { name }), message: operationText('abortMessage', { name: name.toLowerCase(), project }) };
}

export interface SequenceFeedback { kind: 'success' | 'warning' | 'error'; title: string; detail: string; files: string[]; offerActions: boolean }

export function sequenceFeedback(kind: OperationKind, action: SequenceAction, outcome: SequenceOutcome): SequenceFeedback {
  const name = operationName(kind);
  if (outcome.kind === 'completed') {
    const title = action === 'abort' ? operationText('aborted', { name }) : action === 'skip' ? operationText('skipped') : operationText('completed', { name });
    return { kind: 'success', title, detail: '', files: [], offerActions: false };
  }
  const current = operationName(outcome.operation.kind);
  if (outcome.kind === 'conflicts') {
    return { kind: 'warning', title: operationText('stoppedConflicts', { name: current }), detail: operationText('conflicts', { count: outcome.files.length }),
      files: outcome.files.map(file => file.display), offerActions: true };
  }
  return { kind: 'warning', title: operationText('stopped', { name: current }), detail: outcome.detail, files: [], offerActions: true };
}

export function sequenceErrorText(kind: OperationKind | null, error: RemoteError): { title: string; detail: string } {
  const title = operationText('failed', { name: kind ? operationName(kind) : 'Git' });
  const known: Record<string, OperationMessage> = {
    'sequence.unresolved': 'unresolved', 'sequence.none': 'none', 'sequence.unsupported': 'unsupported', 'sequence.stillInProgress': 'stillInProgress',
  };
  const key = error.key ? known[error.key] : undefined;
  return { title, detail: key ? operationText(key) : error.detail ?? '' };
}

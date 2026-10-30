import { expect, it } from 'vitest';
import { CommitSession } from '../src/features/commit/session.svelte';
it('fills the full undone message and keeps both the user input and pre-amend draft recoverable', () => {
  const session = new CommitSession();
  session.message = 'current input'; session.draft = 'before amend'; session.amend = true;
  session.restoreUndoMessage('subject\n\nbody\n\ntrailer: value\n');
  expect(session.message).toBe('subject\n\nbody\n\ntrailer: value\n');
  expect(session.amend).toBe(false);
  expect(session.history).toEqual(['current input', 'before amend']);
  expect(session.draft).toBe('current input');
});

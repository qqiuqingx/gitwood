import { expect, it } from 'vitest';
import { render } from 'svelte/server';
import CredentialDialog from '../src/features/git/CredentialDialog.svelte';
import { credentialAnswer, credentialHost, credentialSecret, withoutCredential, type CredentialRequest } from '../src/features/git/credential';

Object.defineProperty(globalThis.navigator, 'language', { configurable: true, value: 'en-US' });
const request = (kind: CredentialRequest['kind'], prompt: string): CredentialRequest => ({ generation: 3, operation: 1, requestId: 1, kind, prompt });
const body = (value: CredentialRequest | null) => render(CredentialDialog, { props: { request: value, answer: () => {} } }).body;

it('renders nothing while no prompt is pending', () => {
  expect(body(null)).not.toContain('<dialog');
});

it('takes the host from the prompt and shows it in the title', () => {
  const username = body(request('username', "Username for 'https://git.example.com/repo': "));
  expect(username).toContain('Git Credentials — git.example.com');
  expect(username).toContain('type="text"');
  expect(credentialHost("Enter passphrase for key '/home/me/.ssh/id_ed25519': ")).toBeNull();
  expect(credentialHost("Password for 'https://me:secret@example.com': ")).toBe('example.com');
});

it('masks passwords, passphrases and unrecognised prompts', () => {
  for (const kind of ['password', 'passphrase', 'generic'] as const) {
    const markup = body(request(kind, 'Password for \'https://example.com\': '));
    expect(markup).toContain('type="password"');
    expect(markup).toContain('Gitwood does not store credentials');
    expect(credentialSecret(kind)).toBe(true);
  }
  expect(credentialSecret('username')).toBe(false);
});

it('confirms a host key without asking for a value', () => {
  const markup = body(request('hostKey', 'Are you sure you want to continue connecting (yes/no/[fingerprint])?'));
  expect(markup).toContain('Continue Connecting');
  expect(markup).not.toContain('type="password"');
  expect(markup).not.toContain('<input');
  expect(credentialAnswer('hostKey', '')).toBe('yes');
});

it('renders repository text as plain text, never as markup', () => {
  const markup = body(request('generic', 'Token for <img src=x onerror=alert(1)>: '));
  expect(markup).toContain('&lt;img src=x onerror=alert(1)>');
  expect(markup).not.toContain('<img src=x');
});

it('cancels rather than sending an empty credential', () => {
  expect(credentialAnswer('username', '   ')).toBeNull();
  expect(credentialAnswer('password', '')).toBeNull();
});

it('sends secrets exactly as typed and trims only the username', () => {
  expect(credentialAnswer('username', ' alice ')).toBe('alice');
  expect(credentialAnswer('password', ' hunter2 ')).toBe(' hunter2 ');
  expect(credentialAnswer('passphrase', '   ')).toBe('   ');
  expect(credentialAnswer('generic', ' token')).toBe(' token');
});

it('closes only the prompt of the operation that ended', () => {
  const stale = request('username', "Username for 'https://example.com': ");
  const next = { ...request('password', "Password for 'https://example.com': "), operation: 2 };
  // Same generation and ordinal: only the operation tells them apart.
  expect(withoutCredential([stale, next], stale)).toEqual([next]);
  expect(withoutCredential([stale, next], { generation: 3, operation: 2, requestId: 1 })).toEqual([stale]);
});

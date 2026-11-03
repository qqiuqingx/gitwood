/** Credential prompts relayed from Git and SSH; the dialog only presents one request. */
export type CredentialKind = 'username' | 'password' | 'passphrase' | 'hostKey' | 'generic';

/** Identifies one prompt; request ordinals restart for every remote operation. */
export interface CredentialKey {
  generation: number;
  operation: number;
  requestId: number;
}

export interface CredentialRequest extends CredentialKey {
  kind: CredentialKind;
  /** Prompt text from Git/SSH with embedded URL credentials already removed by Rust. */
  prompt: string;
}

/** Host from the prompt URL, shown in the dialog title like IDEA's login dialog. */
export function credentialHost(prompt: string): string | null {
  const match = /[a-z][a-z0-9+.-]*:\/\/([^/'\s]+)/i.exec(prompt);
  return match ? match[1].replace(/^[^@]*@/, '') : null;
}

/** Secret fields are masked; a username and a host-key confirmation are not. */
export function credentialSecret(kind: CredentialKind): boolean {
  return kind === 'password' || kind === 'passphrase' || kind === 'generic';
}

/** Field label for the input, `null` when the prompt has no input (host key confirmation). */
export function credentialLabel(
  kind: CredentialKind,
): 'username' | 'password' | 'passphrase' | 'generic' | null {
  return kind === 'hostKey' ? null : kind;
}

/**
 * Answer to send back. Host-key confirmation is always `yes`. A username is trimmed;
 * secrets are sent exactly as typed, since spaces may be part of a password or passphrase.
 * An empty answer cancels, which aborts the operation instead of sending an empty credential.
 */
export function credentialAnswer(kind: CredentialKind, value: string): string | null {
  if (kind === 'hostKey') return 'yes';
  const text = credentialSecret(kind) ? value : value.trim();
  return text.length ? text : null;
}

/** Drop a prompt that was answered or whose operation ended, wherever it is queued. */
export function withoutCredential(queue: CredentialRequest[], key: CredentialKey): CredentialRequest[] {
  return queue.filter(request => request.generation !== key.generation
    || request.operation !== key.operation || request.requestId !== key.requestId);
}

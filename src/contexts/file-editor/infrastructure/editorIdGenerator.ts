export function createEditorTabId(): string {
  return crypto.randomUUID();
}

export function createEditorRevealNonce(): string {
  return crypto.randomUUID();
}

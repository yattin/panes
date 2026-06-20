export function createTerminalGroupId(): string {
  return crypto.randomUUID();
}

export function createTerminalSplitId(): string {
  return crypto.randomUUID();
}

export function createTerminalWorktreeRunId(): string {
  return crypto.randomUUID().slice(0, 8);
}

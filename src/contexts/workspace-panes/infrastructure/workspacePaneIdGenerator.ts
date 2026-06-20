export function createWorkspacePaneId(prefix: string): string {
  return `${prefix}-${crypto.randomUUID()}`;
}

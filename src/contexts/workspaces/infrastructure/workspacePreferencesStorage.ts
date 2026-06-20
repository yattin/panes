const LAST_WORKSPACE_KEY = "panes:lastActiveWorkspaceId";
const LAST_REPO_BY_WORKSPACE_KEY = "panes:lastActiveRepoByWorkspace";

export type LastRepoByWorkspace = Record<string, string>;

export function readLastWorkspaceId(): string | null {
  try {
    return localStorage.getItem(LAST_WORKSPACE_KEY);
  } catch {
    return null;
  }
}

export function writeLastWorkspaceId(workspaceId: string): void {
  try {
    localStorage.setItem(LAST_WORKSPACE_KEY, workspaceId);
  } catch {
    // localStorage unavailable or full; ignore persistence failure.
  }
}

export function readLastRepoByWorkspace(): LastRepoByWorkspace {
  try {
    const raw = localStorage.getItem(LAST_REPO_BY_WORKSPACE_KEY);
    if (!raw) {
      return {};
    }

    const parsed = JSON.parse(raw) as unknown;
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      return {};
    }

    const next: LastRepoByWorkspace = {};
    for (const [workspaceId, repoId] of Object.entries(parsed as Record<string, unknown>)) {
      if (typeof workspaceId !== "string" || typeof repoId !== "string") {
        continue;
      }
      const normalizedRepoId = repoId.trim();
      if (!normalizedRepoId) {
        continue;
      }
      next[workspaceId] = normalizedRepoId;
    }
    return next;
  } catch {
    return {};
  }
}

export function writeLastRepoByWorkspace(next: LastRepoByWorkspace): void {
  try {
    localStorage.setItem(LAST_REPO_BY_WORKSPACE_KEY, JSON.stringify(next));
  } catch {
    // localStorage unavailable or full; ignore persistence failure.
  }
}

export function rememberLastRepo(workspaceId: string, repoId: string): void {
  const current = readLastRepoByWorkspace();
  if (current[workspaceId] === repoId) {
    return;
  }
  current[workspaceId] = repoId;
  writeLastRepoByWorkspace(current);
}

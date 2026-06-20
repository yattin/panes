import type { Repo, Workspace } from "../../../types";

type RepoRoot = Pick<Repo, "id" | "isActive">;

export function isTransientLinuxAppImageRoot(rootPath: string): boolean {
  return /^\/(?:var\/tmp|tmp)\/\.mount_[^/]+(?:\/|$)/.test(rootPath);
}

export function resolveStartupWorkspaceId(
  workspaces: Workspace[],
  savedId: string | null,
): string | null {
  const savedWorkspace = savedId
    ? workspaces.find((workspace) => workspace.id === savedId) ?? null
    : null;

  if (savedWorkspace && !isTransientLinuxAppImageRoot(savedWorkspace.rootPath)) {
    return savedWorkspace.id;
  }

  if (!savedId) {
    return null;
  }

  return (
    workspaces.find((workspace) => !isTransientLinuxAppImageRoot(workspace.rootPath))?.id ?? null
  );
}

export function resolveActiveRepoIdFromSelection(
  workspaceId: string,
  repos: RepoRoot[],
  currentActiveRepoId: string | null,
  lastRepoByWorkspace: Record<string, string>,
): string | null {
  if (!repos.length) {
    return null;
  }

  if (currentActiveRepoId && repos.some((repo) => repo.id === currentActiveRepoId)) {
    return currentActiveRepoId;
  }

  const persisted = lastRepoByWorkspace[workspaceId];
  if (persisted && repos.some((repo) => repo.id === persisted && repo.isActive)) {
    return persisted;
  }

  return repos.find((repo) => repo.isActive)?.id ?? repos[0]?.id ?? null;
}

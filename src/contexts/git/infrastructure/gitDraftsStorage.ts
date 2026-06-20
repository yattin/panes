import {
  EMPTY_GIT_DRAFTS,
  type GitDraftsPayload,
  normalizeGitDrafts,
} from "../domain/gitDrafts";

export function gitDraftStorageKey(workspaceId: string): string {
  return `panes:git.drafts:${workspaceId}`;
}

function getLocalStorage(): Storage | undefined {
  try {
    return globalThis.localStorage;
  } catch {
    return undefined;
  }
}

export function readStoredGitDrafts(workspaceId: string): GitDraftsPayload {
  try {
    const raw = getLocalStorage()?.getItem(gitDraftStorageKey(workspaceId));
    if (!raw) return { ...EMPTY_GIT_DRAFTS };
    return normalizeGitDrafts(JSON.parse(raw));
  } catch {
    return { ...EMPTY_GIT_DRAFTS };
  }
}

export function writeStoredGitDrafts(workspaceId: string, payload: GitDraftsPayload): void {
  try {
    getLocalStorage()?.setItem(gitDraftStorageKey(workspaceId), JSON.stringify(payload));
  } catch {
    // localStorage full or unavailable: draft persistence is best-effort.
  }
}

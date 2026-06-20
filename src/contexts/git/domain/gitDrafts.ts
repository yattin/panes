export interface GitDraftsPayload {
  commitMessage: string;
  branchName: string;
  commitHistory: string[];
  branchHistory: string[];
}

export const GIT_DRAFT_HISTORY_MAX = 3;

export const EMPTY_GIT_DRAFTS: GitDraftsPayload = {
  commitMessage: "",
  branchName: "",
  commitHistory: [],
  branchHistory: [],
};

function normalizeHistory(value: unknown): string[] {
  if (!Array.isArray(value)) {
    return [];
  }

  const normalized: string[] = [];
  for (const entry of value) {
    if (typeof entry !== "string") {
      continue;
    }
    const trimmed = entry.trim();
    if (!trimmed || normalized.includes(trimmed)) {
      continue;
    }
    normalized.push(trimmed);
    if (normalized.length >= GIT_DRAFT_HISTORY_MAX) {
      break;
    }
  }
  return normalized;
}

export function normalizeGitDrafts(value: unknown): GitDraftsPayload {
  const parsed = value && typeof value === "object" ? (value as Partial<GitDraftsPayload>) : {};
  return {
    commitMessage: typeof parsed.commitMessage === "string" ? parsed.commitMessage : "",
    branchName: typeof parsed.branchName === "string" ? parsed.branchName : "",
    commitHistory: normalizeHistory(parsed.commitHistory),
    branchHistory: normalizeHistory(parsed.branchHistory),
  };
}

export function addToGitDraftHistory(history: readonly string[], entry: string): string[] {
  return normalizeHistory([entry, ...history]);
}

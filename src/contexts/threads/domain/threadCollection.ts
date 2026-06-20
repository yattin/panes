import type { Thread } from "../../../types";

export function mergeWorkspaceThreads(
  current: Record<string, Thread[]>,
  workspaceId: string,
  threads: Thread[],
): Record<string, Thread[]> {
  return {
    ...current,
    [workspaceId]: threads,
  };
}

export function flattenThreadsByWorkspace(threadsByWorkspace: Record<string, Thread[]>): Thread[] {
  return Object.values(threadsByWorkspace)
    .flat()
    .sort(compareThreadsByRecentActivity);
}

function threadActivityTime(thread: Thread): number {
  const parsed = Date.parse(thread.lastActivityAt);
  return Number.isFinite(parsed) ? parsed : 0;
}

function compareThreadsByRecentActivity(a: Thread, b: Thread): number {
  return threadActivityTime(b) - threadActivityTime(a);
}

export function applyThreadReasoningEffort(
  thread: Thread,
  reasoningEffort: string | null,
): Thread {
  const metadata = { ...(thread.engineMetadata ?? {}) };
  if (reasoningEffort) {
    metadata.reasoningEffort = reasoningEffort;
  } else {
    delete metadata.reasoningEffort;
  }

  return {
    ...thread,
    engineMetadata: Object.keys(metadata).length ? metadata : undefined,
  };
}

export function applyThreadLastModel(
  thread: Thread,
  modelId: string | null,
): Thread {
  const metadata = { ...(thread.engineMetadata ?? {}) };
  if (modelId) {
    metadata.lastModelId = modelId;
  } else {
    delete metadata.lastModelId;
  }

  return {
    ...thread,
    engineMetadata: Object.keys(metadata).length ? metadata : undefined,
  };
}

function readThreadLastModelId(thread: Thread): string | null {
  const raw = thread.engineMetadata?.lastModelId;
  if (typeof raw !== "string") {
    return null;
  }
  const normalized = raw.trim();
  return normalized.length > 0 ? normalized : null;
}

export function threadMatchesRequestedModel(thread: Thread, modelId: string): boolean {
  return thread.modelId === modelId || readThreadLastModelId(thread) === modelId;
}

export interface ThreadScopeSelectionInput {
  threads: Thread[];
  repoId: string | null;
  engineId: string;
  modelId: string;
  activeThreadId: string | null;
}

export function selectThreadForScope({
  threads,
  repoId,
  engineId,
  modelId,
  activeThreadId,
}: ThreadScopeSelectionInput): Thread | null {
  const scopedForModel = threads
    .filter((thread) => thread.repoId === repoId && thread.engineId === engineId)
    .filter((thread) => threadMatchesRequestedModel(thread, modelId))
    .sort(compareThreadsByRecentActivity);

  return (
    scopedForModel.find((thread) => thread.id === activeThreadId) ??
    scopedForModel[0] ??
    null
  );
}

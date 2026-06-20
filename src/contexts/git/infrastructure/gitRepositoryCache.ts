import type { GitDiffPreview, GitStatus } from "../../../types";
import {
  isCurrentGitRequestGeneration,
  isGitCacheEntryFresh,
  nextGitRequestGeneration,
  shouldReuseGitInFlightRequest,
} from "../domain/gitCachePolicy";

const GIT_STATUS_CACHE_TTL_MS = 1_000;
const GIT_DIFF_CACHE_TTL_MS = 1_200;
const GIT_STATUS_CACHE_MAX_ENTRIES = 32;
const GIT_DIFF_CACHE_MAX_ENTRIES = 320;
const GIT_STATUS_CACHE_MAX_BYTES = 3 * 1024 * 1024;
const GIT_DIFF_CACHE_MAX_BYTES = 24 * 1024 * 1024;

type LoadGitStatus = (repoPath: string) => Promise<GitStatus>;
type LoadGitDiff = (
  repoPath: string,
  filePath: string,
  staged: boolean,
) => Promise<GitDiffPreview>;

interface GitStatusCacheEntry {
  status: GitStatus;
  revision: number;
  updatedAt: number;
}

interface GitDiffCacheEntry {
  diff: GitDiffPreview;
  revision: number;
  updatedAt: number;
}

const repoRevisionByPath = new Map<string, number>();
const statusCacheByRepo = new Map<string, GitStatusCacheEntry>();
const statusInFlightByRepo = new Map<string, Promise<GitStatus>>();
const statusRequestGenerationByRepo = new Map<string, number>();
const diffCacheByKey = new Map<string, GitDiffCacheEntry>();
const diffInFlightByKey = new Map<string, Promise<GitDiffPreview>>();
const diffRequestGenerationByKey = new Map<string, number>();
let statusCacheBytes = 0;
let diffCacheBytes = 0;

function estimateStatusCacheEntryBytes(repoPath: string, entry: GitStatusCacheEntry): number {
  let bytes = repoPath.length * 2 + entry.status.branch.length * 2 + 96;
  for (const file of entry.status.files) {
    bytes += file.path.length * 2;
    bytes += (file.indexStatus?.length ?? 0) * 2;
    bytes += (file.worktreeStatus?.length ?? 0) * 2;
    bytes += 48;
  }
  return bytes;
}

function estimateDiffCacheEntryBytes(key: string, entry: GitDiffCacheEntry): number {
  return (key.length + entry.diff.content.length) * 2 + 128;
}

function removeStatusCacheEntry(repoPath: string) {
  const existing = statusCacheByRepo.get(repoPath);
  if (!existing) {
    return;
  }
  statusCacheBytes = Math.max(
    0,
    statusCacheBytes - estimateStatusCacheEntryBytes(repoPath, existing),
  );
  statusCacheByRepo.delete(repoPath);
}

function removeDiffCacheEntry(key: string) {
  const existing = diffCacheByKey.get(key);
  if (!existing) {
    return;
  }
  diffCacheBytes = Math.max(0, diffCacheBytes - estimateDiffCacheEntryBytes(key, existing));
  diffCacheByKey.delete(key);
}

function trimStatusCacheToLimits() {
  while (
    statusCacheByRepo.size > GIT_STATUS_CACHE_MAX_ENTRIES ||
    statusCacheBytes > GIT_STATUS_CACHE_MAX_BYTES
  ) {
    let oldestKey: string | null = null;
    let oldestUpdatedAt = Number.POSITIVE_INFINITY;

    for (const [key, entry] of statusCacheByRepo.entries()) {
      if (entry.updatedAt < oldestUpdatedAt) {
        oldestUpdatedAt = entry.updatedAt;
        oldestKey = key;
      }
    }

    if (!oldestKey) {
      break;
    }
    removeStatusCacheEntry(oldestKey);
  }
}

function trimDiffCacheToLimits() {
  while (
    diffCacheByKey.size > GIT_DIFF_CACHE_MAX_ENTRIES ||
    diffCacheBytes > GIT_DIFF_CACHE_MAX_BYTES
  ) {
    let oldestKey: string | null = null;
    let oldestUpdatedAt = Number.POSITIVE_INFINITY;

    for (const [key, entry] of diffCacheByKey.entries()) {
      if (entry.updatedAt < oldestUpdatedAt) {
        oldestUpdatedAt = entry.updatedAt;
        oldestKey = key;
      }
    }

    if (!oldestKey) {
      break;
    }
    removeDiffCacheEntry(oldestKey);
  }
}

function setStatusCacheEntry(repoPath: string, entry: GitStatusCacheEntry) {
  removeStatusCacheEntry(repoPath);
  statusCacheByRepo.set(repoPath, entry);
  statusCacheBytes += estimateStatusCacheEntryBytes(repoPath, entry);
  trimStatusCacheToLimits();
}

function setDiffCacheEntry(key: string, entry: GitDiffCacheEntry) {
  removeDiffCacheEntry(key);
  diffCacheByKey.set(key, entry);
  diffCacheBytes += estimateDiffCacheEntryBytes(key, entry);
  trimDiffCacheToLimits();
}

function getRepoRevision(repoPath: string): number {
  return repoRevisionByPath.get(repoPath) ?? 0;
}

function incrementRepoRevision(repoPath: string): number {
  const next = getRepoRevision(repoPath) + 1;
  repoRevisionByPath.set(repoPath, next);
  return next;
}

function buildDiffCacheKey(repoPath: string, filePath: string, staged: boolean): string {
  return `${repoPath}::${staged ? "staged" : "worktree"}::${filePath}`;
}

export function invalidateGitRepoCaches(repoPath: string) {
  incrementRepoRevision(repoPath);
  removeStatusCacheEntry(repoPath);
  statusInFlightByRepo.delete(repoPath);
  for (const key of [...diffCacheByKey.keys()]) {
    if (key.startsWith(`${repoPath}::`)) {
      removeDiffCacheEntry(key);
    }
  }
  for (const key of diffInFlightByKey.keys()) {
    if (key.startsWith(`${repoPath}::`)) {
      diffInFlightByKey.delete(key);
    }
  }
}

export async function getGitStatusCached(
  repoPath: string,
  loadStatus: LoadGitStatus,
  force = false,
): Promise<GitStatus> {
  const revision = getRepoRevision(repoPath);
  const now = performance.now();
  const cached = statusCacheByRepo.get(repoPath);
  if (
    !force &&
    cached &&
    isGitCacheEntryFresh(
      cached.revision,
      revision,
      cached.updatedAt,
      now,
      GIT_STATUS_CACHE_TTL_MS,
    )
  ) {
    setStatusCacheEntry(repoPath, {
      ...cached,
      updatedAt: now,
    });
    return cached.status;
  }

  const inFlight = statusInFlightByRepo.get(repoPath);
  if (inFlight && shouldReuseGitInFlightRequest(force)) {
    return inFlight;
  }

  const requestRevision = revision;
  const requestGeneration = nextGitRequestGeneration(statusRequestGenerationByRepo.get(repoPath));
  statusRequestGenerationByRepo.set(repoPath, requestGeneration);
  const requestPromise = loadStatus(repoPath)
    .then((status) => {
      if (
        getRepoRevision(repoPath) === requestRevision &&
        isCurrentGitRequestGeneration(
          statusRequestGenerationByRepo.get(repoPath),
          requestGeneration,
        )
      ) {
        setStatusCacheEntry(repoPath, {
          status,
          revision: requestRevision,
          updatedAt: performance.now(),
        });
      }
      return status;
    })
    .finally(() => {
      if (statusInFlightByRepo.get(repoPath) === requestPromise) {
        statusInFlightByRepo.delete(repoPath);
      }
    });

  statusInFlightByRepo.set(repoPath, requestPromise);
  return requestPromise;
}

export async function getGitDiffCached(
  repoPath: string,
  filePath: string,
  staged: boolean,
  loadDiff: LoadGitDiff,
  force = false,
): Promise<GitDiffPreview> {
  const key = buildDiffCacheKey(repoPath, filePath, staged);
  const revision = getRepoRevision(repoPath);
  const now = performance.now();
  const cached = diffCacheByKey.get(key);
  if (
    !force &&
    cached &&
    isGitCacheEntryFresh(
      cached.revision,
      revision,
      cached.updatedAt,
      now,
      GIT_DIFF_CACHE_TTL_MS,
    )
  ) {
    setDiffCacheEntry(key, {
      ...cached,
      updatedAt: now,
    });
    return cached.diff;
  }

  const inFlight = diffInFlightByKey.get(key);
  if (inFlight && shouldReuseGitInFlightRequest(force)) {
    return inFlight;
  }

  const requestRevision = revision;
  const requestGeneration = nextGitRequestGeneration(diffRequestGenerationByKey.get(key));
  diffRequestGenerationByKey.set(key, requestGeneration);
  const requestPromise = loadDiff(repoPath, filePath, staged)
    .then((diff) => {
      if (
        getRepoRevision(repoPath) === requestRevision &&
        isCurrentGitRequestGeneration(diffRequestGenerationByKey.get(key), requestGeneration)
      ) {
        setDiffCacheEntry(key, {
          diff,
          revision: requestRevision,
          updatedAt: performance.now(),
        });
      }
      return diff;
    })
    .finally(() => {
      if (diffInFlightByKey.get(key) === requestPromise) {
        diffInFlightByKey.delete(key);
      }
    });

  diffInFlightByKey.set(key, requestPromise);
  return requestPromise;
}

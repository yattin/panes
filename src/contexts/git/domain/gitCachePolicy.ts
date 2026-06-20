export function shouldReuseGitInFlightRequest(force: boolean): boolean {
  return !force;
}

export function isGitCacheEntryFresh(
  entryRevision: number,
  currentRevision: number,
  updatedAt: number,
  now: number,
  ttlMs: number,
): boolean {
  return entryRevision === currentRevision && now - updatedAt <= ttlMs;
}

export function nextGitRequestGeneration(currentGeneration: number | undefined): number {
  return (currentGeneration ?? 0) + 1;
}

export function isCurrentGitRequestGeneration(
  currentGeneration: number | undefined,
  requestGeneration: number,
): boolean {
  return currentGeneration === requestGeneration;
}

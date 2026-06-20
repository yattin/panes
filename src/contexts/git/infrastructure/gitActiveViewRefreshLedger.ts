import {
  buildGitActiveViewRefreshKey,
  type GitActiveViewState,
  shouldRefreshGitActiveView,
} from "../domain/gitPanelView";

const refreshedAtByKey = new Map<string, number>();

export function invalidateGitActiveViewRefreshes(repoPath: string) {
  for (const key of refreshedAtByKey.keys()) {
    if (key.startsWith(`${repoPath}::`)) {
      refreshedAtByKey.delete(key);
    }
  }
}

export function shouldRefreshGitActiveViewForRepo(
  repoPath: string,
  state: GitActiveViewState,
  force: boolean,
): boolean {
  const key = buildGitActiveViewRefreshKey(repoPath, state);
  return shouldRefreshGitActiveView(
    repoPath,
    state,
    force,
    refreshedAtByKey.get(key),
    performance.now(),
  );
}

export function markGitActiveViewRefreshed(repoPath: string, state: GitActiveViewState) {
  if (state.activeView === "changes") {
    return;
  }
  refreshedAtByKey.set(buildGitActiveViewRefreshKey(repoPath, state), performance.now());
}

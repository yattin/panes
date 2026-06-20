import type { GitBranchScope } from "../../../types";

export type GitPanelView = "changes" | "branches" | "commits" | "stash" | "worktrees";

export interface GitActiveViewState {
  activeView: GitPanelView;
  branchScope: GitBranchScope;
  branchSearch: string;
}

export const GIT_ACTIVE_VIEW_REFRESH_MIN_INTERVAL_MS = 1_500;

export function buildGitActiveViewRefreshKey(
  repoPath: string,
  state: GitActiveViewState,
): string {
  if (state.activeView === "branches") {
    return `${repoPath}::branches::${state.branchScope}::${state.branchSearch}`;
  }
  return `${repoPath}::${state.activeView}`;
}

export function shouldRefreshGitActiveView(
  repoPath: string,
  state: GitActiveViewState,
  force: boolean,
  lastRefreshedAt: number | undefined,
  now: number,
): boolean {
  if (state.activeView === "changes") {
    return false;
  }
  if (force || lastRefreshedAt === undefined) {
    return true;
  }
  return now - lastRefreshedAt >= GIT_ACTIVE_VIEW_REFRESH_MIN_INTERVAL_MS;
}

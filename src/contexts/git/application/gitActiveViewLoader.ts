import type { GitBranch, GitCommit, GitStash, GitWorktree } from "../../../types";
import { gitBranchSearchParam } from "../domain/gitBranchFilters";
import type { GitActiveViewState } from "../domain/gitPanelView";
import { normalizeGitPage } from "../domain/gitPagination";
import { getGitGateway } from "./gitGateway";

const BRANCH_PAGE_SIZE = 200;
const COMMIT_PAGE_SIZE = 100;

export interface GitActiveViewData {
  branches?: GitBranch[];
  branchesTotal?: number;
  branchesHasMore?: boolean;
  branchesOffset?: number;
  commits?: GitCommit[];
  commitsOffset?: number;
  commitsHasMore?: boolean;
  commitsTotal?: number;
  stashes?: GitStash[];
  worktrees?: GitWorktree[];
}

export async function refreshGitActiveView(
  repoPath: string,
  state: GitActiveViewState,
): Promise<GitActiveViewData> {
  if (state.activeView === "branches") {
    const branchesPage = normalizeGitPage(
      await getGitGateway().listGitBranches(
        repoPath,
        state.branchScope,
        0,
        BRANCH_PAGE_SIZE,
        gitBranchSearchParam(state.branchSearch),
      ),
      0,
      BRANCH_PAGE_SIZE,
    );
    return {
      branches: branchesPage.entries,
      branchesTotal: branchesPage.total,
      branchesHasMore: branchesPage.hasMore,
      branchesOffset: branchesPage.offset + branchesPage.entries.length,
    };
  }

  if (state.activeView === "commits") {
    const commitsPage = normalizeGitPage(
      await getGitGateway().listGitCommits(repoPath, 0, COMMIT_PAGE_SIZE),
      0,
      COMMIT_PAGE_SIZE,
    );
    return {
      commits: commitsPage.entries,
      commitsOffset: commitsPage.offset + commitsPage.entries.length,
      commitsHasMore: commitsPage.hasMore,
      commitsTotal: commitsPage.total,
    };
  }

  if (state.activeView === "stash") {
    return {
      stashes: await getGitGateway().listGitStashes(repoPath),
    };
  }

  if (state.activeView === "worktrees") {
    return {
      worktrees: await getGitGateway().listGitWorktrees(repoPath),
    };
  }

  return {};
}

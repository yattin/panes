import type {
  GitBranchPage,
  GitBranchScope,
  GitCommitPage,
  GitDiffPreview,
  GitInitRepoStatus,
  GitRemote,
  GitStash,
  GitStatus,
  GitWorktree,
} from "../../../types";
import type { GitDraftsPayload } from "../domain/gitDrafts";
import type { GitActiveViewState } from "../domain/gitPanelView";

export type GitMetricName = "git.refresh.ms" | "git.file_diff.ms";

export interface GitRepoChangedEvent {
  repoPath: string;
}

export interface GitGateway {
  addGitRemote(repoPath: string, name: string, url: string): Promise<void>;
  addGitWorktree(
    repoPath: string,
    worktreePath: string,
    branchName: string,
    baseRef?: string | null,
  ): Promise<GitWorktree>;
  applyGitStash(repoPath: string, stashIndex: number): Promise<void>;
  checkoutGitBranch(repoPath: string, branchName: string, isRemote: boolean): Promise<void>;
  commit(repoPath: string, message: string): Promise<string>;
  createGitBranch(repoPath: string, branchName: string, fromRef?: string | null): Promise<void>;
  deleteGitBranch(repoPath: string, branchName: string, force: boolean): Promise<void>;
  discardFiles(repoPath: string, files: string[]): Promise<void>;
  fetchGit(repoPath: string): Promise<void>;
  getCachedFileDiff(
    repoPath: string,
    filePath: string,
    staged: boolean,
    force?: boolean,
  ): Promise<GitDiffPreview>;
  getCachedGitStatus(repoPath: string, force?: boolean): Promise<GitStatus>;
  getGitStatus(repoPath: string): Promise<GitStatus>;
  getCommitDiff(repoPath: string, commitHash: string): Promise<GitDiffPreview>;
  initGitRepo(rootPath: string, dryRun?: boolean): Promise<GitInitRepoStatus>;
  invalidateActiveViewRefreshes(repoPath: string): void;
  invalidateRepoCaches(repoPath: string): void;
  listenGitRepoChanged(onEvent: (event: GitRepoChangedEvent) => void): Promise<() => void>;
  listGitBranches(
    repoPath: string,
    scope: GitBranchScope,
    offset?: number,
    limit?: number,
    search?: string,
  ): Promise<GitBranchPage>;
  listGitCommits(repoPath: string, offset?: number, limit?: number): Promise<GitCommitPage>;
  listGitRemotes(repoPath: string): Promise<GitRemote[]>;
  listGitStashes(repoPath: string): Promise<GitStash[]>;
  listGitWorktrees(repoPath: string): Promise<GitWorktree[]>;
  markActiveViewRefreshed(repoPath: string, state: GitActiveViewState): void;
  performanceNow(): number;
  popGitStash(repoPath: string, stashIndex: number): Promise<void>;
  pruneGitWorktrees(repoPath: string): Promise<void>;
  pullGit(repoPath: string): Promise<void>;
  pushGit(repoPath: string): Promise<void>;
  pushGitStash(repoPath: string, message?: string): Promise<void>;
  readStoredGitDrafts(workspaceId: string): GitDraftsPayload;
  recordMetric(name: GitMetricName, value: number, meta?: Record<string, unknown>): void;
  removeGitRemote(repoPath: string, name: string): Promise<void>;
  removeGitWorktree(
    repoPath: string,
    worktreePath: string,
    force: boolean,
    branchName?: string | null,
    deleteBranch?: boolean,
  ): Promise<void>;
  renameGitBranch(repoPath: string, oldName: string, newName: string): Promise<void>;
  renameGitRemote(repoPath: string, oldName: string, newName: string): Promise<void>;
  shouldRefreshActiveView(repoPath: string, state: GitActiveViewState, force: boolean): boolean;
  softResetLastCommit(repoPath: string): Promise<void>;
  stageFiles(repoPath: string, files: string[]): Promise<void>;
  unstageFiles(repoPath: string, files: string[]): Promise<void>;
  watchGitRepo(repoPath: string): Promise<void>;
  writeStoredGitDrafts(workspaceId: string, payload: GitDraftsPayload): void;
}

let configuredGitGateway: GitGateway | null = null;

export function configureGitGateway(gateway: GitGateway): void {
  configuredGitGateway = gateway;
}

export function getGitGateway(): GitGateway {
  if (!configuredGitGateway) {
    throw new Error("GitGateway has not been configured.");
  }
  return configuredGitGateway;
}

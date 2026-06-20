import * as ipcModule from "../../../lib/ipc";
import type { GitGateway } from "../application/gitGateway";
import {
  readStoredGitDrafts,
  writeStoredGitDrafts,
} from "./gitDraftsStorage";
import {
  invalidateGitActiveViewRefreshes,
  markGitActiveViewRefreshed,
  shouldRefreshGitActiveViewForRepo,
} from "./gitActiveViewRefreshLedger";
import {
  getGitDiffCached,
  getGitStatusCached,
  invalidateGitRepoCaches,
} from "./gitRepositoryCache";
import { gitRuntime } from "./gitRuntime";
import { gitTelemetry } from "./gitTelemetry";

const { ipc } = ipcModule;

export interface GitRepoChangedEvent {
  repoPath: string;
}

type GitRepoChangedUnlisten = () => void;

export const gitRepository = {
  addGitRemote: ipc.addGitRemote,
  addGitWorktree: ipc.addGitWorktree,
  applyGitStash: ipc.applyGitStash,
  checkoutGitBranch: ipc.checkoutGitBranch,
  commit: ipc.commit,
  createGitBranch: ipc.createGitBranch,
  deleteGitBranch: ipc.deleteGitBranch,
  discardFiles: ipc.discardFiles,
  fetchGit: ipc.fetchGit,
  getCommitDiff: ipc.getCommitDiff,
  getFileDiff: ipc.getFileDiff,
  getGitStatus: ipc.getGitStatus,
  initGitRepo: ipc.initGitRepo,
  listenGitRepoChanged(
    onEvent: (event: GitRepoChangedEvent) => void,
  ): Promise<GitRepoChangedUnlisten> {
    return ipcModule.listenGitRepoChanged(onEvent);
  },
  listGitBranches: ipc.listGitBranches,
  listGitCommits: ipc.listGitCommits,
  listGitRemotes: ipc.listGitRemotes,
  listGitStashes: ipc.listGitStashes,
  listGitWorktrees: ipc.listGitWorktrees,
  popGitStash: ipc.popGitStash,
  pruneGitWorktrees: ipc.pruneGitWorktrees,
  pullGit: ipc.pullGit,
  pushGit: ipc.pushGit,
  pushGitStash: ipc.pushGitStash,
  removeGitRemote: ipc.removeGitRemote,
  removeGitWorktree: ipc.removeGitWorktree,
  renameGitBranch: ipc.renameGitBranch,
  renameGitRemote: ipc.renameGitRemote,
  softResetLastCommit: ipc.softResetLastCommit,
  stageFiles: ipc.stageFiles,
  unstageFiles: ipc.unstageFiles,
  watchGitRepo: ipc.watchGitRepo,
};

export const gitGateway: GitGateway = {
  addGitRemote: gitRepository.addGitRemote,
  addGitWorktree: gitRepository.addGitWorktree,
  applyGitStash: gitRepository.applyGitStash,
  checkoutGitBranch: gitRepository.checkoutGitBranch,
  commit: gitRepository.commit,
  createGitBranch: gitRepository.createGitBranch,
  deleteGitBranch: gitRepository.deleteGitBranch,
  discardFiles: gitRepository.discardFiles,
  fetchGit: gitRepository.fetchGit,
  getCachedFileDiff: (repoPath, filePath, staged, force) =>
    getGitDiffCached(
      repoPath,
      filePath,
      staged,
      (path, file, isStaged) => gitRepository.getFileDiff(path, file, isStaged),
      force,
    ),
  getCachedGitStatus: (repoPath, force) =>
    getGitStatusCached(repoPath, (path) => gitRepository.getGitStatus(path), force),
  getGitStatus: gitRepository.getGitStatus,
  getCommitDiff: gitRepository.getCommitDiff,
  initGitRepo: gitRepository.initGitRepo,
  invalidateActiveViewRefreshes: invalidateGitActiveViewRefreshes,
  invalidateRepoCaches: invalidateGitRepoCaches,
  listenGitRepoChanged: gitRepository.listenGitRepoChanged,
  listGitBranches: gitRepository.listGitBranches,
  listGitCommits: gitRepository.listGitCommits,
  listGitRemotes: gitRepository.listGitRemotes,
  listGitStashes: gitRepository.listGitStashes,
  listGitWorktrees: gitRepository.listGitWorktrees,
  markActiveViewRefreshed: markGitActiveViewRefreshed,
  performanceNow: gitRuntime.performanceNow,
  popGitStash: gitRepository.popGitStash,
  pruneGitWorktrees: gitRepository.pruneGitWorktrees,
  pullGit: gitRepository.pullGit,
  pushGit: gitRepository.pushGit,
  pushGitStash: gitRepository.pushGitStash,
  readStoredGitDrafts,
  recordMetric: gitTelemetry.recordMetric,
  removeGitRemote: gitRepository.removeGitRemote,
  removeGitWorktree: gitRepository.removeGitWorktree,
  renameGitBranch: gitRepository.renameGitBranch,
  renameGitRemote: gitRepository.renameGitRemote,
  shouldRefreshActiveView: shouldRefreshGitActiveViewForRepo,
  softResetLastCommit: gitRepository.softResetLastCommit,
  stageFiles: gitRepository.stageFiles,
  unstageFiles: gitRepository.unstageFiles,
  watchGitRepo: gitRepository.watchGitRepo,
  writeStoredGitDrafts,
};

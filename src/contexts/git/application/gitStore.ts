import { create } from "zustand";
import type {
  GitBranch,
  GitBranchScope,
  GitCommit,
  GitDiffPreview,
  GitRemote,
  GitStash,
  GitStatus,
  GitWorktree,
} from "../../../types";
import {
  addToGitDraftHistory,
  EMPTY_GIT_DRAFTS,
  type GitDraftsPayload,
} from "../domain/gitDrafts";
import { normalizeGitPage } from "../domain/gitPagination";
import type { GitPanelView } from "../domain/gitPanelView";
import {
  gitBranchSearchParam,
  normalizeGitBranchSearch,
} from "../domain/gitBranchFilters";
import { getGitGateway } from "./gitGateway";
import { refreshGitActiveView } from "./gitActiveViewLoader";

const BRANCH_PAGE_SIZE = 200;
const COMMIT_PAGE_SIZE = 100;
export type GitRemoteSyncAction = "fetch" | "pull" | "push";

function invalidateRepoCaches(repoPath: string) {
  getGitGateway().invalidateRepoCaches(repoPath);
  getGitGateway().invalidateActiveViewRefreshes(repoPath);
}

interface GitState {
  status?: GitStatus;
  selectedFile?: string;
  selectedFileStaged?: boolean;
  diff?: GitDiffPreview;
  loading: boolean;
  error?: string;
  activeRepoPath: string | null;
  remoteSyncAction: GitRemoteSyncAction | null;
  remoteSyncRepoPath: string | null;
  activeView: GitPanelView;
  branchScope: GitBranchScope;
  branches: GitBranch[];
  branchesTotal: number;
  branchesHasMore: boolean;
  branchesOffset: number;
  branchSearch: string;
  commits: GitCommit[];
  commitsOffset: number;
  commitsHasMore: boolean;
  commitsTotal: number;
  stashes: GitStash[];
  worktrees: GitWorktree[];
  remotes: GitRemote[];
  remotesRepoPath: string | null;
  remotesLoading: boolean;
  remotesError?: string;
  mainRepoPath: string | null;
  selectedCommitHash?: string;
  commitDiff?: GitDiffPreview;
  setActiveRepoPath: (repoPath: string | null) => void;
  refresh: (repoPath: string, options?: { force?: boolean }) => Promise<void>;
  invalidateRepoCache: (repoPath: string) => void;
  setActiveView: (view: GitPanelView) => void;
  setBranchScope: (scope: GitBranchScope) => void;
  selectFile: (repoPath: string, filePath: string, staged?: boolean) => Promise<void>;
  stage: (repoPath: string, filePath: string) => Promise<void>;
  stageMany: (repoPath: string, files: string[]) => Promise<void>;
  unstage: (repoPath: string, filePath: string) => Promise<void>;
  unstageMany: (repoPath: string, files: string[]) => Promise<void>;
  discardFiles: (repoPath: string, files: string[]) => Promise<void>;
  commit: (repoPath: string, message: string) => Promise<string>;
  softResetLastCommit: (repoPath: string) => Promise<void>;
  fetchRemote: (repoPath: string) => Promise<void>;
  pullRemote: (repoPath: string) => Promise<void>;
  pushRemote: (repoPath: string) => Promise<void>;
  loadBranches: (repoPath: string, scope?: GitBranchScope, search?: string) => Promise<void>;
  loadMoreBranches: (repoPath: string) => Promise<void>;
  setBranchSearch: (repoPath: string, query: string) => Promise<void>;
  checkoutBranch: (repoPath: string, branchName: string, isRemote: boolean) => Promise<void>;
  createBranch: (repoPath: string, branchName: string, fromRef?: string | null) => Promise<void>;
  renameBranch: (repoPath: string, oldName: string, newName: string) => Promise<void>;
  deleteBranch: (repoPath: string, branchName: string, force: boolean) => Promise<void>;
  loadCommits: (repoPath: string, append?: boolean) => Promise<void>;
  loadMoreCommits: (repoPath: string) => Promise<void>;
  setMainRepoPath: (path: string | null) => void;
  loadWorktrees: (repoPath: string) => Promise<void>;
  addWorktree: (repoPath: string, worktreePath: string, branchName: string, baseRef?: string | null) => Promise<GitWorktree>;
  removeWorktree: (repoPath: string, worktreePath: string, force: boolean, branchName?: string | null, deleteBranch?: boolean) => Promise<void>;
  pruneWorktrees: (repoPath: string) => Promise<void>;
  loadStashes: (repoPath: string) => Promise<void>;
  pushStash: (repoPath: string, message?: string) => Promise<void>;
  applyStash: (repoPath: string, stashIndex: number) => Promise<void>;
  popStash: (repoPath: string, stashIndex: number) => Promise<void>;
  selectCommit: (repoPath: string, commitHash: string) => Promise<void>;
  clearCommitSelection: () => void;
  loadRemotes: (repoPath: string) => Promise<void>;
  addRemote: (repoPath: string, name: string, url: string) => Promise<void>;
  removeRemote: (repoPath: string, name: string) => Promise<void>;
  renameRemote: (repoPath: string, oldName: string, newName: string) => Promise<void>;
  getStatusForRepo: (repoPath: string) => Promise<GitStatus>;
  clearError: () => void;
  drafts: GitDraftsPayload;
  loadDraftsForWorkspace: (workspaceId: string) => void;
  setCommitMessageDraft: (workspaceId: string, message: string) => void;
  setBranchNameDraft: (workspaceId: string, name: string) => void;
  pushCommitHistory: (workspaceId: string, message: string) => void;
  pushBranchHistory: (workspaceId: string, name: string) => void;
  flushDrafts: (workspaceId: string) => void;
}

export const useGitStore = create<GitState>((set, get) => {
  let loadingOps = 0;
  let refreshSeq = 0;
  let selectFileSeq = 0;
  let branchesSeq = 0;
  let commitsSeq = 0;
  let stashesSeq = 0;
  let worktreesSeq = 0;
  let commitDiffSeq = 0;
  let remotesSeq = 0;

  const isRepoActive = (repoPath: string): boolean => {
    const activeRepoPath = get().activeRepoPath;
    return activeRepoPath === null || activeRepoPath === repoPath;
  };

  const isRepoInWorktreeContext = (repoPath: string): boolean => {
    const { activeRepoPath, mainRepoPath } = get();
    if (activeRepoPath === null) {
      return true;
    }
    return activeRepoPath === repoPath || mainRepoPath === repoPath;
  };

  const resolveRefreshRepoPathForWorktreeMutation = (repoPath: string): string => {
    const { activeRepoPath, mainRepoPath } = get();
    if (mainRepoPath && mainRepoPath === repoPath && activeRepoPath) {
      return activeRepoPath;
    }
    return repoPath;
  };

  const beginLoading = () => {
    loadingOps += 1;
    if (loadingOps === 1) {
      set({ loading: true });
    }
  };

  const endLoading = () => {
    loadingOps = Math.max(0, loadingOps - 1);
    if (loadingOps === 0) {
      set({ loading: false });
    }
  };

  const runRefresh = async (repoPath: string, options?: { force?: boolean }) => {
    const requestSeq = ++refreshSeq;
    const startedAt = getGitGateway().performanceNow();

    try {
      const status = await getGitGateway().getCachedGitStatus(repoPath, options?.force ?? false);
      const currentState = get();
      const selectedFile = currentState.selectedFile;
      const selectedFileStaged = currentState.selectedFileStaged ?? false;
      let selectedDiff: GitDiffPreview | undefined = currentState.diff;
      let nextSelectedFile = selectedFile;
      let nextSelectedFileStaged = currentState.selectedFileStaged;
      const shouldRefreshSelectedDiff = currentState.activeView === "changes";
      let selectedDiffRefreshed = false;

      if (selectedFile) {
        const selectedStatus = status.files.find((file) => file.path === selectedFile);
        const sameStateExists = selectedStatus
          ? (selectedFileStaged ? Boolean(selectedStatus.indexStatus) : Boolean(selectedStatus.worktreeStatus))
          : false;
        const oppositeStateExists = selectedStatus
          ? (selectedFileStaged ? Boolean(selectedStatus.worktreeStatus) : Boolean(selectedStatus.indexStatus))
          : false;

        if (!sameStateExists && !oppositeStateExists) {
          selectedDiff = undefined;
          nextSelectedFile = undefined;
          nextSelectedFileStaged = undefined;
        } else if (shouldRefreshSelectedDiff) {
          if (sameStateExists) {
            try {
              if (options?.force) {
                selectFileSeq += 1;
              }
              selectedDiff = await getGitGateway().getCachedFileDiff(
                repoPath,
                selectedFile,
                selectedFileStaged,
                options?.force ?? false,
              );
              selectedDiffRefreshed = true;
            } catch {
              selectedDiff = undefined;
            }
          } else {
            const flippedStaged = !selectedFileStaged;
            nextSelectedFileStaged = flippedStaged;
            try {
              if (options?.force) {
                selectFileSeq += 1;
              }
              selectedDiff = await getGitGateway().getCachedFileDiff(
                repoPath,
                selectedFile,
                flippedStaged,
                options?.force ?? false,
              );
              selectedDiffRefreshed = true;
            } catch {
              selectedDiff = undefined;
            }
          }
        } else {
          if (!sameStateExists && oppositeStateExists) {
            nextSelectedFileStaged = !selectedFileStaged;
          }
          selectedDiff = undefined;
        }
      }

      const forceRefresh = options?.force ?? false;
      const activeViewState = {
        activeView: currentState.activeView,
        branchScope: currentState.branchScope,
        branchSearch: currentState.branchSearch,
      };
      const refreshView = getGitGateway().shouldRefreshActiveView(
        repoPath,
        activeViewState,
        forceRefresh,
      );
      const viewState = refreshView
        ? await refreshGitActiveView(repoPath, activeViewState)
        : {};

      if (requestSeq === refreshSeq && isRepoActive(repoPath)) {
        set({
          ...viewState,
          status,
          selectedFile: nextSelectedFile,
          selectedFileStaged: nextSelectedFileStaged,
          diff: selectedDiff,
          error: undefined,
        });
        if (refreshView) {
          getGitGateway().markActiveViewRefreshed(repoPath, activeViewState);
        }
      }

      getGitGateway().recordMetric("git.refresh.ms", getGitGateway().performanceNow() - startedAt, {
        repoPath,
        fileCount: status.files.length,
        cached: !forceRefresh,
        viewRefreshed: refreshView,
        selectedDiffRefreshed,
      });
    } catch (error) {
      if (requestSeq === refreshSeq && isRepoActive(repoPath)) {
        set({ error: String(error) });
      }
      getGitGateway().recordMetric("git.refresh.ms", getGitGateway().performanceNow() - startedAt, {
        repoPath,
        failed: true,
      });
    }
  };

  const runRepoMutationWithRefresh = async <T>(
    repoPath: string,
    mutation: () => Promise<T>,
    options?: { remoteSyncAction?: GitRemoteSyncAction },
  ): Promise<T> => {
    beginLoading();
    set({ error: undefined });

    if (options?.remoteSyncAction) {
      set({ remoteSyncAction: options.remoteSyncAction, remoteSyncRepoPath: repoPath });
    }

    try {
      const result = await mutation();
      get().invalidateRepoCache(repoPath);
      await runRefresh(repoPath, { force: true });
      return result;
    } catch (error) {
      if (isRepoActive(repoPath)) {
        set({ error: String(error) });
      }
      throw error;
    } finally {
      if (
        options?.remoteSyncAction &&
        get().remoteSyncAction === options.remoteSyncAction &&
        get().remoteSyncRepoPath === repoPath
      ) {
        set({ remoteSyncAction: null, remoteSyncRepoPath: null });
      }
      endLoading();
    }
  };

  return {
    loading: false,
    activeRepoPath: null,
    remoteSyncAction: null,
    remoteSyncRepoPath: null,
    activeView: "changes",
    branchScope: "local",
    branches: [],
    branchesTotal: 0,
    branchesHasMore: false,
    branchesOffset: 0,
    branchSearch: "",
    commits: [],
    commitsOffset: 0,
    commitsHasMore: false,
    commitsTotal: 0,
    stashes: [],
    worktrees: [],
    remotes: [],
    remotesRepoPath: null,
    remotesLoading: false,
    remotesError: undefined,
    mainRepoPath: null,
    setActiveRepoPath: (repoPath) => {
      if (get().activeRepoPath === repoPath) {
        return;
      }

      set({
        activeRepoPath: repoPath,
        mainRepoPath: null,
        status: undefined,
        selectedFile: undefined,
        selectedFileStaged: undefined,
        diff: undefined,
        branches: [],
        branchesTotal: 0,
        branchesHasMore: false,
        branchesOffset: 0,
        branchSearch: "",
        commits: [],
        commitsOffset: 0,
        commitsHasMore: false,
        commitsTotal: 0,
        stashes: [],
        worktrees: [],
        remotes: [],
        remotesRepoPath: null,
        remotesLoading: false,
        remotesError: undefined,
        selectedCommitHash: undefined,
        commitDiff: undefined,
        error: undefined,
      });
    },
    refresh: async (repoPath, options) => {
      beginLoading();
      await runRefresh(repoPath, options);
      endLoading();
    },
    invalidateRepoCache: (repoPath) => {
      invalidateRepoCaches(repoPath);
    },
    setActiveView: (view) => {
      set({ activeView: view, error: undefined });
    },
    setBranchScope: (scope) => {
      set({ branchScope: scope, error: undefined });
    },
    selectFile: async (repoPath, filePath, staged = false) => {
      const requestSeq = ++selectFileSeq;
      const startedAt = getGitGateway().performanceNow();
      try {
        const diff = await getGitGateway().getCachedFileDiff(
          repoPath,
          filePath,
          staged,
        );
        if (requestSeq === selectFileSeq && isRepoActive(repoPath)) {
          set({ selectedFile: filePath, selectedFileStaged: staged, diff, error: undefined });
        }
        getGitGateway().recordMetric("git.file_diff.ms", getGitGateway().performanceNow() - startedAt, {
          repoPath,
          filePath,
          staged,
          truncated: diff.truncated,
          returnedBytes: diff.returnedBytes,
          originalBytes: diff.originalBytes,
        });
      } catch (error) {
        if (requestSeq === selectFileSeq && isRepoActive(repoPath)) {
          set({ error: String(error) });
        }
        getGitGateway().recordMetric("git.file_diff.ms", getGitGateway().performanceNow() - startedAt, {
          repoPath,
          filePath,
          staged,
          failed: true,
        });
      }
    },
    stage: async (repoPath, filePath) => {
      await runRepoMutationWithRefresh(repoPath, () => getGitGateway().stageFiles(repoPath, [filePath]));
    },
    stageMany: async (repoPath, files) => {
      if (files.length === 0) {
        return;
      }
      await runRepoMutationWithRefresh(repoPath, () => getGitGateway().stageFiles(repoPath, files));
    },
    unstage: async (repoPath, filePath) => {
      await runRepoMutationWithRefresh(repoPath, () => getGitGateway().unstageFiles(repoPath, [filePath]));
    },
    unstageMany: async (repoPath, files) => {
      if (files.length === 0) {
        return;
      }
      await runRepoMutationWithRefresh(repoPath, () => getGitGateway().unstageFiles(repoPath, files));
    },
    discardFiles: async (repoPath, files) => {
      await runRepoMutationWithRefresh(repoPath, () => getGitGateway().discardFiles(repoPath, files));
    },
    commit: async (repoPath, message) => {
      return runRepoMutationWithRefresh(repoPath, () => getGitGateway().commit(repoPath, message));
    },
    softResetLastCommit: async (repoPath) => {
      await runRepoMutationWithRefresh(repoPath, () => getGitGateway().softResetLastCommit(repoPath));
    },
    fetchRemote: async (repoPath) => {
      await runRepoMutationWithRefresh(repoPath, () => getGitGateway().fetchGit(repoPath), {
        remoteSyncAction: "fetch",
      });
    },
    pullRemote: async (repoPath) => {
      await runRepoMutationWithRefresh(repoPath, () => getGitGateway().pullGit(repoPath), {
        remoteSyncAction: "pull",
      });
    },
    pushRemote: async (repoPath) => {
      await runRepoMutationWithRefresh(repoPath, () => getGitGateway().pushGit(repoPath), {
        remoteSyncAction: "push",
      });
    },
    loadBranches: async (repoPath, scope, search) => {
      const requestSeq = ++branchesSeq;
      const nextScope = scope ?? get().branchScope;
      const searchQuery = normalizeGitBranchSearch(
        search !== undefined ? search : get().branchSearch,
      );
      beginLoading();
      set({ error: undefined, branchScope: nextScope, branchSearch: searchQuery });

      try {
        const page = normalizeGitPage(
          await getGitGateway().listGitBranches(
            repoPath,
            nextScope,
            0,
            BRANCH_PAGE_SIZE,
            gitBranchSearchParam(searchQuery),
          ),
          0,
          BRANCH_PAGE_SIZE,
        );
        if (requestSeq === branchesSeq && isRepoActive(repoPath)) {
          set({
            branches: page.entries,
            branchesTotal: page.total,
            branchesHasMore: page.hasMore,
            branchesOffset: page.offset + page.entries.length,
          });
        }
      } catch (error) {
        if (requestSeq === branchesSeq && isRepoActive(repoPath)) {
          set({ error: String(error) });
        }
      } finally {
        endLoading();
      }
    },
    loadMoreBranches: async (repoPath) => {
      if (!get().branchesHasMore) return;
      const requestSeq = ++branchesSeq;
      const { branchScope, branchSearch, branchesOffset, branches } = get();

      beginLoading();
      set({ error: undefined });

      try {
        const page = normalizeGitPage(
          await getGitGateway().listGitBranches(
            repoPath,
            branchScope,
            branchesOffset,
            BRANCH_PAGE_SIZE,
            gitBranchSearchParam(branchSearch),
          ),
          branchesOffset,
          BRANCH_PAGE_SIZE,
        );
        if (requestSeq === branchesSeq && isRepoActive(repoPath)) {
          set({
            branches: [...branches, ...page.entries],
            branchesTotal: page.total,
            branchesHasMore: page.hasMore,
            branchesOffset: page.offset + page.entries.length,
          });
        }
      } catch (error) {
        if (requestSeq === branchesSeq && isRepoActive(repoPath)) {
          set({ error: String(error) });
        }
      } finally {
        endLoading();
      }
    },
    setBranchSearch: async (repoPath, query) => {
      await get().loadBranches(repoPath, undefined, query);
    },
    checkoutBranch: async (repoPath, branchName, isRemote) => {
      await runRepoMutationWithRefresh(repoPath, () =>
        getGitGateway().checkoutGitBranch(repoPath, branchName, isRemote),
      );
    },
    createBranch: async (repoPath, branchName, fromRef) => {
      await runRepoMutationWithRefresh(repoPath, () =>
        getGitGateway().createGitBranch(repoPath, branchName, fromRef ?? null),
      );
    },
    renameBranch: async (repoPath, oldName, newName) => {
      await runRepoMutationWithRefresh(repoPath, () =>
        getGitGateway().renameGitBranch(repoPath, oldName, newName),
      );
    },
    deleteBranch: async (repoPath, branchName, force) => {
      await runRepoMutationWithRefresh(repoPath, () =>
        getGitGateway().deleteGitBranch(repoPath, branchName, force),
      );
    },
    loadCommits: async (repoPath, append = false) => {
      const requestSeq = ++commitsSeq;
      const offset = append ? get().commitsOffset : 0;
      const previousEntries = append ? get().commits : [];

      beginLoading();
      set({ error: undefined });

      try {
        const page = normalizeGitPage(
          await getGitGateway().listGitCommits(repoPath, offset, COMMIT_PAGE_SIZE),
          offset,
          COMMIT_PAGE_SIZE,
        );
        if (requestSeq !== commitsSeq || !isRepoActive(repoPath)) {
          return;
        }

        const entries = append ? [...previousEntries, ...page.entries] : page.entries;
        set({
          commits: entries,
          commitsOffset: page.offset + page.entries.length,
          commitsHasMore: page.hasMore,
          commitsTotal: page.total,
        });
      } catch (error) {
        if (requestSeq === commitsSeq && isRepoActive(repoPath)) {
          set({ error: String(error) });
        }
      } finally {
        endLoading();
      }
    },
    loadMoreCommits: async (repoPath) => {
      if (!get().commitsHasMore) {
        return;
      }
      await get().loadCommits(repoPath, true);
    },
    setMainRepoPath: (path) => {
      set({ mainRepoPath: path });
    },
    loadWorktrees: async (repoPath) => {
      const requestSeq = ++worktreesSeq;
      beginLoading();
      set({ error: undefined });
      try {
        const worktrees = await getGitGateway().listGitWorktrees(repoPath);
        if (requestSeq === worktreesSeq && isRepoInWorktreeContext(repoPath)) {
          set({ worktrees });
        }
      } catch (error) {
        if (requestSeq === worktreesSeq && isRepoInWorktreeContext(repoPath)) {
          set({ error: String(error) });
        }
      } finally {
        endLoading();
      }
    },
    addWorktree: async (repoPath, worktreePath, branchName, baseRef) => {
      const refreshRepoPath = resolveRefreshRepoPathForWorktreeMutation(repoPath);
      return runRepoMutationWithRefresh(refreshRepoPath, () =>
        getGitGateway().addGitWorktree(repoPath, worktreePath, branchName, baseRef),
      );
    },
    removeWorktree: async (repoPath, worktreePath, force, branchName, deleteBranch) => {
      const { activeRepoPath, mainRepoPath } = get();
      const removingActiveWorktree =
        activeRepoPath !== null &&
        activeRepoPath === worktreePath &&
        mainRepoPath === repoPath;

      if (removingActiveWorktree) {
        beginLoading();
        set({ error: undefined });
        try {
          await getGitGateway().removeGitWorktree(repoPath, worktreePath, force, branchName, deleteBranch);
          get().setActiveRepoPath(repoPath);
          set({ mainRepoPath: null });
          get().invalidateRepoCache(repoPath);
          await runRefresh(repoPath, { force: true });
        } catch (error) {
          if (isRepoInWorktreeContext(repoPath)) {
            set({ error: String(error) });
          }
          throw error;
        } finally {
          endLoading();
        }
        return;
      }

      const refreshRepoPath = resolveRefreshRepoPathForWorktreeMutation(repoPath);
      await runRepoMutationWithRefresh(refreshRepoPath, () =>
        getGitGateway().removeGitWorktree(repoPath, worktreePath, force, branchName, deleteBranch),
      );
    },
    pruneWorktrees: async (repoPath) => {
      const refreshRepoPath = resolveRefreshRepoPathForWorktreeMutation(repoPath);
      await runRepoMutationWithRefresh(refreshRepoPath, () => getGitGateway().pruneGitWorktrees(repoPath));
    },
    loadStashes: async (repoPath) => {
      const requestSeq = ++stashesSeq;
      beginLoading();
      set({ error: undefined });
      try {
        const stashes = await getGitGateway().listGitStashes(repoPath);
        if (requestSeq === stashesSeq && isRepoActive(repoPath)) {
          set({ stashes });
        }
      } catch (error) {
        if (requestSeq === stashesSeq && isRepoActive(repoPath)) {
          set({ error: String(error) });
        }
      } finally {
        endLoading();
      }
    },
    pushStash: async (repoPath, message) => {
      await runRepoMutationWithRefresh(repoPath, () => getGitGateway().pushGitStash(repoPath, message));
    },
    applyStash: async (repoPath, stashIndex) => {
      await runRepoMutationWithRefresh(repoPath, () => getGitGateway().applyGitStash(repoPath, stashIndex));
    },
    popStash: async (repoPath, stashIndex) => {
      await runRepoMutationWithRefresh(repoPath, () => getGitGateway().popGitStash(repoPath, stashIndex));
    },
    selectCommit: async (repoPath, commitHash) => {
      const current = get().selectedCommitHash;
      if (current === commitHash) {
        set({ selectedCommitHash: undefined, commitDiff: undefined });
        return;
      }

      const requestSeq = ++commitDiffSeq;
      const startedAt = getGitGateway().performanceNow();
      set({ selectedCommitHash: commitHash, commitDiff: undefined });
      try {
        const diff = await getGitGateway().getCommitDiff(repoPath, commitHash);
        if (
          requestSeq === commitDiffSeq &&
          isRepoActive(repoPath) &&
          get().selectedCommitHash === commitHash
        ) {
          set({ commitDiff: diff });
        }
        getGitGateway().recordMetric("git.file_diff.ms", getGitGateway().performanceNow() - startedAt, {
          repoPath,
          commitHash,
          truncated: diff.truncated,
          returnedBytes: diff.returnedBytes,
          originalBytes: diff.originalBytes,
        });
      } catch (error) {
        if (
          requestSeq === commitDiffSeq &&
          isRepoActive(repoPath) &&
          get().selectedCommitHash === commitHash
        ) {
          set({ error: String(error), selectedCommitHash: undefined, commitDiff: undefined });
        }
        getGitGateway().recordMetric("git.file_diff.ms", getGitGateway().performanceNow() - startedAt, {
          repoPath,
          commitHash,
          failed: true,
        });
      }
    },
    clearCommitSelection: () => {
      set({ selectedCommitHash: undefined, commitDiff: undefined });
    },
    loadRemotes: async (repoPath) => {
      const requestSeq = ++remotesSeq;
      const { remotes, remotesRepoPath } = get();
      const shouldClearRemotes = remotesRepoPath !== repoPath;

      set({
        remotes: shouldClearRemotes ? [] : remotes,
        remotesRepoPath: repoPath,
        remotesLoading: true,
        remotesError: undefined,
        error: undefined,
      });
      try {
        const remotes = await getGitGateway().listGitRemotes(repoPath);
        if (requestSeq === remotesSeq && isRepoActive(repoPath)) {
          set({ remotes, remotesRepoPath: repoPath, remotesError: undefined });
        }
      } catch (error) {
        if (requestSeq === remotesSeq && isRepoActive(repoPath)) {
          set({ error: String(error), remotesError: String(error) });
        }
      } finally {
        if (requestSeq === remotesSeq) {
          set({ remotesLoading: false });
        }
      }
    },
    addRemote: async (repoPath, name, url) => {
      await runRepoMutationWithRefresh(repoPath, async () => {
        await getGitGateway().addGitRemote(repoPath, name, url);
      });
      await get().loadRemotes(repoPath);
      // Auto-fetch from the new remote and refresh cached git state so new refs
      // appear immediately. Swallow network/empty-remote failures.
      try {
        await getGitGateway().fetchGit(repoPath);
        get().invalidateRepoCache(repoPath);
        beginLoading();
        try {
          await runRefresh(repoPath, { force: true });
        } finally {
          endLoading();
        }
      } catch {
        // Swallow: remote may be unreachable or empty
      }
    },
    removeRemote: async (repoPath, name) => {
      await runRepoMutationWithRefresh(repoPath, async () => {
        await getGitGateway().removeGitRemote(repoPath, name);
      });
      await get().loadRemotes(repoPath);
    },
    renameRemote: async (repoPath, oldName, newName) => {
      await runRepoMutationWithRefresh(repoPath, async () => {
        await getGitGateway().renameGitRemote(repoPath, oldName, newName);
      });
      await get().loadRemotes(repoPath);
    },
    getStatusForRepo: (repoPath) =>
      getGitGateway().getCachedGitStatus(repoPath),
    clearError: () => set({ error: undefined }),
    drafts: { ...EMPTY_GIT_DRAFTS },
    loadDraftsForWorkspace: (workspaceId) => {
      set({ drafts: getGitGateway().readStoredGitDrafts(workspaceId) });
    },
    setCommitMessageDraft: (_workspaceId, message) => {
      set((state) => ({ drafts: { ...state.drafts, commitMessage: message } }));
    },
    setBranchNameDraft: (_workspaceId, name) => {
      set((state) => ({ drafts: { ...state.drafts, branchName: name } }));
    },
    pushCommitHistory: (workspaceId, message) => {
      const drafts = get().drafts;
      const next: GitDraftsPayload = {
        ...drafts,
        commitMessage: "",
        commitHistory: addToGitDraftHistory(drafts.commitHistory, message),
      };
      set({ drafts: next });
      getGitGateway().writeStoredGitDrafts(workspaceId, next);
    },
    pushBranchHistory: (workspaceId, name) => {
      const drafts = get().drafts;
      const next: GitDraftsPayload = {
        ...drafts,
        branchName: "",
        branchHistory: addToGitDraftHistory(drafts.branchHistory, name),
      };
      set({ drafts: next });
      getGitGateway().writeStoredGitDrafts(workspaceId, next);
    },
    flushDrafts: (workspaceId) => {
      getGitGateway().writeStoredGitDrafts(workspaceId, get().drafts);
    },
  };
});

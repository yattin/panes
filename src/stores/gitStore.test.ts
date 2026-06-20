import { beforeEach, describe, expect, it, vi } from "vitest";
import type { GitBranch, GitDiffPreview, GitStatus, GitWorktree } from "../types";

const mockIpc = vi.hoisted(() => ({
  getGitStatus: vi.fn(),
  getFileDiff: vi.fn(),
  stageFiles: vi.fn(),
  unstageFiles: vi.fn(),
  discardFiles: vi.fn(),
  commit: vi.fn(),
  softResetLastCommit: vi.fn(),
  fetchGit: vi.fn(),
  pullGit: vi.fn(),
  pushGit: vi.fn(),
  listGitBranches: vi.fn(),
  checkoutGitBranch: vi.fn(),
  createGitBranch: vi.fn(),
  renameGitBranch: vi.fn(),
  deleteGitBranch: vi.fn(),
  listGitCommits: vi.fn(),
  listGitStashes: vi.fn(),
  pushGitStash: vi.fn(),
  applyGitStash: vi.fn(),
  popGitStash: vi.fn(),
  addGitWorktree: vi.fn(),
  listGitWorktrees: vi.fn(),
  removeGitWorktree: vi.fn(),
  pruneGitWorktrees: vi.fn(),
  getCommitDiff: vi.fn(),
}));

vi.mock("../lib/ipc", () => ({
  ipc: mockIpc,
}));

import { configureGitGateway } from "../contexts/git/application/gitGateway";
import { gitGateway } from "../contexts/git/infrastructure/gitRepository";
import { useGitStore } from "./gitStore";

function makeStatus(branch: string, files: GitStatus["files"] = []): GitStatus {
  return {
    branch,
    files,
    ahead: 0,
    behind: 0,
  };
}

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function makeDiffPreview(content = ""): GitDiffPreview {
  return {
    content,
    truncated: false,
    originalBytes: content.length,
    returnedBytes: content.length,
  };
}

async function flushPromises() {
  await new Promise((resolve) => setTimeout(resolve, 0));
}

describe("gitStore", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.unstubAllGlobals();
    configureGitGateway(gitGateway);

    mockIpc.getGitStatus.mockResolvedValue(makeStatus("main"));
    mockIpc.getFileDiff.mockResolvedValue(makeDiffPreview());
    mockIpc.stageFiles.mockResolvedValue(undefined);
    mockIpc.unstageFiles.mockResolvedValue(undefined);
    mockIpc.discardFiles.mockResolvedValue(undefined);
    mockIpc.commit.mockResolvedValue("abc123");
    mockIpc.softResetLastCommit.mockResolvedValue(undefined);
    mockIpc.fetchGit.mockResolvedValue(undefined);
    mockIpc.pullGit.mockResolvedValue(undefined);
    mockIpc.pushGit.mockResolvedValue(undefined);
    mockIpc.listGitBranches.mockResolvedValue({ entries: [] });
    mockIpc.checkoutGitBranch.mockResolvedValue(undefined);
    mockIpc.createGitBranch.mockResolvedValue(undefined);
    mockIpc.renameGitBranch.mockResolvedValue(undefined);
    mockIpc.deleteGitBranch.mockResolvedValue(undefined);
    mockIpc.listGitCommits.mockResolvedValue({
      entries: [],
      offset: 0,
      limit: 100,
      total: 0,
      hasMore: false,
    });
    mockIpc.listGitStashes.mockResolvedValue([]);
    mockIpc.pushGitStash.mockResolvedValue(undefined);
    mockIpc.applyGitStash.mockResolvedValue(undefined);
    mockIpc.popGitStash.mockResolvedValue(undefined);
    mockIpc.addGitWorktree.mockResolvedValue({
      path: "/repo/.panes/worktrees/feature",
      headSha: null,
      branch: "feature",
      isMain: false,
      isLocked: false,
      isPrunable: false,
    } satisfies GitWorktree);
    mockIpc.listGitWorktrees.mockResolvedValue([]);
    mockIpc.removeGitWorktree.mockResolvedValue(undefined);
    mockIpc.pruneGitWorktrees.mockResolvedValue(undefined);
    mockIpc.getCommitDiff.mockResolvedValue(makeDiffPreview());

    useGitStore.setState({
      status: undefined,
      selectedFile: undefined,
      selectedFileStaged: undefined,
      diff: undefined,
      loading: false,
      error: undefined,
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
      mainRepoPath: null,
      selectedCommitHash: undefined,
      commitDiff: undefined,
      drafts: {
        commitMessage: "",
        branchName: "",
        commitHistory: [],
        branchHistory: [],
      },
    });
  });

  it("keeps loading true until all overlapping operations settle", async () => {
    const fetchDeferred = deferred<void>();
    const pullDeferred = deferred<void>();
    mockIpc.fetchGit.mockReturnValueOnce(fetchDeferred.promise);
    mockIpc.pullGit.mockReturnValueOnce(pullDeferred.promise);

    const fetchPromise = useGitStore.getState().fetchRemote("/repo");
    const pullPromise = useGitStore.getState().pullRemote("/repo");
    await flushPromises();
    expect(useGitStore.getState().loading).toBe(true);

    fetchDeferred.resolve(undefined);
    await flushPromises();
    expect(useGitStore.getState().loading).toBe(true);

    pullDeferred.resolve(undefined);
    await Promise.all([fetchPromise, pullPromise]);
    expect(useGitStore.getState().loading).toBe(false);
  });

  it("tracks remote sync state only for remote operations", async () => {
    const pushDeferred = deferred<void>();
    mockIpc.pushGit.mockReturnValueOnce(pushDeferred.promise);

    const pushPromise = useGitStore.getState().pushRemote("/repo");
    await flushPromises();
    expect(useGitStore.getState().remoteSyncAction).toBe("push");
    expect(useGitStore.getState().remoteSyncRepoPath).toBe("/repo");

    pushDeferred.resolve(undefined);
    await pushPromise;
    expect(useGitStore.getState().remoteSyncAction).toBeNull();
    expect(useGitStore.getState().remoteSyncRepoPath).toBeNull();
  });

  it("ignores stale refresh responses after repo switch", async () => {
    const repoAStatus = deferred<GitStatus>();
    mockIpc.getGitStatus.mockImplementation((repoPath: string) => {
      if (repoPath === "/repo-a") {
        return repoAStatus.promise;
      }
      return Promise.resolve(makeStatus("repo-b-branch"));
    });

    useGitStore.getState().setActiveRepoPath("/repo-a");
    const repoARefresh = useGitStore.getState().refresh("/repo-a");
    await flushPromises();

    useGitStore.getState().setActiveRepoPath("/repo-b");
    await useGitStore.getState().refresh("/repo-b");
    expect(useGitStore.getState().status?.branch).toBe("repo-b-branch");

    repoAStatus.resolve(makeStatus("repo-a-branch"));
    await repoARefresh;
    expect(useGitStore.getState().status?.branch).toBe("repo-b-branch");
  });

  it("refreshes status after bulk stage mutation", async () => {
    const repoPath = "/repo-stage";
    mockIpc.getGitStatus
      .mockResolvedValueOnce(makeStatus("main", []))
      .mockResolvedValueOnce(makeStatus("main", [{ path: "a.ts", indexStatus: "added" }]));

    useGitStore.getState().setActiveRepoPath(repoPath);
    await useGitStore.getState().refresh(repoPath);
    expect(useGitStore.getState().status?.files).toHaveLength(0);

    await useGitStore.getState().stageMany(repoPath, ["a.ts"]);
    expect(mockIpc.stageFiles).toHaveBeenCalledWith(repoPath, ["a.ts"]);
    expect(useGitStore.getState().status?.files).toHaveLength(1);
    expect(useGitStore.getState().status?.files[0]?.path).toBe("a.ts");
  });

  it("starts a fresh status request for forced refresh while another request is in flight", async () => {
    const repoPath = "/repo-force-refresh";
    const firstStatus = deferred<GitStatus>();
    mockIpc.getGitStatus
      .mockReturnValueOnce(firstStatus.promise)
      .mockResolvedValueOnce(makeStatus("forced-branch"));

    const firstRefresh = useGitStore.getState().refresh(repoPath);
    await flushPromises();
    expect(mockIpc.getGitStatus).toHaveBeenCalledTimes(1);

    const forcedRefresh = useGitStore.getState().refresh(repoPath, { force: true });
    await flushPromises();

    expect(mockIpc.getGitStatus).toHaveBeenCalledTimes(2);
    await forcedRefresh;
    expect(useGitStore.getState().status?.branch).toBe("forced-branch");

    firstStatus.resolve(makeStatus("stale-branch"));
    await firstRefresh;
    expect(useGitStore.getState().status?.branch).toBe("forced-branch");
  });

  it("refreshes the selected file diff during a forced changes refresh", async () => {
    const repoPath = "/repo-force-diff";
    const filePath = "src/app.ts";
    mockIpc.getFileDiff
      .mockResolvedValueOnce(makeDiffPreview("old diff"))
      .mockResolvedValueOnce(makeDiffPreview("new diff"));
    mockIpc.getGitStatus.mockResolvedValue(
      makeStatus("main", [{ path: filePath, worktreeStatus: "modified" }]),
    );

    await useGitStore.getState().selectFile(repoPath, filePath, false);
    expect(useGitStore.getState().diff?.content).toBe("old diff");

    await useGitStore.getState().refresh(repoPath, { force: true });

    expect(mockIpc.getFileDiff).toHaveBeenCalledTimes(2);
    expect(mockIpc.getFileDiff).toHaveBeenLastCalledWith(repoPath, filePath, false);
    expect(useGitStore.getState().diff?.content).toBe("new diff");
  });

  it("keeps the forced selected file diff when an earlier diff response resolves later", async () => {
    const repoPath = "/repo-stale-diff";
    const filePath = "src/app.ts";
    const staleDiff = deferred<GitDiffPreview>();
    mockIpc.getFileDiff
      .mockReturnValueOnce(staleDiff.promise)
      .mockResolvedValueOnce(makeDiffPreview("forced diff"));
    mockIpc.getGitStatus.mockResolvedValue(
      makeStatus("main", [{ path: filePath, worktreeStatus: "modified" }]),
    );
    useGitStore.setState({
      activeView: "changes",
      selectedFile: filePath,
      selectedFileStaged: false,
      diff: makeDiffPreview("initial diff"),
    });

    const staleSelect = useGitStore.getState().selectFile(repoPath, filePath, false);
    await flushPromises();
    expect(mockIpc.getFileDiff).toHaveBeenCalledTimes(1);

    await useGitStore.getState().refresh(repoPath, { force: true });
    expect(useGitStore.getState().diff?.content).toBe("forced diff");

    staleDiff.resolve(makeDiffPreview("stale diff"));
    await staleSelect;

    expect(useGitStore.getState().diff?.content).toBe("forced diff");
  });

  it("falls back to the main repo after removing the active worktree", async () => {
    const mainRepoPath = "/repo-main";
    const worktreePath = "/repo-main/.panes/worktrees/feature";
    const remainingWorktrees: GitWorktree[] = [
      {
        path: mainRepoPath,
        headSha: null,
        branch: "main",
        isMain: true,
        isLocked: false,
        isPrunable: false,
      },
    ];

    mockIpc.listGitWorktrees.mockResolvedValue(remainingWorktrees);
    mockIpc.getGitStatus.mockResolvedValue(makeStatus("main"));

    useGitStore.setState({
      activeRepoPath: worktreePath,
      mainRepoPath,
      activeView: "worktrees",
    });

    await useGitStore
      .getState()
      .removeWorktree(mainRepoPath, worktreePath, false, "feature", false);

    expect(mockIpc.removeGitWorktree).toHaveBeenCalledWith(
      mainRepoPath,
      worktreePath,
      false,
      "feature",
      false,
    );
    expect(useGitStore.getState().activeRepoPath).toBe(mainRepoPath);
    expect(useGitStore.getState().mainRepoPath).toBeNull();
    expect(mockIpc.getGitStatus).toHaveBeenLastCalledWith(mainRepoPath);
  });

  it("normalizes stored git draft history before showing suggestions", () => {
    const storedDrafts = {
      commitMessage: "in progress",
      branchName: "feature/new-panel",
      commitHistory: ["  fix panel layout  ", "", "ship git pane", "fix panel layout", "older"],
      branchHistory: ["  feature/new-panel  ", "", "main", "feature/new-panel"],
    };
    const localStorageMock = {
      getItem: vi.fn(() => JSON.stringify(storedDrafts)),
      setItem: vi.fn(),
    };
    vi.stubGlobal("localStorage", localStorageMock);

    useGitStore.getState().loadDraftsForWorkspace("workspace-1");

    expect(localStorageMock.getItem).toHaveBeenCalledWith("panes:git.drafts:workspace-1");
    expect(useGitStore.getState().drafts).toEqual({
      commitMessage: "in progress",
      branchName: "feature/new-panel",
      commitHistory: ["fix panel layout", "ship git pane", "older"],
      branchHistory: ["feature/new-panel", "main"],
    });
  });

  it("loads branch pages with safe pagination defaults", async () => {
    const branch = {
      name: "feature/git-context",
      fullName: "feature/git-context",
      isCurrent: false,
      isRemote: false,
      ahead: 0,
      behind: 0,
    } satisfies GitBranch;
    mockIpc.listGitBranches.mockResolvedValueOnce({ entries: [branch] });

    await useGitStore.getState().loadBranches("/repo");

    expect(useGitStore.getState().branches).toEqual([branch]);
    expect(useGitStore.getState().branchesTotal).toBe(1);
    expect(useGitStore.getState().branchesOffset).toBe(1);
    expect(useGitStore.getState().branchesHasMore).toBe(false);
  });

  it("refreshes branches after the branch scope changes", async () => {
    const localBranch = {
      name: "main",
      fullName: "main",
      isCurrent: true,
      isRemote: false,
      ahead: 0,
      behind: 0,
    } satisfies GitBranch;
    const remoteBranch = {
      name: "origin/main",
      fullName: "refs/remotes/origin/main",
      isCurrent: false,
      isRemote: true,
      ahead: 0,
      behind: 0,
    } satisfies GitBranch;
    mockIpc.listGitBranches
      .mockResolvedValueOnce({
        entries: [localBranch],
        offset: 0,
        limit: 200,
        total: 1,
        hasMore: false,
      })
      .mockResolvedValueOnce({
        entries: [remoteBranch],
        offset: 0,
        limit: 200,
        total: 1,
        hasMore: false,
      });

    useGitStore.getState().setActiveView("branches");
    await useGitStore.getState().refresh("/repo");
    expect(useGitStore.getState().branches).toEqual([localBranch]);

    useGitStore.getState().setBranchScope("remote");
    await useGitStore.getState().refresh("/repo");

    expect(mockIpc.listGitBranches).toHaveBeenCalledTimes(2);
    expect(mockIpc.listGitBranches).toHaveBeenLastCalledWith(
      "/repo",
      "remote",
      0,
      200,
      undefined,
    );
    expect(useGitStore.getState().branches).toEqual([remoteBranch]);
  });

  it("treats whitespace-only branch search as an empty query", async () => {
    await useGitStore.getState().setBranchSearch("/repo", "   ");

    expect(mockIpc.listGitBranches).toHaveBeenCalledWith(
      "/repo",
      "local",
      0,
      200,
      undefined,
    );
    expect(useGitStore.getState().branchSearch).toBe("");
  });
});

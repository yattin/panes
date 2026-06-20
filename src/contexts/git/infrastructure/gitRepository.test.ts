import { beforeEach, describe, expect, it, vi } from "vitest";

const mockIpc = vi.hoisted(() => ({
  addGitRemote: vi.fn(),
  addGitWorktree: vi.fn(),
  applyGitStash: vi.fn(),
  checkoutGitBranch: vi.fn(),
  commit: vi.fn(),
  createGitBranch: vi.fn(),
  deleteGitBranch: vi.fn(),
  discardFiles: vi.fn(),
  fetchGit: vi.fn(),
  getCommitDiff: vi.fn(),
  getFileDiff: vi.fn(),
  getGitStatus: vi.fn(),
  initGitRepo: vi.fn(),
  listGitBranches: vi.fn(),
  listGitCommits: vi.fn(),
  listGitRemotes: vi.fn(),
  listGitStashes: vi.fn(),
  listGitWorktrees: vi.fn(),
  popGitStash: vi.fn(),
  pruneGitWorktrees: vi.fn(),
  pullGit: vi.fn(),
  pushGit: vi.fn(),
  pushGitStash: vi.fn(),
  removeGitRemote: vi.fn(),
  removeGitWorktree: vi.fn(),
  renameGitBranch: vi.fn(),
  renameGitRemote: vi.fn(),
  softResetLastCommit: vi.fn(),
  stageFiles: vi.fn(),
  unstageFiles: vi.fn(),
  watchGitRepo: vi.fn(),
}));

const mockListenGitRepoChanged = vi.hoisted(() => vi.fn());

vi.mock("../../../lib/ipc", () => ({
  ipc: mockIpc,
  listenGitRepoChanged: mockListenGitRepoChanged,
}));

import { gitRepository } from "./gitRepository";

describe("gitRepository", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("starts watching a Git repository through the native adapter", async () => {
    mockIpc.watchGitRepo.mockResolvedValue(undefined);

    await gitRepository.watchGitRepo("C:/repo");

    expect(mockIpc.watchGitRepo).toHaveBeenCalledWith("C:/repo");
  });

  it("initializes or validates a Git repository through the native adapter", async () => {
    const status = { canInitialize: true, blockingRepoPath: null };
    mockIpc.initGitRepo.mockResolvedValue(status);

    await expect(gitRepository.initGitRepo("C:/repo", true)).resolves.toBe(status);

    expect(mockIpc.initGitRepo).toHaveBeenCalledWith("C:/repo", true);
  });

  it("listens for Git repository change events through the native adapter", async () => {
    const stop = vi.fn();
    const handler = vi.fn();
    mockListenGitRepoChanged.mockResolvedValue(stop);

    await expect(gitRepository.listenGitRepoChanged(handler)).resolves.toBe(stop);

    expect(mockListenGitRepoChanged).toHaveBeenCalledWith(handler);
  });
});

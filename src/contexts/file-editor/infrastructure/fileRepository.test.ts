import { beforeEach, describe, expect, it, vi } from "vitest";

const mockIpc = vi.hoisted(() => ({
  createDir: vi.fn(),
  createFile: vi.fn(),
  deletePath: vi.fn(),
  getGitFileCompare: vi.fn(),
  listDir: vi.fn(),
  openPathWithDefaultApp: vi.fn(),
  readFile: vi.fn(),
  renamePath: vi.fn(),
  revealPath: vi.fn(),
  searchWorkspaceFiles: vi.fn(),
  writeFile: vi.fn(),
}));

vi.mock("../../../lib/ipc", () => ({
  ipc: mockIpc,
}));

import { fileRepository } from "./fileRepository";

describe("fileRepository", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("lists directory entries through the native file adapter", async () => {
    const entries = [{ path: "src/App.tsx", isDir: false }];
    mockIpc.listDir.mockResolvedValue(entries);

    await expect(fileRepository.listDir("C:/repo", "src")).resolves.toBe(entries);

    expect(mockIpc.listDir).toHaveBeenCalledWith("C:/repo", "src");
  });

  it("searches workspace files through the native file adapter", async () => {
    const page = { entries: [], offset: 0, limit: 50, total: 0, hasMore: false, scanTruncated: false };
    mockIpc.searchWorkspaceFiles.mockResolvedValue(page);

    await expect(
      fileRepository.searchWorkspaceFiles("workspace-1", "query", 0, 50, true),
    ).resolves.toBe(page);

    expect(mockIpc.searchWorkspaceFiles).toHaveBeenCalledWith(
      "workspace-1",
      "query",
      0,
      50,
      true,
    );
  });

  it("opens a path with the default app through the native file adapter", async () => {
    mockIpc.openPathWithDefaultApp.mockResolvedValue(undefined);

    await fileRepository.openPathWithDefaultApp("C:/repo/file.txt");

    expect(mockIpc.openPathWithDefaultApp).toHaveBeenCalledWith("C:/repo/file.txt");
  });
});

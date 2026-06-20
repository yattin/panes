import { beforeEach, describe, expect, it, vi } from "vitest";

const mockIpc = vi.hoisted(() => ({
  archiveWorkspace: vi.fn(),
  bindCueLightProject: vi.fn(),
  getCueLightBinding: vi.fn(),
  getRepos: vi.fn(),
  getWorkspaceStartupPreset: vi.fn(),
  hasWorkspaceGitSelection: vi.fn(),
  listArchivedWorkspaces: vi.fn(),
  listWorkspaces: vi.fn(),
  normalizeWorkspaceStartupPreset: vi.fn(),
  normalizeWorkspaceStartupPresetRaw: vi.fn(),
  openWorkspace: vi.fn(),
  restoreWorkspace: vi.fn(),
  revealPath: vi.fn(),
  serializeWorkspaceStartupPreset: vi.fn(),
  setRepoGitActive: vi.fn(),
  setRepoTrustLevel: vi.fn(),
  setWorkspaceStartupPreset: vi.fn(),
  setWorkspaceStartupPresetRaw: vi.fn(),
  setWorkspaceGitActiveRepos: vi.fn(),
  clearWorkspaceStartupPreset: vi.fn(),
  unbindCueLightProject: vi.fn(),
}));

vi.mock("../../../lib/ipc", () => ({
  ipc: mockIpc,
}));

import { workspaceRepository } from "./workspaceRepository";

describe("workspaceRepository", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("reveals a workspace path through the native adapter", async () => {
    mockIpc.revealPath.mockResolvedValue(undefined);

    await workspaceRepository.revealWorkspacePath("C:/projects/panes");

    expect(mockIpc.revealPath).toHaveBeenCalledWith("C:/projects/panes");
  });

  it("normalizes a raw workspace startup preset through the native adapter", async () => {
    const preset = { version: 1, terminal: { applyWhen: "no_live_sessions", groups: [] } };
    mockIpc.normalizeWorkspaceStartupPresetRaw.mockResolvedValue(preset);

    await expect(
      workspaceRepository.normalizeWorkspaceStartupPresetRaw("workspace-1", "json", "{}"),
    ).resolves.toBe(preset);

    expect(mockIpc.normalizeWorkspaceStartupPresetRaw).toHaveBeenCalledWith(
      "workspace-1",
      "json",
      "{}",
    );
  });

  it("saves a workspace startup preset through the native adapter", async () => {
    const preset = { version: 1, terminal: { applyWhen: "no_live_sessions", groups: [] } };
    mockIpc.setWorkspaceStartupPreset.mockResolvedValue(preset);

    await expect(
      workspaceRepository.setWorkspaceStartupPreset("workspace-1", preset as never),
    ).resolves.toBe(preset);

    expect(mockIpc.setWorkspaceStartupPreset).toHaveBeenCalledWith("workspace-1", preset);
  });
});

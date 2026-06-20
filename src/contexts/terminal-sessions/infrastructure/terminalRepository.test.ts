import { beforeEach, describe, expect, it, vi } from "vitest";

const mockIpc = vi.hoisted(() => ({
  addGitWorktree: vi.fn(),
  getRepos: vi.fn(),
  getWorkspaceStartupPreset: vi.fn(),
  launchHarness: vi.fn(),
  removeGitWorktree: vi.fn(),
  terminalClearNotification: vi.fn(),
  terminalCloseSession: vi.fn(),
  terminalCloseWorkspaceSessions: vi.fn(),
  terminalCreateSession: vi.fn(),
  terminalDrainOutput: vi.fn(),
  terminalGetRendererDiagnostics: vi.fn(),
  terminalListNotifications: vi.fn(),
  terminalListSessions: vi.fn(),
  terminalResumeSession: vi.fn(),
  terminalResize: vi.fn(),
  terminalSetNotificationFocus: vi.fn(),
  terminalWrite: vi.fn(),
  terminalWriteBytes: vi.fn(),
}));

const mockListenTerminalExit = vi.hoisted(() => vi.fn());
const mockListenTerminalForegroundChanged = vi.hoisted(() => vi.fn());
const mockListenTerminalNotification = vi.hoisted(() => vi.fn());
const mockListenTerminalNotificationCleared = vi.hoisted(() => vi.fn());
const mockListenTerminalOutput = vi.hoisted(() => vi.fn());
const mockWriteCommandToNewSession = vi.hoisted(() => vi.fn());

vi.mock("../../../lib/ipc", () => ({
  ipc: mockIpc,
  listenTerminalExit: mockListenTerminalExit,
  listenTerminalForegroundChanged: mockListenTerminalForegroundChanged,
  listenTerminalNotification: mockListenTerminalNotification,
  listenTerminalNotificationCleared: mockListenTerminalNotificationCleared,
  listenTerminalOutput: mockListenTerminalOutput,
  writeCommandToNewSession: mockWriteCommandToNewSession,
}));

import { terminalRepository } from "./terminalRepository";

describe("terminalRepository", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("writes terminal byte input through the native terminal adapter", async () => {
    mockIpc.terminalWriteBytes.mockResolvedValue(undefined);

    await expect(
      terminalRepository.terminalWriteBytes("workspace-1", "session-1", [13]),
    ).resolves.toBeUndefined();

    expect(mockIpc.terminalWriteBytes).toHaveBeenCalledWith("workspace-1", "session-1", [13]);
  });

  it("resizes terminal sessions through the native terminal adapter", async () => {
    mockIpc.terminalResize.mockResolvedValue(undefined);

    await expect(
      terminalRepository.terminalResize("workspace-1", "session-1", 80, 24, 640, 384),
    ).resolves.toBeUndefined();

    expect(mockIpc.terminalResize).toHaveBeenCalledWith(
      "workspace-1",
      "session-1",
      80,
      24,
      640,
      384,
    );
  });

  it("loads terminal renderer diagnostics through the native terminal adapter", async () => {
    const diagnostics = { mode: "webgl" };
    mockIpc.terminalGetRendererDiagnostics.mockResolvedValue(diagnostics);

    await expect(
      terminalRepository.terminalGetRendererDiagnostics("workspace-1", "session-1"),
    ).resolves.toBe(diagnostics);

    expect(mockIpc.terminalGetRendererDiagnostics).toHaveBeenCalledWith(
      "workspace-1",
      "session-1",
    );
  });

  it("resumes and drains terminal output through the native terminal adapter", async () => {
    const resume = { chunks: [], nextSeq: 7 };
    const drained = { chunks: [], nextSeq: 9 };
    mockIpc.terminalResumeSession.mockResolvedValue(resume);
    mockIpc.terminalDrainOutput.mockResolvedValue(drained);

    await expect(
      terminalRepository.terminalResumeSession("workspace-1", "session-1", 5),
    ).resolves.toBe(resume);
    await expect(
      terminalRepository.terminalDrainOutput("workspace-1", "session-1", 7, 65536),
    ).resolves.toBe(drained);

    expect(mockIpc.terminalResumeSession).toHaveBeenCalledWith("workspace-1", "session-1", 5);
    expect(mockIpc.terminalDrainOutput).toHaveBeenCalledWith(
      "workspace-1",
      "session-1",
      7,
      65536,
    );
  });

  it("listens to terminal events through the native terminal adapter", async () => {
    const unlisten = vi.fn();
    const onEvent = vi.fn();
    mockListenTerminalOutput.mockResolvedValue(unlisten);
    mockListenTerminalExit.mockResolvedValue(unlisten);
    mockListenTerminalForegroundChanged.mockResolvedValue(unlisten);
    mockListenTerminalNotification.mockResolvedValue(unlisten);
    mockListenTerminalNotificationCleared.mockResolvedValue(unlisten);

    await expect(
      terminalRepository.listenTerminalOutput("workspace-1", onEvent),
    ).resolves.toBe(unlisten);
    await expect(
      terminalRepository.listenTerminalExit("workspace-1", onEvent),
    ).resolves.toBe(unlisten);
    await expect(
      terminalRepository.listenTerminalForegroundChanged("workspace-1", onEvent),
    ).resolves.toBe(unlisten);
    await expect(
      terminalRepository.listenTerminalNotification("workspace-1", onEvent),
    ).resolves.toBe(unlisten);
    await expect(
      terminalRepository.listenTerminalNotificationCleared("workspace-1", onEvent),
    ).resolves.toBe(unlisten);

    expect(mockListenTerminalOutput).toHaveBeenCalledWith("workspace-1", onEvent);
    expect(mockListenTerminalExit).toHaveBeenCalledWith("workspace-1", onEvent);
    expect(mockListenTerminalForegroundChanged).toHaveBeenCalledWith("workspace-1", onEvent);
    expect(mockListenTerminalNotification).toHaveBeenCalledWith("workspace-1", onEvent);
    expect(mockListenTerminalNotificationCleared).toHaveBeenCalledWith("workspace-1", onEvent);
  });
});

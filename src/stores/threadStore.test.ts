import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Thread } from "../types";

const mockIpc = vi.hoisted(() => ({
  archiveThread: vi.fn(),
  attachCodexRemoteThread: vi.fn(),
  attachOpenCodeRemoteSession: vi.fn(),
  clearLastActiveThreadId: vi.fn(),
  compactCodexThread: vi.fn(),
  createThread: vi.fn(),
  forkCodexThread: vi.fn(),
  listArchivedThreads: vi.fn(),
  listThreads: vi.fn(),
  readLastActiveThreadId: vi.fn(),
  renameThread: vi.fn(),
  restoreThread: vi.fn(),
  rollbackCodexThread: vi.fn(),
  writeLastActiveThreadId: vi.fn(),
}));

vi.mock("../contexts/engines/application/engineStore", () => ({
  useEngineStore: {
    getState: () => ({ engines: [] }),
  },
}));

vi.mock("../contexts/onboarding/application/onboardingStore", () => ({
  useOnboardingStore: {
    getState: () => ({ selectedChatEngines: [] }),
  },
}));

vi.mock("../contexts/chat-composer/application/chatComposerStore", () => ({
  useChatComposerStore: {
    getState: () => ({ runtimeByWorkspace: {} }),
  },
}));

import { configureThreadGateway } from "../contexts/threads/application/threadGateway";
import { useThreadStore } from "./threadStore";

function thread(id: string, workspaceId: string, lastActivityAt: string): Thread {
  return {
    id,
    workspaceId,
    repoId: null,
    engineId: "codex",
    modelId: "gpt-5.4",
    engineThreadId: null,
    title: id,
    status: "idle",
    messageCount: 0,
    totalTokens: 0,
    createdAt: "2026-01-01T00:00:00.000Z",
    lastActivityAt,
  };
}

describe("threadStore", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockIpc.readLastActiveThreadId.mockReturnValue("thread-old");
    configureThreadGateway(mockIpc);
    useThreadStore.setState({
      threads: [],
      threadsByWorkspace: {},
      archivedThreadsByWorkspace: {},
      activeThreadId: null,
      loading: false,
      error: undefined,
    });
  });

  it("refreshes all workspaces, sorts threads by activity, and restores the saved active thread", async () => {
    const oldThread = thread("thread-old", "workspace-1", "2026-01-01T00:00:00.000Z");
    const newThread = thread("thread-new", "workspace-2", "2026-02-01T00:00:00.000Z");
    mockIpc.listThreads.mockImplementation(async (workspaceId: string) => {
      if (workspaceId === "workspace-1") {
        return [oldThread];
      }
      if (workspaceId === "workspace-2") {
        return [newThread];
      }
      return [];
    });

    await useThreadStore.getState().refreshAllThreads(["workspace-1", "workspace-2"]);

    expect(mockIpc.listThreads).toHaveBeenCalledWith("workspace-1");
    expect(mockIpc.listThreads).toHaveBeenCalledWith("workspace-2");
    expect(useThreadStore.getState().threads.map((item) => item.id)).toEqual([
      "thread-new",
      "thread-old",
    ]);
    expect(useThreadStore.getState()).toMatchObject({
      activeThreadId: "thread-old",
      loading: false,
      error: undefined,
    });
  });

  it("persists and clears the active thread selection", () => {
    useThreadStore.getState().setActiveThread("thread-selected");

    expect(mockIpc.writeLastActiveThreadId).toHaveBeenCalledWith("thread-selected");
    expect(useThreadStore.getState().activeThreadId).toBe("thread-selected");

    useThreadStore.getState().setActiveThread(null);

    expect(mockIpc.clearLastActiveThreadId).toHaveBeenCalled();
    expect(useThreadStore.getState().activeThreadId).toBeNull();
  });

  it("reuses the active scoped thread before falling back to the most recent match", async () => {
    const activeThread = thread("thread-active", "workspace-1", "2026-01-01T00:00:00.000Z");
    const newerThread = thread("thread-newer", "workspace-1", "2026-02-01T00:00:00.000Z");
    mockIpc.listThreads.mockResolvedValue([newerThread, activeThread]);
    useThreadStore.setState({ activeThreadId: activeThread.id });

    const selectedId = await useThreadStore.getState().ensureThreadForScope({
      workspaceId: "workspace-1",
      repoId: null,
      engineId: "codex",
      modelId: "gpt-5.4",
    });

    expect(selectedId).toBe(activeThread.id);
    expect(useThreadStore.getState().activeThreadId).toBe(activeThread.id);
    expect(useThreadStore.getState().threadsByWorkspace["workspace-1"].map((item) => item.id)).toEqual([
      activeThread.id,
      newerThread.id,
    ]);
  });
});

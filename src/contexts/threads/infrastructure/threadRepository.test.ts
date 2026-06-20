import { beforeEach, describe, expect, it, vi } from "vitest";

const mockIpc = vi.hoisted(() => ({
  archiveThread: vi.fn(),
  attachCodexRemoteThread: vi.fn(),
  attachOpenCodeRemoteSession: vi.fn(),
  compactCodexThread: vi.fn(),
  createThread: vi.fn(),
  forkCodexThread: vi.fn(),
  listArchivedThreads: vi.fn(),
  listThreads: vi.fn(),
  renameThread: vi.fn(),
  restoreThread: vi.fn(),
  rollbackCodexThread: vi.fn(),
}));

const mockListenThreadUpdated = vi.hoisted(() => vi.fn());

vi.mock("../../../lib/ipc", () => ({
  ipc: mockIpc,
  listenThreadUpdated: mockListenThreadUpdated,
}));

import { threadRepository } from "./threadRepository";

describe("threadRepository", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("listens for thread updates through the native thread adapter", async () => {
    const unlisten = vi.fn();
    const onEvent = vi.fn();
    mockListenThreadUpdated.mockResolvedValue(unlisten);

    await expect(threadRepository.listenThreadUpdated(onEvent)).resolves.toBe(unlisten);

    expect(mockListenThreadUpdated).toHaveBeenCalledWith(onEvent);
  });
});

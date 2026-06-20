import { beforeEach, describe, expect, it, vi } from "vitest";

const mockIpc = vi.hoisted(() => ({
  showAgentNotification: vi.fn(),
}));

const mockListenMenuAction = vi.hoisted(() => vi.fn());

vi.mock("../../../lib/ipc", () => ({
  ipc: mockIpc,
  listenMenuAction: mockListenMenuAction,
}));

import { shellNativeRepository } from "./shellNativeRepository";

describe("shellNativeRepository", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("shows agent notifications through the native shell adapter", async () => {
    mockIpc.showAgentNotification.mockResolvedValue(undefined);

    await expect(
      shellNativeRepository.showAgentNotification("Codex", "Done"),
    ).resolves.toBeUndefined();

    expect(mockIpc.showAgentNotification).toHaveBeenCalledWith("Codex", "Done");
  });

  it("listens for native menu actions through the native shell adapter", async () => {
    const unlisten = vi.fn();
    const onEvent = vi.fn();
    mockListenMenuAction.mockResolvedValue(unlisten);

    await expect(shellNativeRepository.listenMenuAction(onEvent)).resolves.toBe(unlisten);

    expect(mockListenMenuAction).toHaveBeenCalledWith(onEvent);
  });
});

import { beforeEach, describe, expect, it, vi } from "vitest";

const mockIpc = vi.hoisted(() => ({
  engineHealth: vi.fn(),
  listEngines: vi.fn(),
}));

const mockListenEngineRuntimeUpdated = vi.hoisted(() => vi.fn());

vi.mock("../../../lib/ipc", () => ({
  ipc: mockIpc,
  listenEngineRuntimeUpdated: mockListenEngineRuntimeUpdated,
}));

import {
  getEngineHealth,
  listenEngineRuntimeUpdated,
  listEngines,
} from "./engineRepository";

describe("engineRepository", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("loads engines through the native engine adapter", async () => {
    const engines = [{ id: "codex" }];
    mockIpc.listEngines.mockResolvedValue(engines);

    await expect(listEngines()).resolves.toBe(engines);

    expect(mockIpc.listEngines).toHaveBeenCalledWith();
  });

  it("loads engine health through the native engine adapter", async () => {
    const health = { ok: true };
    mockIpc.engineHealth.mockResolvedValue(health);

    await expect(getEngineHealth("codex")).resolves.toBe(health);

    expect(mockIpc.engineHealth).toHaveBeenCalledWith("codex");
  });

  it("listens for engine runtime updates through the native engine adapter", async () => {
    const unlisten = vi.fn();
    const onEvent = vi.fn();
    mockListenEngineRuntimeUpdated.mockResolvedValue(unlisten);

    await expect(listenEngineRuntimeUpdated(onEvent)).resolves.toBe(unlisten);

    expect(mockListenEngineRuntimeUpdated).toHaveBeenCalledWith(onEvent);
  });
});

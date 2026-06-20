import { beforeEach, describe, expect, it, vi } from "vitest";

const mockIpc = vi.hoisted(() => ({
  getTerminalAcceleratedRendering: vi.fn(),
  setTerminalAcceleratedRendering: vi.fn(),
}));

vi.mock("../../../lib/ipc", () => ({
  ipc: mockIpc,
}));

import {
  getTerminalAcceleratedRenderingPreference,
  setTerminalAcceleratedRenderingPreference,
} from "./terminalRenderingSettings";

describe("terminalRenderingSettings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("loads the accelerated rendering preference through the native adapter", async () => {
    mockIpc.getTerminalAcceleratedRendering.mockResolvedValue(true);

    await expect(getTerminalAcceleratedRenderingPreference()).resolves.toBe(true);

    expect(mockIpc.getTerminalAcceleratedRendering).toHaveBeenCalledWith();
  });

  it("saves the accelerated rendering preference through the native adapter", async () => {
    mockIpc.setTerminalAcceleratedRendering.mockResolvedValue(false);

    await expect(setTerminalAcceleratedRenderingPreference(false)).resolves.toBe(false);

    expect(mockIpc.setTerminalAcceleratedRendering).toHaveBeenCalledWith(false);
  });
});

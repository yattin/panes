import { beforeEach, describe, expect, it, vi } from "vitest";
import type { HarnessInfo } from "../types";

const mockIpc = vi.hoisted(() => ({
  checkHarnesses: vi.fn(),
  launchHarness: vi.fn(),
}));

import { configureHarnessGateway } from "../contexts/harnesses/application/harnessGateway";
import { useHarnessStore } from "./harnessStore";

function harness(id: string, found: boolean): HarnessInfo {
  return {
    id,
    name: id,
    description: `${id} harness`,
    command: id,
    found,
    version: found ? "1.0.0" : null,
    path: found ? `/bin/${id}` : null,
    canAutoInstall: !found,
    website: `https://example.com/${id}`,
    native: false,
  };
}

describe("harnessStore", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    configureHarnessGateway({
      checkHarnesses: mockIpc.checkHarnesses,
      launchHarness: mockIpc.launchHarness,
    });
    useHarnessStore.setState({
      phase: "idle",
      harnesses: [],
      npmAvailable: false,
      error: null,
      loadedOnce: false,
    });
  });

  it("scans harnesses and exposes only installed harnesses", async () => {
    mockIpc.checkHarnesses.mockResolvedValue({
      npmAvailable: true,
      harnesses: [harness("codex", true), harness("kiro", false)],
    });

    await useHarnessStore.getState().scan();

    expect(mockIpc.checkHarnesses).toHaveBeenCalledTimes(1);
    expect(useHarnessStore.getState()).toMatchObject({
      phase: "idle",
      npmAvailable: true,
      loadedOnce: true,
      error: null,
    });
    expect(useHarnessStore.getState().getInstalledHarnesses()).toEqual([
      harness("codex", true),
    ]);
  });
});

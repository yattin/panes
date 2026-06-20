import { beforeEach, describe, expect, it, vi } from "vitest";

const mockIpc = vi.hoisted(() => ({
  checkDependencies: vi.fn(),
  installDependency: vi.fn(),
  installHarness: vi.fn(),
}));

vi.mock("../../../lib/ipc", () => ({
  ipc: mockIpc,
  listenInstallProgress: vi.fn(),
}));

import { onboardingRepository } from "./onboardingRepository";

describe("onboardingRepository", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("checks dependencies through the native onboarding adapter", async () => {
    const report = {
      node: { found: true, version: "22.0.0", path: "node", canAutoInstall: false, installMethod: null },
      codex: { found: false, version: null, path: null, canAutoInstall: true, installMethod: "npm" },
      git: { found: true, version: "2.0.0", path: "git", canAutoInstall: false, installMethod: null },
      platform: "windows",
      packageManagers: ["npm"],
    };
    mockIpc.checkDependencies.mockResolvedValue(report);

    await expect(onboardingRepository.checkDependencies()).resolves.toBe(report);

    expect(mockIpc.checkDependencies).toHaveBeenCalledWith();
  });
});

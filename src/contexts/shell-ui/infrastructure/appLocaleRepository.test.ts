import { beforeEach, describe, expect, it, vi } from "vitest";

const mockIpc = vi.hoisted(() => ({
  getAppLocale: vi.fn(),
  setAppLocale: vi.fn(),
}));

vi.mock("../../../lib/ipc", () => ({
  ipc: mockIpc,
}));

import { appLocaleRepository } from "./appLocaleRepository";

describe("appLocaleRepository", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("loads the persisted app locale through the native adapter", async () => {
    mockIpc.getAppLocale.mockResolvedValue("zh-CN");

    await expect(appLocaleRepository.getPersistedLocale()).resolves.toBe("zh-CN");

    expect(mockIpc.getAppLocale).toHaveBeenCalledWith();
  });

  it("saves the app locale through the native adapter", async () => {
    mockIpc.setAppLocale.mockResolvedValue("pt-BR");

    await expect(appLocaleRepository.setPersistedLocale("pt-BR")).resolves.toBe("pt-BR");

    expect(mockIpc.setAppLocale).toHaveBeenCalledWith("pt-BR");
  });
});

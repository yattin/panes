import { beforeEach, describe, expect, it, vi } from "vitest";
import type { TerminalNotificationSettings } from "../types";

const mockIpc = vi.hoisted(() => ({
  getTerminalNotificationSettings: vi.fn(),
  installTerminalNotificationIntegration: vi.fn(),
  previewNotificationSound: vi.fn(),
  setChatNotificationsEnabled: vi.fn(),
  setNotificationSound: vi.fn(),
  setTerminalNotificationsEnabled: vi.fn(),
}));

vi.mock("../contexts/shell-ui/application/toastStore", () => ({
  toast: {
    success: vi.fn(),
    error: vi.fn(),
  },
}));

vi.mock("../i18n", () => ({
  t: (key: string) => key,
}));

import { configureTerminalNotificationSettingsGateway } from "../contexts/terminal-sessions/application/terminalNotificationSettingsGateway";
import { useTerminalNotificationSettingsStore } from "./terminalNotificationSettingsStore";

function makeSettings(): TerminalNotificationSettings {
  const integration = {
    configured: false,
    configExists: false,
    conflict: false,
  };
  return {
    chatEnabled: true,
    terminalEnabled: false,
    terminalSetupComplete: true,
    notificationSound: "Glass",
    claude: integration,
    codex: integration,
  };
}

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

async function flushPromises() {
  await new Promise((resolve) => setTimeout(resolve, 0));
}

describe("terminalNotificationSettingsStore", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    configureTerminalNotificationSettingsGateway(mockIpc);
    useTerminalNotificationSettingsStore.setState({
      settings: null,
      loading: false,
      loadedOnce: false,
      modalOpen: false,
      updatingChatEnabled: false,
      updatingTerminalEnabled: false,
      installingIntegration: null,
    });
  });

  it("reuses one in-flight settings load for overlapping requests", async () => {
    const settings = makeSettings();
    const request = deferred<TerminalNotificationSettings>();
    mockIpc.getTerminalNotificationSettings.mockReturnValueOnce(request.promise);

    const first = useTerminalNotificationSettingsStore.getState().load();
    await flushPromises();
    const second = useTerminalNotificationSettingsStore.getState().load();

    expect(mockIpc.getTerminalNotificationSettings).toHaveBeenCalledTimes(1);
    expect(useTerminalNotificationSettingsStore.getState().loading).toBe(true);

    request.resolve(settings);

    await expect(first).resolves.toEqual(settings);
    await expect(second).resolves.toEqual(settings);
    expect(useTerminalNotificationSettingsStore.getState()).toMatchObject({
      settings,
      loading: false,
      loadedOnce: true,
    });
  });
});

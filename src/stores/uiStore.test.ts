import { beforeEach, describe, expect, it, vi } from "vitest";
import { COMMAND_PALETTE_DEFAULT_LAUNCH } from "../contexts/shell-ui/domain/commandPalette";
import type { AppTheme } from "../contexts/shell-ui/domain/appTheme";

type UiStoreModule = typeof import("./uiStore");
type ShellUiGatewayModule = typeof import("../contexts/shell-ui/application/shellUiGateway");
type ShellUiStoreModule = typeof import("../contexts/shell-ui/application/uiStore");

describe("uiStore focus mode", () => {
  let useUiStore: UiStoreModule["useUiStore"];
  let hydrateShellUiPreferences: ShellUiStoreModule["hydrateShellUiPreferences"];
  let configureShellUiGateway: ShellUiGatewayModule["configureShellUiGateway"];
  const shellUiGateway = {
    closeNativeWindow: vi.fn<() => Promise<void>>(() => Promise.resolve()),
    destroyNativeWindow: vi.fn<() => Promise<void>>(() => Promise.resolve()),
    getAppVersion: vi.fn<() => Promise<string>>(() => Promise.resolve("0.0.0")),
    getPersistedAppLocale: vi.fn(() => Promise.resolve(null)),
    getPersistedAppTheme: vi.fn<() => Promise<AppTheme>>(() => Promise.resolve("dark")),
    hideNativeWindow: vi.fn<() => Promise<void>>(() => Promise.resolve()),
    isNativeWindowFullscreen: vi.fn<() => Promise<boolean>>(() => Promise.resolve(false)),
    isTauriRuntime: vi.fn<() => boolean>(() => false),
    listenWindowFileDrops: vi.fn(() => Promise.resolve(() => undefined)),
    minimizeNativeWindow: vi.fn<() => Promise<void>>(() => Promise.resolve()),
    now: vi.fn<() => number>(() => 1234),
    openExternalUrl: vi.fn<() => Promise<void>>(() => Promise.resolve()),
    readCachedAppTheme: vi.fn<() => AppTheme | null>(() => null),
    readExplorerOpenPreference: vi.fn<() => boolean | null>(() => null),
    readGitPanelPinnedPreference: vi.fn<() => boolean | null>(() => null),
    readSidebarPinnedPreference: vi.fn<() => boolean | null>(() => null),
    saveTextFile: vi.fn<() => Promise<boolean>>(() => Promise.resolve(false)),
    selectDirectoryPath: vi.fn<() => Promise<string | null>>(() => Promise.resolve(null)),
    selectFilePaths: vi.fn<() => Promise<string[]>>(() => Promise.resolve([])),
    selectTextFile: vi.fn<() => Promise<{ path: string; text: string } | null>>(() =>
      Promise.resolve(null),
    ),
    setNativeWindowFullscreen: vi.fn<() => Promise<void>>(() => Promise.resolve()),
    setPersistedAppLocale: vi.fn((locale) => Promise.resolve(locale)),
    setPersistedAppTheme: vi.fn<(theme: AppTheme) => Promise<AppTheme>>((theme) => Promise.resolve(theme)),
    startNativeWindowDrag: vi.fn<() => Promise<void>>(() => Promise.resolve()),
    startNativeWindowResizeDrag: vi.fn<() => Promise<void>>(() => Promise.resolve()),
    toggleNativeWindowMaximize: vi.fn<() => Promise<void>>(() => Promise.resolve()),
    writeCachedAppTheme: vi.fn(),
    writeExplorerOpenPreference: vi.fn(),
    writeGitPanelPinnedPreference: vi.fn(),
    writeSidebarPinnedPreference: vi.fn(),
  };

  beforeEach(async () => {
    vi.resetModules();
    vi.clearAllMocks();
    shellUiGateway.getPersistedAppTheme.mockResolvedValue("dark");
    shellUiGateway.readCachedAppTheme.mockReturnValue(null);
    shellUiGateway.setPersistedAppTheme.mockImplementation((theme) => Promise.resolve(theme));
    Object.defineProperty(globalThis, "document", {
      configurable: true,
      value: {
        documentElement: {
          theme: "dark",
          setAttribute(name: string, value: string) {
            if (name === "data-theme") {
              this.theme = value;
            }
          },
          getAttribute(name: string) {
            return name === "data-theme" ? this.theme : null;
          },
        },
      },
    });
    ({ useUiStore } = await import("./uiStore"));
    ({ configureShellUiGateway } = await import("../contexts/shell-ui/application/shellUiGateway"));
    ({ hydrateShellUiPreferences } = await import("../contexts/shell-ui/application/uiStore"));
    configureShellUiGateway(shellUiGateway);
    useUiStore.setState({
      showSidebar: true,
      sidebarPinned: true,
      showGitPanel: true,
      gitPanelPinned: true,
      showExplorer: true,
      theme: "dark",
      focusMode: false,
      focusModeSnapshot: null,
      activeView: "chat",
      settingsWorkspaceId: null,
      commandPaletteOpen: false,
      commandPaletteLaunch: COMMAND_PALETTE_DEFAULT_LAUNCH,
      messageFocusTarget: null,
    });
  });

  it("captures the current shell state and hides the left sidebar on entry", () => {
    useUiStore.getState().setFocusMode(true);

    expect(useUiStore.getState()).toMatchObject({
      focusMode: true,
      showSidebar: false,
      showGitPanel: true,
      focusModeSnapshot: {
        showSidebar: true,
        showGitPanel: true,
      },
    });
  });

  it("keeps sidebar and git toggles working while focus mode is active", () => {
    const state = useUiStore.getState();

    state.setFocusMode(true);
    state.toggleSidebar();
    state.toggleGitPanel();

    expect(useUiStore.getState()).toMatchObject({
      focusMode: true,
      showSidebar: true,
      showGitPanel: false,
    });
  });

  it("restores the pre-focus shell state when leaving focus mode", () => {
    useUiStore.setState({
      showSidebar: true,
      showGitPanel: false,
      gitPanelPinned: true,
      focusMode: false,
      focusModeSnapshot: null,
    });

    const state = useUiStore.getState();
    state.setFocusMode(true);
    state.toggleSidebar();
    state.toggleGitPanel();
    state.toggleFocusMode();

    expect(useUiStore.getState()).toMatchObject({
      focusMode: false,
      showSidebar: true,
      showGitPanel: false,
      focusModeSnapshot: null,
    });
  });

  it("does not overwrite the original snapshot on repeated activation", () => {
    useUiStore.setState({
      showSidebar: false,
      showGitPanel: true,
      gitPanelPinned: false,
      focusMode: false,
      focusModeSnapshot: null,
    });

    const state = useUiStore.getState();
    state.setFocusMode(true);
    state.toggleGitPanel();
    state.setFocusMode(true);
    state.setFocusMode(false);

    expect(useUiStore.getState()).toMatchObject({
      focusMode: false,
      showSidebar: false,
      showGitPanel: true,
      gitPanelPinned: false,
      focusModeSnapshot: null,
    });
  });

  it("keeps git pin state separate from visibility toggles", () => {
    const state = useUiStore.getState();

    state.setGitPanelPinned(false);
    state.toggleGitPanel();
    state.toggleGitPanel();

    expect(useUiStore.getState()).toMatchObject({
      showGitPanel: true,
      gitPanelPinned: false,
    });
  });

  it("persists git pin state changes and forces the panel visible", () => {
    const state = useUiStore.getState();

    useUiStore.setState({ showGitPanel: false, gitPanelPinned: true });
    state.toggleGitPanelPin();

    expect(shellUiGateway.writeGitPanelPinnedPreference).toHaveBeenCalledWith(false);
    expect(useUiStore.getState()).toMatchObject({
      showGitPanel: true,
      gitPanelPinned: false,
    });
  });

  it("persists explicit explorer visibility changes", () => {
    useUiStore.getState().setExplorerOpen(false);

    expect(shellUiGateway.writeExplorerOpenPreference).toHaveBeenCalledWith(false);
    expect(useUiStore.getState().showExplorer).toBe(false);
  });

  it("hydrates persisted shell preferences through the shell UI gateway", () => {
    shellUiGateway.readSidebarPinnedPreference.mockReturnValue(false);
    shellUiGateway.readGitPanelPinnedPreference.mockReturnValue(false);
    shellUiGateway.readExplorerOpenPreference.mockReturnValue(false);

    hydrateShellUiPreferences();

    expect(useUiStore.getState()).toMatchObject({
      sidebarPinned: false,
      gitPanelPinned: false,
      showExplorer: false,
    });
  });

  it("hydrates cached theme immediately and then applies persisted theme", async () => {
    shellUiGateway.readCachedAppTheme.mockReturnValue("light");
    shellUiGateway.getPersistedAppTheme.mockResolvedValue("dark");

    hydrateShellUiPreferences();

    expect(useUiStore.getState().theme).toBe("light");
    expect(document.documentElement.getAttribute("data-theme")).toBe("light");

    await Promise.resolve();

    expect(shellUiGateway.getPersistedAppTheme).toHaveBeenCalled();
    expect(shellUiGateway.writeCachedAppTheme).toHaveBeenCalledWith("dark");
    expect(useUiStore.getState().theme).toBe("dark");
    expect(document.documentElement.getAttribute("data-theme")).toBe("dark");
  });

  it("persists theme changes", async () => {
    shellUiGateway.setPersistedAppTheme.mockResolvedValue("light");

    await expect(useUiStore.getState().setTheme("light")).resolves.toBe("light");

    expect(shellUiGateway.setPersistedAppTheme).toHaveBeenCalledWith("light");
    expect(shellUiGateway.writeCachedAppTheme).toHaveBeenCalledWith("light");
    expect(useUiStore.getState().theme).toBe("light");
    expect(document.documentElement.getAttribute("data-theme")).toBe("light");
  });

  it("rolls back theme changes when persistence fails", async () => {
    useUiStore.setState({ theme: "dark" });
    document.documentElement.setAttribute("data-theme", "dark");
    shellUiGateway.setPersistedAppTheme.mockRejectedValue(new Error("save failed"));

    await expect(useUiStore.getState().setTheme("light")).resolves.toBeNull();

    expect(useUiStore.getState().theme).toBe("dark");
    expect(document.documentElement.getAttribute("data-theme")).toBe("dark");
    expect(shellUiGateway.writeCachedAppTheme).toHaveBeenLastCalledWith("dark");
  });

  it("opens the command palette with structured launch defaults", () => {
    useUiStore.getState().openCommandPalette({ variant: "search", initialQuery: "?", searchScope: "threads" });

    expect(useUiStore.getState()).toMatchObject({
      commandPaletteOpen: true,
      commandPaletteLaunch: {
        variant: "search",
        initialQuery: "?",
        searchScope: "threads",
      },
    });
  });

  it("resets command palette launch state when closing", () => {
    const state = useUiStore.getState();
    state.openCommandPalette({ variant: "search", initialQuery: "?", searchScope: "files" });
    state.closeCommandPalette();

    expect(useUiStore.getState()).toMatchObject({
      commandPaletteOpen: false,
      commandPaletteLaunch: COMMAND_PALETTE_DEFAULT_LAUNCH,
    });
  });

  it("stamps message focus requests with shell runtime time", () => {
    useUiStore.getState().setMessageFocusTarget({
      threadId: "thread-1",
      messageId: "message-1",
    });

    expect(useUiStore.getState().messageFocusTarget).toEqual({
      threadId: "thread-1",
      messageId: "message-1",
      requestedAt: 1234,
    });
  });
});

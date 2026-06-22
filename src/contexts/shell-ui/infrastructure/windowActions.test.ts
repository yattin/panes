import { beforeEach, describe, expect, it, vi } from "vitest";

const mockClose = vi.hoisted(() => vi.fn());
const mockDestroy = vi.hoisted(() => vi.fn());
const mockHide = vi.hoisted(() => vi.fn());
const mockIsFullscreen = vi.hoisted(() => vi.fn());
const mockMinimize = vi.hoisted(() => vi.fn());
const mockSetFullscreen = vi.hoisted(() => vi.fn());
const mockToggleMaximize = vi.hoisted(() => vi.fn());
const mockRequestCloseTab = vi.hoisted(() => vi.fn());
const mockIsTauri = vi.hoisted(() => vi.fn());

const mockWorkspaceState = vi.hoisted(() => ({
  activeWorkspaceId: "ws-1" as string | null,
}));

const mockTerminalState = vi.hoisted(() => ({
  workspaces: {
    "ws-1": {
      layoutMode: "editor",
    },
  } as Record<string, { layoutMode: string }>,
}));

const mockFileState = vi.hoisted(() => ({
  activeTabId: "tab-1" as string | null,
  requestCloseTab: mockRequestCloseTab,
}));

vi.mock("@tauri-apps/api/core", () => ({
  isTauri: mockIsTauri,
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    close: mockClose,
    destroy: mockDestroy,
    hide: mockHide,
    isFullscreen: mockIsFullscreen,
    minimize: mockMinimize,
    setFullscreen: mockSetFullscreen,
    toggleMaximize: mockToggleMaximize,
  }),
}));

vi.mock("../../workspaces/application/workspaceStore", () => ({
  useWorkspaceStore: {
    getState: () => mockWorkspaceState,
  },
}));

vi.mock("../../terminal-sessions/application/terminalStore", () => ({
  useTerminalStore: {
    getState: () => mockTerminalState,
  },
}));

vi.mock("../../file-editor/application/fileStore", () => ({
  useFileStore: {
    getState: () => mockFileState,
  },
}));

import { configureShellUiGateway } from "../application/shellUiGateway";
import {
  closeCurrentWindow,
  isMacDesktop,
  isLinuxDesktop,
  isWindowsDesktop,
  minimizeCurrentWindow,
  shouldHandleAppShortcutWhileTerminalFocused,
  isTerminalInputFocused,
  requestWindowClose,
  toggleCurrentWindowMaximize,
  usesCustomWindowFrame,
} from "./windowActions";

describe("windowActions", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    configureShellUiGateway({
      closeNativeWindow: mockClose,
      destroyNativeWindow: mockDestroy,
      getAppVersion: () => Promise.resolve("0.0.0"),
      getPersistedAppLocale: () => Promise.resolve(null),
      getPersistedAppTheme: () => Promise.resolve("dark"),
      hideNativeWindow: mockHide,
      isNativeWindowFullscreen: mockIsFullscreen,
      isTauriRuntime: mockIsTauri,
      listenWindowFileDrops: () => Promise.resolve(() => undefined),
      minimizeNativeWindow: mockMinimize,
      now: () => 0,
      openExternalUrl: () => Promise.resolve(),
      readCachedAppTheme: () => null,
      readExplorerOpenPreference: () => null,
      readGitPanelPinnedPreference: () => null,
      readSidebarPinnedPreference: () => null,
      saveTextFile: () => Promise.resolve(false),
      selectDirectoryPath: () => Promise.resolve(null),
      selectFilePaths: () => Promise.resolve([]),
      selectTextFile: () => Promise.resolve(null),
      setNativeWindowFullscreen: mockSetFullscreen,
      setPersistedAppLocale: (locale) => Promise.resolve(locale),
      setPersistedAppTheme: (theme) => Promise.resolve(theme),
      startNativeWindowDrag: () => Promise.resolve(),
      startNativeWindowResizeDrag: () => Promise.resolve(),
      toggleNativeWindowMaximize: mockToggleMaximize,
      writeCachedAppTheme: () => undefined,
      writeExplorerOpenPreference: () => undefined,
      writeGitPanelPinnedPreference: () => undefined,
      writeSidebarPinnedPreference: () => undefined,
    });
    mockIsTauri.mockReturnValue(true);
    mockWorkspaceState.activeWorkspaceId = "ws-1";
    mockTerminalState.workspaces = {
      "ws-1": {
        layoutMode: "editor",
      },
    };
    mockFileState.activeTabId = "tab-1";
    mockClose.mockResolvedValue(undefined);
    mockDestroy.mockResolvedValue(undefined);
    mockHide.mockResolvedValue(undefined);
    mockIsFullscreen.mockResolvedValue(false);
    mockMinimize.mockResolvedValue(undefined);
    mockSetFullscreen.mockResolvedValue(undefined);
    mockToggleMaximize.mockResolvedValue(undefined);
  });

  it("treats Linux custom chrome as Tauri-only", () => {
    const originalNavigator = Object.getOwnPropertyDescriptor(globalThis, "navigator");

    Object.defineProperty(globalThis, "navigator", {
      configurable: true,
      value: { platform: "Linux x86_64" },
    });

    try {
      expect(isLinuxDesktop()).toBe(true);

      mockIsTauri.mockReturnValue(false);
      expect(isLinuxDesktop()).toBe(false);
    } finally {
      if (originalNavigator) {
        Object.defineProperty(globalThis, "navigator", originalNavigator);
      } else {
        Reflect.deleteProperty(globalThis, "navigator");
      }
    }
  });

  it("treats Windows custom chrome as Tauri-only", () => {
    const originalNavigator = Object.getOwnPropertyDescriptor(globalThis, "navigator");

    Object.defineProperty(globalThis, "navigator", {
      configurable: true,
      value: { platform: "Win32" },
    });

    try {
      expect(isWindowsDesktop()).toBe(true);
      expect(usesCustomWindowFrame()).toBe(true);

      mockIsTauri.mockReturnValue(false);
      expect(isWindowsDesktop()).toBe(false);
      expect(usesCustomWindowFrame()).toBe(false);
    } finally {
      if (originalNavigator) {
        Object.defineProperty(globalThis, "navigator", originalNavigator);
      } else {
        Reflect.deleteProperty(globalThis, "navigator");
      }
    }
  });

  it("treats macOS custom chrome as Tauri-only", () => {
    const originalNavigator = Object.getOwnPropertyDescriptor(globalThis, "navigator");

    Object.defineProperty(globalThis, "navigator", {
      configurable: true,
      value: { platform: "MacIntel" },
    });

    try {
      expect(isMacDesktop()).toBe(true);
      expect(usesCustomWindowFrame()).toBe(false);

      mockIsTauri.mockReturnValue(false);
      expect(isMacDesktop()).toBe(false);
    } finally {
      if (originalNavigator) {
        Object.defineProperty(globalThis, "navigator", originalNavigator);
      } else {
        Reflect.deleteProperty(globalThis, "navigator");
      }
    }
  });

  it("detects focused xterm input", () => {
    expect(
      isTerminalInputFocused({
        activeElement: {
          classList: {
            contains: (className: string) => className === "xterm-helper-textarea",
          },
        } as unknown as HTMLElement,
      } as unknown as Document),
    ).toBe(true);
  });

  it("keeps app-owned shortcuts active while the terminal is focused", () => {
    expect(shouldHandleAppShortcutWhileTerminalFocused("s", false)).toBe(true);
    expect(shouldHandleAppShortcutWhileTerminalFocused("i", true)).toBe(true);
    expect(shouldHandleAppShortcutWhileTerminalFocused("d", false)).toBe(true);
    expect(shouldHandleAppShortcutWhileTerminalFocused("t", true)).toBe(true);
    expect(shouldHandleAppShortcutWhileTerminalFocused("k", false)).toBe(true);
    expect(shouldHandleAppShortcutWhileTerminalFocused("n", true)).toBe(true);
    expect(shouldHandleAppShortcutWhileTerminalFocused("b", false)).toBe(false);
    expect(shouldHandleAppShortcutWhileTerminalFocused("f", false)).toBe(false);
  });

  it("closes the active editor tab for generic close-window requests", async () => {
    await requestWindowClose();

    expect(mockRequestCloseTab).toHaveBeenCalledWith("tab-1");
    expect(mockClose).not.toHaveBeenCalled();
  });

  it("closes the native window for explicit window actions", async () => {
    await closeCurrentWindow();

    expect(mockClose).toHaveBeenCalledTimes(1);
    expect(mockRequestCloseTab).not.toHaveBeenCalled();
  });

  it("minimizes the native window", async () => {
    await minimizeCurrentWindow();

    expect(mockMinimize).toHaveBeenCalledTimes(1);
  });

  it("toggles maximize state for the native window", async () => {
    await toggleCurrentWindowMaximize();

    expect(mockToggleMaximize).toHaveBeenCalledTimes(1);
  });

  it("falls back to closing the native window when no editor tab is active", async () => {
    mockFileState.activeTabId = null;

    await requestWindowClose();

    expect(mockClose).toHaveBeenCalledTimes(1);
    expect(mockRequestCloseTab).not.toHaveBeenCalled();
  });
});

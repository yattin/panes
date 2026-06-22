import { create } from "zustand";
import {
  applyAppTheme,
  DEFAULT_APP_THEME,
  type AppTheme,
} from "../domain/appTheme";
import { COMMAND_PALETTE_DEFAULT_LAUNCH } from "../domain/commandPalette";
import {
  enterFocusMode,
  leaveFocusMode,
  toggleFocusModeState,
} from "../domain/focusMode";
import type { UiState } from "../domain/uiState";
import { getShellUiGateway } from "./shellUiGateway";

export const useUiStore = create<UiState>((set) => ({
  showSidebar: true,
  sidebarPinned: true,
  showGitPanel: true,
  gitPanelPinned: true,
  showExplorer: true,
  theme: DEFAULT_APP_THEME,
  focusMode: false,
  focusModeSnapshot: null,
  commandPaletteOpen: false,
  commandPaletteLaunch: COMMAND_PALETTE_DEFAULT_LAUNCH,
  activeView: "chat",
  settingsWorkspaceId: null,
  messageFocusTarget: null,
  openCommandPalette: (launch) =>
    set({
      commandPaletteOpen: true,
      commandPaletteLaunch: {
        ...COMMAND_PALETTE_DEFAULT_LAUNCH,
        ...launch,
      },
    }),
  closeCommandPalette: () =>
    set({
      commandPaletteOpen: false,
      commandPaletteLaunch: COMMAND_PALETTE_DEFAULT_LAUNCH,
    }),
  toggleSidebar: () => set((state) => ({ showSidebar: !state.showSidebar })),
  toggleSidebarPin: () =>
    set((state) => {
      const next = !state.sidebarPinned;
      getShellUiGateway().writeSidebarPinnedPreference(next);
      return { sidebarPinned: next, showSidebar: true };
    }),
  setSidebarPinned: (pinned) => {
    getShellUiGateway().writeSidebarPinnedPreference(pinned);
    set({ sidebarPinned: pinned, showSidebar: true });
  },
  toggleGitPanel: () => set((state) => ({ showGitPanel: !state.showGitPanel })),
  toggleGitPanelPin: () =>
    set((state) => {
      const next = !state.gitPanelPinned;
      getShellUiGateway().writeGitPanelPinnedPreference(next);
      return { gitPanelPinned: next, showGitPanel: true };
    }),
  setGitPanelPinned: (pinned) => {
    getShellUiGateway().writeGitPanelPinnedPreference(pinned);
    set({ gitPanelPinned: pinned, showGitPanel: true });
  },
  toggleExplorer: () =>
    set((state) => {
      const next = !state.showExplorer;
      getShellUiGateway().writeExplorerOpenPreference(next);
      return { showExplorer: next };
    }),
  setExplorerOpen: (open) => {
    getShellUiGateway().writeExplorerOpenPreference(open);
    set({ showExplorer: open });
  },
  setTheme: async (theme: AppTheme) => {
    const gateway = getShellUiGateway();
    const previousTheme = useUiStore.getState().theme;
    applyAppTheme(theme);
    gateway.writeCachedAppTheme(theme);
    set({ theme });

    try {
      const persistedTheme = await gateway.setPersistedAppTheme(theme);
      applyAppTheme(persistedTheme);
      gateway.writeCachedAppTheme(persistedTheme);
      set({ theme: persistedTheme });
      return persistedTheme;
    } catch {
      applyAppTheme(previousTheme);
      gateway.writeCachedAppTheme(previousTheme);
      set({ theme: previousTheme });
      return null;
    }
  },
  setFocusMode: (enabled) =>
    set((state) => (enabled ? enterFocusMode(state) : leaveFocusMode(state))),
  toggleFocusMode: () => set((state) => toggleFocusModeState(state)),
  setActiveView: (view) => {
    set({ activeView: view });
    if (view === "harnesses") {
      void import("../../harnesses/application/harnessStore").then(({ useHarnessStore }) => {
        void useHarnessStore.getState().scan();
      });
    }
  },
  openWorkspaceSettings: (workspaceId) => {
    set({ activeView: "workspace-settings", settingsWorkspaceId: workspaceId });
  },
  setMessageFocusTarget: (target) =>
    set({
      messageFocusTarget: {
        ...target,
        requestedAt: getShellUiGateway().now(),
      },
    }),
  clearMessageFocusTarget: () => set({ messageFocusTarget: null }),
}));

export function hydrateShellUiPreferences(): void {
  const gateway = getShellUiGateway();
  const savedPinned = gateway.readSidebarPinnedPreference();
  const savedGitPanelPinned = gateway.readGitPanelPinnedPreference();
  const savedExplorerOpen = gateway.readExplorerOpenPreference();
  const cachedTheme = gateway.readCachedAppTheme();
  const initialTheme = cachedTheme ?? DEFAULT_APP_THEME;
  applyAppTheme(initialTheme);

  useUiStore.setState({
    sidebarPinned: savedPinned !== null ? savedPinned : true,
    gitPanelPinned: savedGitPanelPinned !== null ? savedGitPanelPinned : true,
    showExplorer: savedExplorerOpen !== null ? savedExplorerOpen : true,
    theme: initialTheme,
  });

  void gateway.getPersistedAppTheme()
    .then((persistedTheme) => {
      applyAppTheme(persistedTheme);
      gateway.writeCachedAppTheme(persistedTheme);
      useUiStore.setState({ theme: persistedTheme });
    })
    .catch(() => undefined);
}

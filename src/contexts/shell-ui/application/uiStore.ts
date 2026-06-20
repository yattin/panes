import { create } from "zustand";
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

  useUiStore.setState({
    sidebarPinned: savedPinned !== null ? savedPinned : true,
    gitPanelPinned: savedGitPanelPinned !== null ? savedGitPanelPinned : true,
    showExplorer: savedExplorerOpen !== null ? savedExplorerOpen : true,
  });
}

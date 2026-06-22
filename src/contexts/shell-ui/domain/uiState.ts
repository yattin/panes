import type { AppTheme } from "./appTheme";

export type CommandPaletteVariant = "general" | "search";

export type CommandPaletteSearchScope = "all" | "messages" | "files" | "threads";

export interface CommandPaletteLaunchState {
  variant: CommandPaletteVariant;
  initialQuery: string;
  searchScope: CommandPaletteSearchScope;
}

export interface MessageFocusTarget {
  threadId: string;
  messageId: string;
  requestedAt: number;
}

export interface FocusModeSnapshot {
  showSidebar: boolean;
  showGitPanel: boolean;
}

export type ActiveView = "chat" | "harnesses" | "workspace-settings";

export interface UiState {
  showSidebar: boolean;
  sidebarPinned: boolean;
  showGitPanel: boolean;
  gitPanelPinned: boolean;
  showExplorer: boolean;
  theme: AppTheme;
  focusMode: boolean;
  focusModeSnapshot: FocusModeSnapshot | null;
  activeView: ActiveView;
  settingsWorkspaceId: string | null;
  commandPaletteOpen: boolean;
  commandPaletteLaunch: CommandPaletteLaunchState;
  messageFocusTarget: MessageFocusTarget | null;
  openCommandPalette: (launch?: Partial<CommandPaletteLaunchState>) => void;
  closeCommandPalette: () => void;
  toggleSidebar: () => void;
  toggleSidebarPin: () => void;
  setSidebarPinned: (pinned: boolean) => void;
  toggleGitPanel: () => void;
  toggleGitPanelPin: () => void;
  setGitPanelPinned: (pinned: boolean) => void;
  toggleExplorer: () => void;
  setExplorerOpen: (open: boolean) => void;
  setTheme: (theme: AppTheme) => Promise<AppTheme | null>;
  setFocusMode: (enabled: boolean) => void;
  toggleFocusMode: () => void;
  setActiveView: (view: ActiveView) => void;
  openWorkspaceSettings: (workspaceId: string) => void;
  setMessageFocusTarget: (target: { threadId: string; messageId: string }) => void;
  clearMessageFocusTarget: () => void;
}

export type ShellVisibilityState = Pick<
  UiState,
  "focusMode" | "focusModeSnapshot" | "showGitPanel" | "showSidebar"
>;

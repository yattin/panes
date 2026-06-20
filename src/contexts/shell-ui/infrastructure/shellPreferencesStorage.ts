const SIDEBAR_PINNED_KEY = "panes:sidebarPinned";
const GIT_PANEL_PINNED_KEY = "panes:gitPanelPinned";
const EXPLORER_OPEN_KEY = "panes:explorerOpen";

function readBooleanPreference(key: string): boolean | null {
  try {
    const saved = localStorage.getItem(key);
    return saved === null ? null : saved === "true";
  } catch {
    return null;
  }
}

function writeBooleanPreference(key: string, value: boolean) {
  try {
    localStorage.setItem(key, String(value));
  } catch {
    // Ignore storage failures in non-browser/test environments.
  }
}

export function readSidebarPinnedPreference(): boolean | null {
  return readBooleanPreference(SIDEBAR_PINNED_KEY);
}

export function writeSidebarPinnedPreference(pinned: boolean) {
  writeBooleanPreference(SIDEBAR_PINNED_KEY, pinned);
}

export function readGitPanelPinnedPreference(): boolean | null {
  return readBooleanPreference(GIT_PANEL_PINNED_KEY);
}

export function writeGitPanelPinnedPreference(pinned: boolean) {
  writeBooleanPreference(GIT_PANEL_PINNED_KEY, pinned);
}

export function readExplorerOpenPreference(): boolean | null {
  return readBooleanPreference(EXPLORER_OPEN_KEY);
}

export function writeExplorerOpenPreference(open: boolean) {
  writeBooleanPreference(EXPLORER_OPEN_KEY, open);
}

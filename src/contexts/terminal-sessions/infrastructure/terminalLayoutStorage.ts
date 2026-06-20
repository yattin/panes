import type { LayoutMode } from "../domain/terminalLayout";

const LAYOUT_MODE_STORAGE_KEY = (workspaceId: string) => `panes:layoutMode:${workspaceId}`;

export function readStoredLayoutMode(workspaceId: string): LayoutMode {
  try {
    const value = localStorage.getItem(LAYOUT_MODE_STORAGE_KEY(workspaceId));
    if (value === "terminal" || value === "split" || value === "editor") {
      return value;
    }
  } catch {
    // localStorage unavailable; fall back to chat layout.
  }
  return "chat";
}

export function writeStoredLayoutMode(workspaceId: string, mode: LayoutMode): void {
  try {
    localStorage.setItem(LAYOUT_MODE_STORAGE_KEY(workspaceId), mode);
  } catch {
    // localStorage unavailable or full; ignore persistence failure.
  }
}

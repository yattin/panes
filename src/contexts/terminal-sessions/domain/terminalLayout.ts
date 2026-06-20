export type LayoutMode = "chat" | "terminal" | "split" | "editor";

export const DEFAULT_PANEL_SIZE = 32;

const MIN_PANEL_SIZE = 15;
const MAX_PANEL_SIZE = 72;

export function clampPanelSize(
  size: number,
  fallback: number = DEFAULT_PANEL_SIZE,
): number {
  const candidate = Number.isFinite(size) ? size : fallback;
  return Math.max(MIN_PANEL_SIZE, Math.min(MAX_PANEL_SIZE, candidate));
}

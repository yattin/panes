import type { WorkspacePaneLegacyMode } from "./workspacePaneLayout";

export function resolveNewThreadTargetLayoutMode(
  currentLayoutMode: WorkspacePaneLegacyMode | null | undefined,
): WorkspacePaneLegacyMode {
  if (currentLayoutMode === "terminal" || currentLayoutMode === "split") {
    return "split";
  }
  return "chat";
}

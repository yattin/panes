import {
  sanitizePersistedWorkspacePaneLayout,
  type WorkspacePaneLayout,
} from "../domain/workspacePaneLayout";
import type { WorkspacePaneGateway } from "../application/workspacePaneGateway";
import { createWorkspacePaneId } from "./workspacePaneIdGenerator";

const STORAGE_KEY = (workspaceId: string) => `panes:workspacePaneLayout:${workspaceId}`;

export function readWorkspacePaneLayout(workspaceId: string): WorkspacePaneLayout | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY(workspaceId));
    if (!raw) {
      return null;
    }
    return sanitizePersistedWorkspacePaneLayout(JSON.parse(raw));
  } catch {
    return null;
  }
}

export function persistWorkspacePaneLayout(
  workspaceId: string,
  layout: WorkspacePaneLayout,
): void {
  try {
    localStorage.setItem(STORAGE_KEY(workspaceId), JSON.stringify(layout));
  } catch {
    // Storage is best-effort in tests and restricted browser contexts.
  }
}

export const workspacePaneGateway: WorkspacePaneGateway = {
  createId: createWorkspacePaneId,
  persistLayout: persistWorkspacePaneLayout,
  readLayout: readWorkspacePaneLayout,
};

import { create } from "zustand";
import {
  activateWorkspacePaneSurfaceInLeaf,
  closeWorkspacePaneLeaf,
  closeWorkspacePaneTab,
  defaultLayoutForLegacyMode,
  focusWorkspacePaneLeaf,
  setWorkspacePaneActiveTab,
  showWorkspacePaneSurface,
  splitWorkspacePaneLeaf,
  updateWorkspacePaneRatio,
  type WorkspacePaneLayout,
  type WorkspacePaneLegacyMode,
  type WorkspacePaneSplitDirection,
  type WorkspacePaneSurfaceKind,
} from "../domain/workspacePaneLayout";
import { getWorkspacePaneGateway } from "./workspacePaneGateway";

interface WorkspacePaneState {
  workspaces: Record<string, WorkspacePaneLayout>;
  ensureWorkspace: (workspaceId: string, legacyMode?: WorkspacePaneLegacyMode) => void;
  applyLegacyLayoutMode: (workspaceId: string, mode: WorkspacePaneLegacyMode) => void;
  focusLeaf: (workspaceId: string, leafId: string) => void;
  setActiveTab: (workspaceId: string, leafId: string, tabId: string) => void;
  activateSurfaceInLeaf: (
    workspaceId: string,
    leafId: string,
    kind: WorkspacePaneSurfaceKind,
  ) => void;
  activateFocusedSurface: (workspaceId: string, kind: WorkspacePaneSurfaceKind) => void;
  showSurface: (
    workspaceId: string,
    kind: WorkspacePaneSurfaceKind,
    leafId?: string | null,
  ) => void;
  showSingleSurface: (workspaceId: string, kind: WorkspacePaneSurfaceKind) => void;
  splitLeaf: (
    workspaceId: string,
    leafId: string,
    direction: WorkspacePaneSplitDirection,
    kind?: WorkspacePaneSurfaceKind | null,
    position?: "before" | "after",
  ) => void;
  splitFocusedLeaf: (
    workspaceId: string,
    direction: WorkspacePaneSplitDirection,
    kind?: WorkspacePaneSurfaceKind | null,
    position?: "before" | "after",
  ) => void;
  closeLeaf: (workspaceId: string, leafId: string) => void;
  closeTab: (workspaceId: string, leafId: string, tabId: string) => void;
  updateRatio: (workspaceId: string, containerId: string, ratio: number) => void;
}

function updateWorkspace(
  state: WorkspacePaneState,
  workspaceId: string,
  updater: (layout: WorkspacePaneLayout) => WorkspacePaneLayout,
): Record<string, WorkspacePaneLayout> {
  const current =
    state.workspaces[workspaceId] ??
    getWorkspacePaneGateway().readLayout(workspaceId) ??
    defaultLayoutForLegacyMode("chat", getWorkspacePaneGateway().createId);
  const next = updater(current);
  getWorkspacePaneGateway().persistLayout(workspaceId, next);
  return { ...state.workspaces, [workspaceId]: next };
}

export const useWorkspacePaneStore = create<WorkspacePaneState>((set, get) => ({
  workspaces: {},

  ensureWorkspace: (workspaceId, legacyMode = "chat") => {
    set((state) => {
      if (state.workspaces[workspaceId]) {
        return state;
      }
      const layout =
        getWorkspacePaneGateway().readLayout(workspaceId) ??
        defaultLayoutForLegacyMode(legacyMode, getWorkspacePaneGateway().createId);
      getWorkspacePaneGateway().persistLayout(workspaceId, layout);
      return {
        workspaces: { ...state.workspaces, [workspaceId]: layout },
      };
    });
  },

  applyLegacyLayoutMode: (workspaceId, mode) => {
    set((state) => ({
      workspaces: updateWorkspace(state, workspaceId, () =>
        defaultLayoutForLegacyMode(mode, getWorkspacePaneGateway().createId),
      ),
    }));
  },

  focusLeaf: (workspaceId, leafId) => {
    set((state) => ({
      workspaces: updateWorkspace(state, workspaceId, (layout) =>
        focusWorkspacePaneLeaf(layout, leafId),
      ),
    }));
  },

  setActiveTab: (workspaceId, leafId, tabId) => {
    set((state) => ({
      workspaces: updateWorkspace(state, workspaceId, (layout) =>
        setWorkspacePaneActiveTab(layout, leafId, tabId),
      ),
    }));
  },

  activateSurfaceInLeaf: (workspaceId, leafId, kind) => {
    set((state) => ({
      workspaces: updateWorkspace(state, workspaceId, (layout) =>
        activateWorkspacePaneSurfaceInLeaf(
          layout,
          leafId,
          kind,
          getWorkspacePaneGateway().createId,
        ),
      ),
    }));
  },

  activateFocusedSurface: (workspaceId, kind) => {
    const layout =
      get().workspaces[workspaceId] ??
      getWorkspacePaneGateway().readLayout(workspaceId);
    if (!layout) {
      get().ensureWorkspace(workspaceId);
      return;
    }
    get().activateSurfaceInLeaf(workspaceId, layout.focusedLeafId, kind);
  },

  showSurface: (workspaceId, kind, leafId) => {
    set((state) => ({
      workspaces: updateWorkspace(state, workspaceId, (layout) =>
        showWorkspacePaneSurface(
          layout,
          kind,
          getWorkspacePaneGateway().createId,
          leafId,
        ),
      ),
    }));
  },

  showSingleSurface: (workspaceId, kind) => {
    const mode: WorkspacePaneLegacyMode =
      kind === "terminal" ? "terminal" : kind === "editor" ? "editor" : "chat";
    get().applyLegacyLayoutMode(workspaceId, mode);
  },

  splitLeaf: (workspaceId, leafId, direction, kind, position = "after") => {
    set((state) => ({
      workspaces: updateWorkspace(state, workspaceId, (layout) =>
        splitWorkspacePaneLeaf(
          layout,
          leafId,
          direction,
          getWorkspacePaneGateway().createId,
          kind,
          position,
        ),
      ),
    }));
  },

  splitFocusedLeaf: (workspaceId, direction, kind, position = "after") => {
    const layout =
      get().workspaces[workspaceId] ??
      getWorkspacePaneGateway().readLayout(workspaceId);
    if (!layout) {
      get().ensureWorkspace(workspaceId);
      return;
    }
    get().splitLeaf(workspaceId, layout.focusedLeafId, direction, kind, position);
  },

  closeLeaf: (workspaceId, leafId) => {
    set((state) => ({
      workspaces: updateWorkspace(state, workspaceId, (layout) =>
        closeWorkspacePaneLeaf(layout, leafId, getWorkspacePaneGateway().createId),
      ),
    }));
  },

  closeTab: (workspaceId, leafId, tabId) => {
    set((state) => ({
      workspaces: updateWorkspace(state, workspaceId, (layout) =>
        closeWorkspacePaneTab(layout, leafId, tabId),
      ),
    }));
  },

  updateRatio: (workspaceId, containerId, ratio) => {
    set((state) => ({
      workspaces: updateWorkspace(state, workspaceId, (layout) =>
        updateWorkspacePaneRatio(layout, containerId, ratio),
      ),
    }));
  },
}));

export type {
  WorkspacePaneLayout,
  WorkspacePaneLeaf,
  WorkspacePaneLegacyMode,
  WorkspacePaneNode,
  WorkspacePaneSplit,
  WorkspacePaneSplitDirection,
  WorkspacePaneSurfaceKind,
  WorkspacePaneTab,
} from "../domain/workspacePaneLayout";

export {
  collectWorkspacePaneLeaves,
  getWorkspacePaneActiveTab,
  SURFACE_ORDER,
} from "../domain/workspacePaneLayout";

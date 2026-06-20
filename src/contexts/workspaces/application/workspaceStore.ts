import { create } from "zustand";
import type { CueLightProjectBinding, Repo, TrustLevel, Workspace } from "../../../types";
import { useGitStore } from "../../git/application/gitStore";
import { useTerminalStore } from "../../terminal-sessions/application/terminalStore";
import {
  resolveActiveRepoIdFromSelection,
  resolveStartupWorkspaceId,
} from "../domain/workspaceSelection";
import { getWorkspaceGateway } from "./workspaceGateway";

interface SetActiveRepoOptions {
  remember?: boolean;
}

export interface WorkspaceState {
  workspaces: Workspace[];
  archivedWorkspaces: Workspace[];
  activeWorkspaceId: string | null;
  repos: Repo[];
  activeRepoId: string | null;
  reposLoading: boolean;
  loading: boolean;
  error?: string;
  loadWorkspaces: () => Promise<void>;
  refreshArchivedWorkspaces: () => Promise<void>;
  openWorkspace: (path: string, scanDepth?: number) => Promise<Workspace | null>;
  removeWorkspace: (workspaceId: string) => Promise<void>;
  restoreWorkspace: (workspaceId: string) => Promise<void>;
  loadRepos: (workspaceId: string) => Promise<void>;
  setActiveWorkspace: (workspaceId: string) => Promise<void>;
  setActiveRepo: (repoId: string | null, options?: SetActiveRepoOptions) => void;
  setRepoGitActive: (repoId: string, isActive: boolean) => Promise<void>;
  setWorkspaceGitActiveRepos: (workspaceId: string, repoIds: string[]) => Promise<void>;
  hasWorkspaceGitSelection: (workspaceId: string) => Promise<boolean>;
  setRepoTrustLevel: (repoId: string, trustLevel: TrustLevel) => Promise<void>;
  setAllReposTrustLevel: (trustLevel: TrustLevel) => Promise<void>;
  rescanWorkspace: (workspaceId: string, scanDepth?: number) => Promise<Workspace | null>;
  bindCueLight: (workspaceId: string, binding: {
    projectId: string;
    projectName: string;
  }) => Promise<void>;
  unbindCueLight: (workspaceId: string) => Promise<void>;
  getCueLightBinding: (workspaceId: string) => CueLightProjectBinding | null | undefined;
}

let reposLoadSeq = 0;

export function resolveActiveRepoId(
  workspaceId: string,
  repos: Repo[],
  currentActiveRepoId: string | null,
): string | null {
  return resolveActiveRepoIdFromSelection(
    workspaceId,
    repos,
    currentActiveRepoId,
    getWorkspaceGateway().readLastRepoByWorkspace(),
  );
}

export const useWorkspaceStore = create<WorkspaceState>((set, get) => ({
  workspaces: [],
  archivedWorkspaces: [],
  activeWorkspaceId: null,
  repos: [],
  activeRepoId: null,
  reposLoading: false,
  loading: false,
  loadWorkspaces: async () => {
    set({ loading: true, error: undefined });
    try {
      const workspaces = await getWorkspaceGateway().listWorkspaces();
      const savedId = getWorkspaceGateway().readLastWorkspaceId();
      const activeWorkspaceId = resolveStartupWorkspaceId(workspaces, savedId);
      set({ workspaces, activeWorkspaceId, loading: false });
      if (activeWorkspaceId) {
        await useTerminalStore.getState().prepareWorkspaceActivation(activeWorkspaceId);
        useGitStore.getState().loadDraftsForWorkspace(activeWorkspaceId);
        await get().loadRepos(activeWorkspaceId);
      }
      await get().refreshArchivedWorkspaces();
    } catch (error) {
      set({ loading: false, error: String(error) });
    }
  },
  refreshArchivedWorkspaces: async () => {
    try {
      const archivedWorkspaces = await getWorkspaceGateway().listArchivedWorkspaces();
      set({ archivedWorkspaces });
    } catch (error) {
      set({ error: String(error) });
    }
  },
  openWorkspace: async (path, scanDepth) => {
    set({ loading: true, error: undefined });
    try {
      const workspace = await getWorkspaceGateway().openWorkspace(path, scanDepth);
      const current = get().workspaces.filter((item) => item.id !== workspace.id);
      const workspaces = [workspace, ...current];
      set((state) => ({
        workspaces,
        archivedWorkspaces: state.archivedWorkspaces.filter((item) => item.id !== workspace.id),
        activeWorkspaceId: workspace.id,
        loading: false,
      }));
      getWorkspaceGateway().writeLastWorkspaceId(workspace.id);
      await useTerminalStore.getState().prepareWorkspaceActivation(workspace.id);
      await get().loadRepos(workspace.id);
      return workspace;
    } catch (error) {
      set({ loading: false, error: String(error) });
      return null;
    }
  },
  removeWorkspace: async (workspaceId) => {
    set({ loading: true, error: undefined });
    try {
      await getWorkspaceGateway().archiveWorkspace(workspaceId);
      const wasActiveWorkspace = get().activeWorkspaceId === workspaceId;
      const removed = get().workspaces.find((workspace) => workspace.id === workspaceId) ?? null;
      const remaining = get().workspaces.filter((workspace) => workspace.id !== workspaceId);
      const nextActive =
        wasActiveWorkspace
          ? remaining[0]?.id ?? null
          : get().activeWorkspaceId;

      set((state) => ({
        workspaces: remaining,
        archivedWorkspaces: removed
          ? [
              removed,
              ...state.archivedWorkspaces.filter((workspace) => workspace.id !== workspaceId),
            ]
          : state.archivedWorkspaces,
        activeWorkspaceId: nextActive,
        loading: false,
      }));

      if (nextActive) {
        if (wasActiveWorkspace) {
          await useTerminalStore.getState().prepareWorkspaceActivation(nextActive);
        }
        await get().loadRepos(nextActive);
      } else {
        set({ repos: [], activeRepoId: null, reposLoading: false });
      }
    } catch (error) {
      set({ loading: false, error: String(error) });
    }
  },
  restoreWorkspace: async (workspaceId) => {
    set({ loading: true, error: undefined });
    try {
      const restored = await getWorkspaceGateway().restoreWorkspace(workspaceId);
      set((state) => {
        const workspaces = [
          restored,
          ...state.workspaces.filter((workspace) => workspace.id !== workspaceId),
        ];
        const nextActiveWorkspaceId = state.activeWorkspaceId ?? restored.id;
        return {
          workspaces,
          archivedWorkspaces: state.archivedWorkspaces.filter(
            (workspace) => workspace.id !== workspaceId,
          ),
          activeWorkspaceId: nextActiveWorkspaceId,
          loading: false,
        };
      });

      if (!get().activeWorkspaceId || get().activeWorkspaceId === restored.id) {
        await useTerminalStore.getState().prepareWorkspaceActivation(restored.id);
        await get().loadRepos(restored.id);
      }
    } catch (error) {
      set({ loading: false, error: String(error) });
    }
  },
  loadRepos: async (workspaceId) => {
    const requestSeq = ++reposLoadSeq;
    set({ reposLoading: true });
    try {
      const repos = await getWorkspaceGateway().getRepos(workspaceId);
      if (requestSeq !== reposLoadSeq) {
        return;
      }
      if (get().activeWorkspaceId !== workspaceId) {
        set({ reposLoading: false });
        return;
      }
      const fallbackActiveRepoId = resolveActiveRepoId(
        workspaceId,
        repos,
        get().activeRepoId,
      );
      if (fallbackActiveRepoId) {
        getWorkspaceGateway().rememberLastRepo(workspaceId, fallbackActiveRepoId);
      }
      set({
        repos,
        activeRepoId: fallbackActiveRepoId,
        reposLoading: false,
      });
    } catch (error) {
      if (requestSeq !== reposLoadSeq) {
        return;
      }
      if (get().activeWorkspaceId !== workspaceId) {
        set({ reposLoading: false });
        return;
      }
      set({ error: String(error), reposLoading: false });
    }
  },
  setActiveWorkspace: async (workspaceId) => {
    const prevWorkspaceId = get().activeWorkspaceId;
    if (prevWorkspaceId) {
      useGitStore.getState().flushDrafts(prevWorkspaceId);
    }
    getWorkspaceGateway().writeLastWorkspaceId(workspaceId);
    set({ activeWorkspaceId: workspaceId, activeRepoId: null, repos: [], error: undefined });
    await useTerminalStore.getState().prepareWorkspaceActivation(workspaceId);
    await get().loadRepos(workspaceId);
    useGitStore.getState().loadDraftsForWorkspace(workspaceId);
  },
  setActiveRepo: (repoId, options) => {
    set({ activeRepoId: repoId });

    if (options?.remember === false || !repoId) {
      return;
    }

    const workspaceId = get().activeWorkspaceId;
    if (!workspaceId) {
      return;
    }

    const repoExistsInWorkspace = get().repos.some(
      (repo) => repo.id === repoId && repo.workspaceId === workspaceId,
    );
    if (!repoExistsInWorkspace) {
      return;
    }

    getWorkspaceGateway().rememberLastRepo(workspaceId, repoId);
  },
  setRepoGitActive: async (repoId, isActive) => {
    try {
      await getWorkspaceGateway().setRepoGitActive(repoId, isActive);
      set((state) => ({
        repos: state.repos.map((repo) =>
          repo.id === repoId
            ? {
                ...repo,
                isActive,
              }
            : repo,
        ),
      }));
    } catch (error) {
      set({ error: String(error) });
      throw error;
    }
  },
  setWorkspaceGitActiveRepos: async (workspaceId, repoIds) => {
    try {
      await getWorkspaceGateway().setWorkspaceGitActiveRepos(workspaceId, repoIds);
      set((state) => {
        const selected = new Set(repoIds);
        return {
          repos: state.repos.map((repo) =>
            repo.workspaceId === workspaceId
              ? {
                  ...repo,
                  isActive: selected.has(repo.id),
                }
              : repo,
          ),
        };
      });
    } catch (error) {
      set({ error: String(error) });
      throw error;
    }
  },
  hasWorkspaceGitSelection: async (workspaceId) => {
    const status = await getWorkspaceGateway().hasWorkspaceGitSelection(workspaceId);
    return status.configured;
  },
  setRepoTrustLevel: async (repoId, trustLevel) => {
    try {
      await getWorkspaceGateway().setRepoTrustLevel(repoId, trustLevel);
      set((state) => ({
        repos: state.repos.map((repo) =>
          repo.id === repoId
            ? {
                ...repo,
                trustLevel
              }
            : repo
        )
      }));
    } catch (error) {
      set({ error: String(error) });
    }
  },
  rescanWorkspace: async (workspaceId, scanDepth) => {
    const workspace = get().workspaces.find((w) => w.id === workspaceId);
    if (!workspace) return null;

    try {
      const updatedWorkspace = await getWorkspaceGateway().openWorkspace(
        workspace.rootPath,
        scanDepth ?? workspace.scanDepth,
      );
      set((state) => ({
        workspaces: [
          updatedWorkspace,
          ...state.workspaces.filter((item) => item.id !== updatedWorkspace.id),
        ],
        archivedWorkspaces: state.archivedWorkspaces.filter(
          (item) => item.id !== updatedWorkspace.id,
        ),
      }));

      if (get().activeWorkspaceId === workspaceId) {
        await get().loadRepos(workspaceId);
      }

      return updatedWorkspace;
    } catch (error) {
      set({ error: String(error) });
      throw error;
    }
  },
  setAllReposTrustLevel: async (trustLevel) => {
    const repos = get().repos;
    if (!repos.length) {
      return;
    }

    try {
      await Promise.all(
        repos.map((repo) => getWorkspaceGateway().setRepoTrustLevel(repo.id, trustLevel))
      );
      set((state) => ({
        repos: state.repos.map((repo) => ({
          ...repo,
          trustLevel
        }))
      }));
    } catch (error) {
      set({ error: String(error) });
    }
  },
  bindCueLight: async (workspaceId, binding) => {
    await getWorkspaceGateway().bindCueLightProject(workspaceId, binding);
    const fresh = await getWorkspaceGateway().getCueLightBinding(workspaceId);
    set((state) => ({
      workspaces: state.workspaces.map((w) =>
        w.id === workspaceId ? { ...w, cueLightBinding: fresh } : w
      ),
    }));
  },
  unbindCueLight: async (workspaceId) => {
    await getWorkspaceGateway().unbindCueLightProject(workspaceId);
    set((state) => ({
      workspaces: state.workspaces.map((w) =>
        w.id === workspaceId ? { ...w, cueLightBinding: null } : w
      ),
    }));
  },
  getCueLightBinding: (workspaceId) => {
    return get().workspaces.find((w) => w.id === workspaceId)?.cueLightBinding;
  },
}));

import { create } from "zustand";
import { t } from "../../../i18n";
import { useHarnessStore } from "../../harnesses/application/harnessStore";
import { toast } from "../../shell-ui/application/toastStore";
import { resolveActiveRepoId, useWorkspaceStore } from "../../workspaces/application/workspaceStore";
import {
  nextTerminalNumber,
  reorderTerminalGroups,
} from "../domain/terminalGroups";
import {
  buildGridSplitTree,
  collectSessionIds,
  removeLeafFromTree,
  replaceLeafInTree,
  updateRatioInTree,
} from "../domain/terminalSplitTree";
import {
  clearNotificationRecord,
  hasNotificationHydrationTouchChange,
  indexNotificationsBySession,
  pruneNotificationsByLiveSessions,
  resolveHydratedNotifications,
  withNotificationHydrationTouch,
} from "../domain/terminalNotifications";
import {
  clampPanelSize,
  DEFAULT_PANEL_SIZE,
  type LayoutMode,
} from "../domain/terminalLayout";
import {
  buildStartupWorktreeBranch,
  buildStartupWorktreePath,
  getGroupWorktreesFromMeta,
  inferWorktreeConfig,
  isAbsolutePath,
  joinPath,
  materializeStartupSplitNode,
  pendingStartupPresetFor,
  resolveSessionStartupCwd,
  serializeRuntimeSplitNode,
  summarizeWarnings,
} from "../domain/terminalStartup";
import { getTerminalSessionGateway } from "./terminalSessionGateway";
import type {
  SplitDirection,
  SplitNode,
  TerminalGroup,
  TerminalNotification,
  TerminalSession,
  TerminalSessionRuntimeMeta,
  WorktreeSessionInfo,
  WorkspaceStartupPreset,
  WorkspaceStartupWorktreeConfig,
} from "../../../types";

const DEFAULT_COLS = 120;
const DEFAULT_ROWS = 36;

function findGroupForSession(groups: TerminalGroup[], sessionId: string): TerminalGroup | null {
  for (const group of groups) {
    if (collectSessionIds(group.root).includes(sessionId)) return group;
  }
  return null;
}

function defaultSessionMeta(meta?: TerminalSessionRuntimeMeta): TerminalSessionRuntimeMeta {
  return {
    harnessId: meta?.harnessId ?? null,
    harnessName: meta?.harnessName ?? null,
    autoDetectedHarness: meta?.autoDetectedHarness ?? false,
    launchHarnessOnCreate: meta?.launchHarnessOnCreate ?? false,
    worktree: meta?.worktree ?? null,
  };
}

export function getSessionMeta(group: TerminalGroup, sessionId: string): TerminalSessionRuntimeMeta {
  return defaultSessionMeta(group.sessionMeta?.[sessionId]);
}

function setSessionMeta(group: TerminalGroup, sessionId: string, meta: TerminalSessionRuntimeMeta): TerminalGroup {
  return {
    ...group,
    sessionMeta: {
      ...(group.sessionMeta ?? {}),
      [sessionId]: defaultSessionMeta(meta),
    },
  };
}

export { getGroupWorktreesFromMeta };

export function getGroupDisplayHarness(group: TerminalGroup): {
  harnessId: string | null;
  harnessName: string | null;
  homogeneous: boolean;
} {
  const sessionIds = collectSessionIds(group.root);
  if (sessionIds.length === 0) {
    return { harnessId: null, harnessName: null, homogeneous: false };
  }

  let harnessId: string | null = null;
  let harnessName: string | null = null;
  for (const sessionId of sessionIds) {
    const meta = group.sessionMeta?.[sessionId];
    const nextHarnessId = meta?.harnessId ?? null;
    if (!nextHarnessId) {
      return { harnessId: null, harnessName: null, homogeneous: false };
    }
    if (harnessId === null) {
      harnessId = nextHarnessId;
      harnessName = meta?.harnessName ?? null;
      continue;
    }
    if (harnessId !== nextHarnessId) {
      return { harnessId: null, harnessName: null, homogeneous: false };
    }
  }

  return { harnessId, harnessName, homogeneous: harnessId !== null };
}

function makeLeafGroup(
  sessionId: string,
  name: string,
  sessionMeta?: TerminalSessionRuntimeMeta,
): TerminalGroup {
  return {
    id: getTerminalSessionGateway().createTerminalGroupId(),
    root: { type: "leaf", sessionId },
    name,
    sessionMeta: {
      [sessionId]: defaultSessionMeta(sessionMeta),
    },
    worktreeConfig: null,
  };
}

export { buildGridSplitTree, collectSessionIds, nextTerminalNumber };
export type { LayoutMode };

function nextHarnessName(baseName: string, harnessId: string, excludeGroupId: string, groups: TerminalGroup[]): string {
  const used = new Set<number>();
  for (const g of groups) {
    const displayHarness = getGroupDisplayHarness(g);
    if (g.id === excludeGroupId || displayHarness.harnessId !== harnessId) continue;
    if (g.name === baseName) { used.add(1); continue; }
    const match = new RegExp(`^${baseName.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")} (\\d+)$`).exec(g.name);
    if (match) used.add(Number(match[1]));
  }
  if (used.size === 0) return baseName;
  let n = 1;
  while (used.has(n)) n++;
  return n === 1 ? baseName : `${baseName} ${n}`;
}

function nextFocusedSessionId(
  groups: TerminalGroup[],
  preferGroupId: string | null,
  previousId: string | null,
): string | null {
  if (groups.length === 0) return null;
  const target =
    (preferGroupId ? groups.find((g) => g.id === preferGroupId) : null) ??
    groups[groups.length - 1];
  const ids = collectSessionIds(target.root);
  if (previousId && ids.includes(previousId)) return previousId;
  return ids[ids.length - 1] ?? null;
}

function pruneDeadSessionsFromGroup(
  group: TerminalGroup,
  liveIds: Set<string>,
): TerminalGroup | null {
  let root: SplitNode | null = group.root;
  for (const sessionId of collectSessionIds(group.root)) {
    if (!liveIds.has(sessionId)) {
      root = root ? removeLeafFromTree(root, sessionId) : null;
    }
  }

  if (!root) {
    return null;
  }

  const sessionIds = collectSessionIds(root);
  const sessionMeta = Object.fromEntries(
    sessionIds.map((sessionId) => [sessionId, getSessionMeta(group, sessionId)]),
  );
  return { ...group, root, sessionMeta };
}

function pruneSessionFromGroup(group: TerminalGroup, sessionId: string): TerminalGroup | null {
  const root = removeLeafFromTree(group.root, sessionId);
  if (!root) {
    return null;
  }

  const nextSessionIds = collectSessionIds(root);
  const sessionMeta = Object.fromEntries(
    nextSessionIds.map((id) => [id, getSessionMeta(group, id)]),
  );
  return { ...group, root, sessionMeta };
}

function isTerminalGroup(value: TerminalGroup | null): value is TerminalGroup {
  return value !== null;
}

// ── State shape ─────────────────────────────────────────────────────

interface WorkspaceTerminalState {
  isOpen: boolean;
  layoutMode: LayoutMode;
  preEditorLayoutMode: LayoutMode;
  panelSize: number;
  sessions: TerminalSession[];
  notificationsBySessionId: Record<string, TerminalNotification>;
  notificationHydrating?: boolean;
  notificationHydrationRequestId?: number;
  notificationTouchedAll?: boolean;
  notificationTouchedSessionIds?: Record<string, true>;
  activeSessionId: string | null;
  groups: TerminalGroup[];
  activeGroupId: string | null;
  focusedSessionId: string | null;
  broadcastGroupId: string | null;
  startupPreset: WorkspaceStartupPreset | null;
  pendingStartupPreset: WorkspaceStartupPreset | null;
  loading: boolean;
  error?: string;
}

interface TerminalState {
  workspaces: Record<string, WorkspaceTerminalState>;
  prepareWorkspaceActivation: (workspaceId: string) => Promise<void>;
  setWorkspaceStartupPresetState: (
    workspaceId: string,
    preset: WorkspaceStartupPreset | null,
  ) => void;
  setWorkspaceStatus: (workspaceId: string, loading: boolean, error?: string) => void;
  openTerminal: (workspaceId: string) => Promise<void>;
  closeTerminal: (workspaceId: string) => Promise<void>;
  toggleTerminal: (workspaceId: string) => Promise<void>;
  setLayoutMode: (workspaceId: string, mode: LayoutMode) => Promise<void>;
  cycleLayoutMode: (workspaceId: string) => Promise<void>;
  runCommandInTerminal: (workspaceId: string, command: string) => Promise<boolean>;
  createSession: (workspaceId: string, cols?: number, rows?: number, harnessId?: string, harnessName?: string) => Promise<string | null>;
  materializeWorkspaceStartupPreset: (
    workspaceId: string,
    preset: WorkspaceStartupPreset,
    cols?: number,
    rows?: number,
  ) => Promise<boolean>;
  serializeWorkspaceRuntimeAsStartupPreset: (workspaceId: string) => WorkspaceStartupPreset | null;
  applyWorkspaceStartupPresetNow: (
    workspaceId: string,
    preset: WorkspaceStartupPreset,
    options?: { removeWorktrees?: boolean },
  ) => Promise<boolean>;
  closeSession: (workspaceId: string, sessionId: string) => Promise<void>;
  setActiveSession: (workspaceId: string, sessionId: string) => void;
  setPanelSize: (workspaceId: string, size: number) => void;
  syncSessions: (workspaceId: string) => Promise<void>;
  hydrateNotifications: (workspaceId: string) => Promise<void>;
  applyNotification: (workspaceId: string, notification: TerminalNotification) => void;
  clearNotificationLocal: (workspaceId: string, sessionId?: string | null) => void;
  clearNotification: (workspaceId: string, sessionId?: string | null) => Promise<void>;
  syncNotificationFocus: (
    workspaceId: string | null,
    sessionId: string | null,
    windowFocused: boolean,
  ) => Promise<void>;
  handleSessionExit: (workspaceId: string, sessionId: string) => void;
  splitSession: (workspaceId: string, sessionId: string, direction: SplitDirection, cols?: number, rows?: number) => Promise<void>;
  setFocusedSession: (workspaceId: string, sessionId: string) => void;
  setActiveGroup: (workspaceId: string, groupId: string) => void;
  updateGroupRatio: (workspaceId: string, groupId: string, containerId: string, ratio: number) => void;
  renameGroup: (workspaceId: string, groupId: string, name: string) => void;
  reorderGroups: (workspaceId: string, fromIndex: number, toIndex: number) => void;
  updateSessionHarness: (
    workspaceId: string,
    sessionId: string,
    harnessId: string | null,
    harnessName: string | null,
    autoDetected: boolean,
  ) => void;
  toggleBroadcast: (workspaceId: string, groupId: string) => void;
  createMultiSessionGroup: (
    workspaceId: string,
    harnesses: Array<{ harnessId: string; name: string }>,
    worktreeConfig?: WorkspaceStartupWorktreeConfig | null,
    cols?: number,
    rows?: number,
  ) => Promise<{ groupId: string; sessionIds: string[] } | null>;
  getGroupWorktrees: (workspaceId: string, groupId: string) => WorktreeSessionInfo[];
  removeGroupWorktrees: (workspaceId: string, worktrees: WorktreeSessionInfo[]) => Promise<void>;
}

function defaultWorkspaceState(): WorkspaceTerminalState {
  return {
    isOpen: false,
    layoutMode: "chat",
    preEditorLayoutMode: "chat",
    panelSize: DEFAULT_PANEL_SIZE,
    sessions: [],
    notificationsBySessionId: {},
    notificationHydrating: false,
    notificationHydrationRequestId: 0,
    notificationTouchedAll: false,
    notificationTouchedSessionIds: {},
    activeSessionId: null,
    groups: [],
    activeGroupId: null,
    focusedSessionId: null,
    broadcastGroupId: null,
    startupPreset: null,
    pendingStartupPreset: null,
    loading: false,
    error: undefined,
  };
}

function mergeWorkspaceState(
  state: TerminalState["workspaces"],
  workspaceId: string,
  next: Partial<WorkspaceTerminalState>,
): TerminalState["workspaces"] {
  const current = state[workspaceId] ?? defaultWorkspaceState();
  return {
    ...state,
    [workspaceId]: {
      ...current,
      ...next,
    },
  };
}

async function removeWorktreesSequential(worktrees: WorktreeSessionInfo[]): Promise<string[]> {
  const failures: string[] = [];
  for (const worktree of worktrees) {
    try {
      await getTerminalSessionGateway().removeGitWorktree(
        worktree.repoPath,
        worktree.worktreePath,
        true,
        worktree.branch,
        true,
      );
    } catch (error) {
      failures.push(`${worktree.branch || worktree.worktreePath}: ${String(error)}`);
    }
  }
  return failures;
}

async function closeSessionsSequential(workspaceId: string, sessionIds: string[]): Promise<void> {
  await Promise.allSettled(
    sessionIds.map((sessionId) =>
      getTerminalSessionGateway().terminalCloseSession(workspaceId, sessionId)
    ),
  );
}

function workspaceRootPath(workspaceId: string): string | null {
  return useWorkspaceStore.getState().workspaces.find((workspace) => workspace.id === workspaceId)?.rootPath ?? null;
}

async function createSessionWithFallback(
  workspaceId: string,
  cols: number,
  rows: number,
  preferredCwd: string | null,
  fallbackLabel: string,
  warnings: string[],
): Promise<TerminalSession> {
  if (preferredCwd) {
    try {
      return await getTerminalSessionGateway().terminalCreateSession(
        workspaceId,
        cols,
        rows,
        preferredCwd,
      );
    } catch (error) {
      warnings.push(`${fallbackLabel} opened in the workspace root because its startup path was invalid.`);
    }
  }
  return getTerminalSessionGateway().terminalCreateSession(workspaceId, cols, rows);
}

export const useTerminalStore = create<TerminalState>((set, get) => ({
  workspaces: {},

  prepareWorkspaceActivation: async (workspaceId) => {
    const fallbackMode = getTerminalSessionGateway().readStoredLayoutMode(workspaceId);
    try {
      const preset = await getTerminalSessionGateway().getWorkspaceStartupPreset(workspaceId);
      const targetMode = preset?.defaultView ?? fallbackMode;
      const nextPendingPreset = pendingStartupPresetFor(preset);
      set((state) => {
        const current = state.workspaces[workspaceId] ?? defaultWorkspaceState();
        const hasLiveSessions = current.sessions.length > 0;
        return {
          workspaces: mergeWorkspaceState(state.workspaces, workspaceId, {
            startupPreset: preset,
            pendingStartupPreset: hasLiveSessions
              ? current.pendingStartupPreset
              : nextPendingPreset,
            layoutMode: hasLiveSessions ? current.layoutMode : targetMode,
            isOpen:
              hasLiveSessions || targetMode === "split" || targetMode === "terminal" || nextPendingPreset
                ? true
                : current.isOpen,
            panelSize: hasLiveSessions
              ? current.panelSize
              : clampPanelSize(preset?.splitPanelSize ?? current.panelSize),
            loading: false,
            error: undefined,
          }),
        };
      });
    } catch (error) {
      const message = String(error);
      toast.warning(t("app:terminal.toasts.invalidStartupPresetIgnored"));
      set((state) => ({
        workspaces: mergeWorkspaceState(state.workspaces, workspaceId, {
          startupPreset: null,
          pendingStartupPreset: null,
          layoutMode: fallbackMode,
          isOpen: fallbackMode === "split" || fallbackMode === "terminal",
          loading: false,
          error: undefined,
        }),
      }));
      console.warn(`Failed to load workspace startup preset for ${workspaceId}:`, message);
    }
  },

  setWorkspaceStartupPresetState: (workspaceId, preset) => {
    set((state) => {
      const current = state.workspaces[workspaceId] ?? defaultWorkspaceState();
      const hasLiveSessions = current.sessions.length > 0;
      return {
        workspaces: mergeWorkspaceState(state.workspaces, workspaceId, {
          startupPreset: preset,
          pendingStartupPreset:
            preset === null
              ? null
              : hasLiveSessions
                ? null
                : pendingStartupPresetFor(preset),
        }),
      };
    });
  },

  setWorkspaceStatus: (workspaceId, loading, error) => {
    set((state) => ({
      workspaces: mergeWorkspaceState(state.workspaces, workspaceId, {
        loading,
        error,
      }),
    }));
  },

  openTerminal: async (workspaceId) => {
    // Only mark the terminal as open. Session creation is deferred to
    // syncSessions which runs after the TerminalPanel mounts and registers
    // its output event listeners, so the initial shell prompt is never lost.
    set((state) => ({
      workspaces: mergeWorkspaceState(state.workspaces, workspaceId, {
        isOpen: true,
        loading: true,
        error: undefined,
      }),
    }));
  },

  closeTerminal: async (workspaceId) => {
    set((state) => ({
      workspaces: mergeWorkspaceState(state.workspaces, workspaceId, {
        loading: true,
        error: undefined,
      }),
    }));
    try {
      await getTerminalSessionGateway().terminalCloseWorkspaceSessions(workspaceId);
      getTerminalSessionGateway().writeStoredLayoutMode(workspaceId, "chat");
      set((state) => ({
        workspaces: mergeWorkspaceState(state.workspaces, workspaceId, {
          isOpen: false,
          layoutMode: "chat",
          sessions: [],
          notificationsBySessionId: {},
          activeSessionId: null,
          groups: [],
          activeGroupId: null,
          focusedSessionId: null,
          broadcastGroupId: null,
          pendingStartupPreset: pendingStartupPresetFor(
            (state.workspaces[workspaceId] ?? defaultWorkspaceState()).startupPreset,
          ),
          loading: false,
          error: undefined,
        }),
      }));
    } catch (error) {
      set((state) => ({
        workspaces: mergeWorkspaceState(state.workspaces, workspaceId, {
          loading: false,
          error: String(error),
        }),
      }));
    }
  },

  toggleTerminal: async (workspaceId) => {
    const workspace = get().workspaces[workspaceId] ?? defaultWorkspaceState();
    if (workspace.isOpen) {
      await get().closeTerminal(workspaceId);
      return;
    }
    await get().openTerminal(workspaceId);
  },

  setLayoutMode: async (workspaceId, mode) => {
    getTerminalSessionGateway().writeStoredLayoutMode(workspaceId, mode);

    if (mode === "split" || mode === "terminal") {
      const workspace = get().workspaces[workspaceId] ?? defaultWorkspaceState();
      if (workspace.sessions.length === 0) {
        await get().openTerminal(workspaceId);
      }
    }

    set((state) => {
      const current = state.workspaces[workspaceId] ?? defaultWorkspaceState();
      const preEditorLayoutMode =
        mode === "editor" && current.layoutMode !== "editor"
          ? current.layoutMode
          : current.preEditorLayoutMode;
      return {
        workspaces: mergeWorkspaceState(state.workspaces, workspaceId, {
          layoutMode: mode,
          preEditorLayoutMode,
          isOpen: (mode === "split" || mode === "terminal") ? true : current.isOpen,
        }),
      };
    });
  },

  // Editor mode is excluded from the cycle — it has its own toggle (Cmd+E)
  cycleLayoutMode: async (workspaceId) => {
    const workspace = get().workspaces[workspaceId] ?? defaultWorkspaceState();
    const order: LayoutMode[] = ["chat", "split", "terminal"];
    const currentIndex = order.indexOf(workspace.layoutMode);
    const nextMode = order[(currentIndex + 1) % order.length];
    await get().setLayoutMode(workspaceId, nextMode);
  },

  runCommandInTerminal: async (workspaceId, command) => {
    const normalized = command.trim();
    if (!workspaceId || !normalized) {
      return false;
    }

    try {
      let workspace = get().workspaces[workspaceId] ?? defaultWorkspaceState();

      if (!workspace.isOpen || workspace.sessions.length === 0) {
        await get().openTerminal(workspaceId);
        workspace = get().workspaces[workspaceId] ?? defaultWorkspaceState();
      }

      let sessionId = workspace.activeSessionId;
      if (!sessionId) {
        sessionId = workspace.sessions[workspace.sessions.length - 1]?.id ?? null;
      }

      if (!sessionId) {
        sessionId = await get().createSession(workspaceId, DEFAULT_COLS, DEFAULT_ROWS);
      }

      if (!sessionId) {
        return false;
      }

      await getTerminalSessionGateway().terminalWrite(workspaceId, sessionId, `${normalized}\r`);
      return true;
    } catch (error) {
      set((state) => ({
        workspaces: mergeWorkspaceState(state.workspaces, workspaceId, {
          error: String(error),
        }),
      }));
      return false;
    }
  },

  materializeWorkspaceStartupPreset: async (
    workspaceId,
    preset,
    cols = DEFAULT_COLS,
    rows = DEFAULT_ROWS,
  ) => {
    const terminalPreset = preset.terminal;
    const workspaceRoot = workspaceRootPath(workspaceId);
    if (!terminalPreset || terminalPreset.groups.length === 0 || !workspaceRoot) {
      set((state) => ({
        workspaces: mergeWorkspaceState(state.workspaces, workspaceId, {
          pendingStartupPreset: null,
          loading: false,
        }),
      }));
      return false;
    }

    set((state) => ({
      workspaces: mergeWorkspaceState(state.workspaces, workspaceId, {
        isOpen: true,
        loading: true,
        error: undefined,
      }),
    }));

    const warnings: string[] = [];
    const launchRequests: Array<{ sessionId: string; harnessId: string; groupName: string }> = [];
    const workspaceStore = useWorkspaceStore.getState();
    const currentActiveRepoId =
      workspaceStore.activeWorkspaceId === workspaceId
        ? workspaceStore.activeRepoId
        : null;
    const repoList =
      workspaceStore.activeWorkspaceId === workspaceId && workspaceStore.repos.length > 0
        ? workspaceStore.repos
        : await getTerminalSessionGateway().getRepos(workspaceId);
    const activeRepoId = resolveActiveRepoId(workspaceId, repoList, currentActiveRepoId);
    const activeRepo = repoList.find((repo) => repo.id === activeRepoId) ?? null;
    const knownHarnesses = new Map(
      useHarnessStore
        .getState()
        .harnesses
        .map((harness) => [harness.id, harness]),
    );

    const runtimeGroups: TerminalGroup[] = [];
    const runtimeSessions: TerminalSession[] = [];
    const logicalSessionIdToRuntimeId: Record<string, string> = {};

    for (const group of terminalPreset.groups) {
      const groupWorktreeConfig = group.worktree?.enabled ? group.worktree : null;
      let resolvedWorktreeConfig: WorkspaceStartupWorktreeConfig | null = groupWorktreeConfig;
      let worktreeRepoPath: string | null = null;

      if (groupWorktreeConfig) {
        if (groupWorktreeConfig.repoMode === "fixed_repo") {
          worktreeRepoPath = groupWorktreeConfig.repoPath
            ? (isAbsolutePath(groupWorktreeConfig.repoPath)
                ? groupWorktreeConfig.repoPath
                : joinPath(workspaceRoot, groupWorktreeConfig.repoPath))
            : null;
        } else {
          worktreeRepoPath = activeRepo?.path ?? null;
        }

        if (!worktreeRepoPath) {
          warnings.push(`"${group.name}" opened without worktrees because no repo was available.`);
          resolvedWorktreeConfig = null;
        }
      }

      const worktreesByLogicalSessionId: Record<string, WorktreeSessionInfo | null> = {};
      if (resolvedWorktreeConfig && worktreeRepoPath) {
        const runId = getTerminalSessionGateway().createTerminalWorktreeRunId();
        const branchPrefix = resolvedWorktreeConfig.branchPrefix?.trim() || "panes/preset";
        const createdWorktrees: WorktreeSessionInfo[] = [];
        let worktreeSetupFailed = false;

        for (let index = 0; index < group.sessions.length; index += 1) {
          const session = group.sessions[index];
          const branch = buildStartupWorktreeBranch(branchPrefix, runId, session.id, index);
          const worktreePath = buildStartupWorktreePath(
            worktreeRepoPath,
            resolvedWorktreeConfig.baseDir,
            runId,
            session.id,
            index,
          );

          try {
            await getTerminalSessionGateway().addGitWorktree(
              worktreeRepoPath,
              worktreePath,
              branch,
              resolvedWorktreeConfig.baseBranch ?? null,
            );
            const info = { repoPath: worktreeRepoPath, worktreePath, branch };
            createdWorktrees.push(info);
            worktreesByLogicalSessionId[session.id] = info;
          } catch (error) {
            worktreeSetupFailed = true;
            warnings.push(`"${group.name}" opened without worktrees because worktree setup failed.`);
            break;
          }
        }

        if (worktreeSetupFailed) {
          await removeWorktreesSequential(createdWorktrees);
          Object.keys(worktreesByLogicalSessionId).forEach((sessionId) => {
            delete worktreesByLogicalSessionId[sessionId];
          });
          resolvedWorktreeConfig = null;
        }
      }

      const groupSessions: TerminalSession[] = [];
      const groupSessionMeta: Record<string, TerminalSessionRuntimeMeta> = {};
      const runtimeSessionMap: Record<string, string> = {};
      let groupFailed = false;
      let warnedWorktreeFallback = false;

      for (const session of group.sessions) {
        try {
          let preferredCwd = resolveSessionStartupCwd(
            workspaceRoot,
            session,
            resolvedWorktreeConfig ? worktreesByLogicalSessionId[session.id]?.worktreePath ?? null : null,
          );

          if (!resolvedWorktreeConfig && (session.cwdBase ?? "workspace") === "worktree") {
            preferredCwd = null;
            if (!warnedWorktreeFallback) {
              warnings.push(`"${group.name}" opened in the workspace root because its worktree was unavailable.`);
              warnedWorktreeFallback = true;
            }
          }

          const created = await createSessionWithFallback(
            workspaceId,
            cols,
            rows,
            preferredCwd,
            `"${group.name}"`,
            warnings,
          );
          groupSessions.push(created);
          runtimeSessionMap[session.id] = created.id;
          logicalSessionIdToRuntimeId[session.id] = created.id;

          const harness = session.harnessId ? knownHarnesses.get(session.harnessId) ?? null : null;
          if (session.harnessId && (session.launchHarnessOnCreate ?? true)) {
            launchRequests.push({
              sessionId: created.id,
              harnessId: session.harnessId,
              groupName: group.name,
            });
          }

          groupSessionMeta[created.id] = defaultSessionMeta({
            harnessId: session.harnessId ?? null,
            harnessName: harness?.name ?? null,
            autoDetectedHarness: false,
            launchHarnessOnCreate: session.launchHarnessOnCreate ?? Boolean(session.harnessId),
            worktree: resolvedWorktreeConfig ? worktreesByLogicalSessionId[session.id] ?? null : null,
          });
        } catch (error) {
          warnings.push(`"${group.name}" was skipped because one of its panes could not be created.`);
          groupFailed = true;
          break;
        }
      }

      if (groupFailed) {
        await closeSessionsSequential(
          workspaceId,
          groupSessions.map((session) => session.id),
        );
        Object.keys(runtimeSessionMap).forEach((sessionId) => {
          delete logicalSessionIdToRuntimeId[sessionId];
        });
        if (resolvedWorktreeConfig) {
          await removeWorktreesSequential(
            Object.values(worktreesByLogicalSessionId).filter(
              (worktree): worktree is WorktreeSessionInfo => worktree !== null,
            ),
          );
        }
        continue;
      }

      runtimeSessions.push(...groupSessions);
      runtimeGroups.push({
        id: group.id,
        root: materializeStartupSplitNode(
          group.root,
          runtimeSessionMap,
          getTerminalSessionGateway().createTerminalSplitId,
        ),
        name: group.name,
        sessionMeta: groupSessionMeta,
        worktreeConfig: resolvedWorktreeConfig,
      });
    }

    if (runtimeGroups.length === 0 || runtimeSessions.length === 0) {
      const summary = summarizeWarnings(warnings);
      if (summary) {
        toast.warning(summary);
      }
      set((state) => ({
        workspaces: mergeWorkspaceState(state.workspaces, workspaceId, {
          pendingStartupPreset: null,
          loading: false,
        }),
      }));
      return false;
    }

    const activeGroupId =
      (terminalPreset.activeGroupId &&
      runtimeGroups.some((group) => group.id === terminalPreset.activeGroupId)
        ? terminalPreset.activeGroupId
        : runtimeGroups[0]?.id) ?? null;
    const focusedSessionId =
      (terminalPreset.focusedSessionId &&
      logicalSessionIdToRuntimeId[terminalPreset.focusedSessionId]
        ? logicalSessionIdToRuntimeId[terminalPreset.focusedSessionId]
        : nextFocusedSessionId(runtimeGroups, activeGroupId, null)) ?? null;
    const configuredBroadcastGroupId =
      terminalPreset.groups.find((group) => group.broadcastOnStart)?.id ?? null;
    const broadcastGroupId =
      configuredBroadcastGroupId && runtimeGroups.some((group) => group.id === configuredBroadcastGroupId)
        ? configuredBroadcastGroupId
        : null;

    getTerminalSessionGateway().writeStoredLayoutMode(workspaceId, preset.defaultView);
    set((state) => ({
      workspaces: mergeWorkspaceState(state.workspaces, workspaceId, {
        isOpen: true,
        layoutMode: preset.defaultView,
        panelSize: clampPanelSize(preset.splitPanelSize ?? DEFAULT_PANEL_SIZE),
        sessions: runtimeSessions,
        groups: runtimeGroups,
        activeGroupId,
        activeSessionId: focusedSessionId,
        focusedSessionId,
        broadcastGroupId,
        startupPreset:
          (state.workspaces[workspaceId] ?? defaultWorkspaceState()).startupPreset,
        pendingStartupPreset: null,
        loading: false,
        error: undefined,
      }),
    }));

    await Promise.all(
      launchRequests.map(async ({ sessionId, harnessId, groupName }) => {
        try {
          const command = await getTerminalSessionGateway().launchHarness(harnessId);
          if (!command) {
            warnings.push(`"${groupName}" opened a normal terminal because "${harnessId}" could not be launched.`);
            return;
          }
          await getTerminalSessionGateway().writeCommandToNewSession(workspaceId, sessionId, command);
        } catch {
          warnings.push(`"${groupName}" opened a normal terminal because "${harnessId}" could not be launched.`);
        }
      }),
    );

    const summary = summarizeWarnings(warnings);
    if (summary) {
      toast.warning(summary);
    }

    return true;
  },

  serializeWorkspaceRuntimeAsStartupPreset: (workspaceId) => {
    if (useWorkspaceStore.getState().activeWorkspaceId !== workspaceId) {
      return null;
    }

    const workspace = get().workspaces[workspaceId] ?? defaultWorkspaceState();
    const workspaceRoot = workspaceRootPath(workspaceId);
    if (!workspaceRoot) {
      return null;
    }

    const relativeToBase = (basePath: string, targetPath: string): string | null => {
      const normalizedBase = basePath.replace(/\/+$/, "");
      if (targetPath === normalizedBase) {
        return ".";
      }
      if (targetPath.startsWith(`${normalizedBase}/`)) {
        return targetPath.slice(normalizedBase.length + 1) || ".";
      }
      return null;
    };

    const groups = workspace.groups.map((group) => {
      const sessionIds = collectSessionIds(group.root);
      const sessions = sessionIds
        .map((sessionId) => {
          const runtimeSession = workspace.sessions.find((session) => session.id === sessionId);
          if (!runtimeSession) {
            return null;
          }
          const meta = getSessionMeta(group, sessionId);
          const worktreePath = meta.worktree?.worktreePath ?? null;

          let cwdBase: "workspace" | "worktree" | "absolute" = "workspace";
          let cwd = runtimeSession.cwd;
          const relativeToWorktree =
            worktreePath ? relativeToBase(worktreePath, runtimeSession.cwd) : null;
          if (relativeToWorktree !== null) {
            cwdBase = "worktree";
            cwd = relativeToWorktree;
          } else {
            const relativeToWorkspace = relativeToBase(workspaceRoot, runtimeSession.cwd);
            if (relativeToWorkspace !== null) {
              cwdBase = "workspace";
              cwd = relativeToWorkspace;
            } else {
              cwdBase = "absolute";
            }
          }

          return {
            id: sessionId,
            cwd,
            cwdBase,
            harnessId: meta.harnessId ?? null,
            launchHarnessOnCreate: meta.launchHarnessOnCreate ?? Boolean(meta.harnessId),
          };
        })
        .filter((session): session is NonNullable<typeof session> => session !== null);

      const runtimeToPresetSessionId = Object.fromEntries(sessionIds.map((sessionId) => [sessionId, sessionId]));

      return {
        id: group.id,
        name: group.name,
        broadcastOnStart: workspace.broadcastGroupId === group.id,
        worktree: inferWorktreeConfig(group),
        sessions,
        root: serializeRuntimeSplitNode(group.root, runtimeToPresetSessionId),
      };
    });

    return {
      version: 1,
      defaultView: workspace.layoutMode,
      splitPanelSize: clampPanelSize(workspace.panelSize),
      terminal:
        groups.length > 0
          ? {
              applyWhen: "no_live_sessions",
              groups,
              activeGroupId: workspace.activeGroupId,
              focusedSessionId: workspace.focusedSessionId,
            }
          : null,
    };
  },

  applyWorkspaceStartupPresetNow: async (workspaceId, preset, options) => {
    const workspace = get().workspaces[workspaceId] ?? defaultWorkspaceState();
    const worktrees =
      options?.removeWorktrees
        ? workspace.groups.flatMap((group) => getGroupWorktreesFromMeta(group))
        : [];
    const nextPendingPreset = pendingStartupPresetFor(preset);
    const shouldBootstrapTerminal =
      nextPendingPreset !== null
      || preset.defaultView === "split"
      || preset.defaultView === "terminal";

    set((state) => ({
      workspaces: mergeWorkspaceState(state.workspaces, workspaceId, {
        loading: true,
        error: undefined,
      }),
    }));

    try {
      await getTerminalSessionGateway().terminalCloseWorkspaceSessions(workspaceId);
      if (worktrees.length > 0) {
        const failures = await removeWorktreesSequential(worktrees);
        if (failures.length > 0) {
          toast.warning(
            t("app:terminal.toasts.someWorktreesNotRemoved", { message: failures[0] }),
          );
        }
      }
      getTerminalSessionGateway().writeStoredLayoutMode(workspaceId, preset.defaultView);
      set((state) => ({
        workspaces: mergeWorkspaceState(state.workspaces, workspaceId, {
          isOpen: shouldBootstrapTerminal,
          layoutMode: preset.defaultView,
          panelSize: clampPanelSize(preset.splitPanelSize ?? workspace.panelSize),
          sessions: [],
          groups: [],
          activeGroupId: null,
          activeSessionId: null,
          focusedSessionId: null,
          broadcastGroupId: null,
          startupPreset:
            (state.workspaces[workspaceId] ?? defaultWorkspaceState()).startupPreset,
          pendingStartupPreset: nextPendingPreset,
          // Mirror openTerminal(): keep loading until the mounted panel has
          // attached listeners and bootstrapped the new terminal state.
          loading: shouldBootstrapTerminal,
          error: undefined,
        }),
      }));
      return true;
    } catch (error) {
      set((state) => ({
        workspaces: mergeWorkspaceState(state.workspaces, workspaceId, {
          loading: false,
          error: String(error),
        }),
      }));
      return false;
    }
  },

  createSession: async (workspaceId, cols = DEFAULT_COLS, rows = DEFAULT_ROWS, harnessId, harnessName) => {
    set((state) => ({
      workspaces: mergeWorkspaceState(state.workspaces, workspaceId, {
        isOpen: true,
        loading: true,
        error: undefined,
      }),
    }));

    try {
      const created = await getTerminalSessionGateway().terminalCreateSession(workspaceId, cols, rows);
      set((state) => {
        const current = state.workspaces[workspaceId] ?? defaultWorkspaceState();

        let groupName: string;
        // Use a temporary id for exclusion since the group doesn't exist yet
        const tempId = "__new__";
        if (harnessId && harnessName) {
          groupName = nextHarnessName(harnessName, harnessId, tempId, current.groups);
        } else {
          groupName = `Terminal ${nextTerminalNumber(current.groups)}`;
        }

        const newGroup = makeLeafGroup(created.id, groupName, {
          harnessId: harnessId ?? null,
          harnessName: harnessName ?? null,
          autoDetectedHarness: false,
          launchHarnessOnCreate: Boolean(harnessId),
        });
        const sessions = [
          ...current.sessions.filter((session) => session.id !== created.id),
          created,
        ];
        const groups = [...current.groups, newGroup];
        return {
          workspaces: mergeWorkspaceState(state.workspaces, workspaceId, {
            isOpen: true,
            sessions,
            activeSessionId: created.id,
            groups,
            activeGroupId: newGroup.id,
            focusedSessionId: created.id,
            pendingStartupPreset: null,
            loading: false,
            error: undefined,
          }),
        };
      });
      return created.id;
    } catch (error) {
      set((state) => ({
        workspaces: mergeWorkspaceState(state.workspaces, workspaceId, {
          loading: false,
          error: String(error),
        }),
      }));
      return null;
    }
  },

  closeSession: async (workspaceId, sessionId) => {
    try {
      await getTerminalSessionGateway().terminalCloseSession(workspaceId, sessionId);
      get().handleSessionExit(workspaceId, sessionId);
    } catch (error) {
      set((state) => ({
        workspaces: mergeWorkspaceState(state.workspaces, workspaceId, {
          error: String(error),
        }),
      }));
    }
  },

  setActiveSession: (workspaceId, sessionId) => {
    set((state) => {
      const workspace = state.workspaces[workspaceId] ?? defaultWorkspaceState();
      if (!workspace.sessions.some((session) => session.id === sessionId)) {
        return state;
      }
      return {
        workspaces: mergeWorkspaceState(state.workspaces, workspaceId, {
          activeSessionId: sessionId,
          focusedSessionId: sessionId,
        }),
      };
    });
  },

  setPanelSize: (workspaceId, size) => {
    set((state) => ({
      workspaces: mergeWorkspaceState(
        state.workspaces,
        workspaceId,
        {
          panelSize: clampPanelSize(
            size,
            (state.workspaces[workspaceId] ?? defaultWorkspaceState()).panelSize,
          ),
        },
      ),
    }));
  },

  syncSessions: async (workspaceId) => {
    try {
      const sessions = await getTerminalSessionGateway().terminalListSessions(workspaceId);
      const storedMode = getTerminalSessionGateway().readStoredLayoutMode(workspaceId);
      set((state) => {
        const current = state.workspaces[workspaceId] ?? defaultWorkspaceState();
        const hasSessions = sessions.length > 0;
        const restoredMode = hasSessions && (storedMode === "split" || storedMode === "terminal")
          ? storedMode
          : current.layoutMode;

        const liveIds = new Set(sessions.map((s) => s.id));
        const notificationsBySessionId = pruneNotificationsByLiveSessions(
          current.notificationsBySessionId,
          liveIds,
        );
        let groups: TerminalGroup[];
        if (current.groups.length === 0 && hasSessions) {
          groups = [];
          for (const s of sessions) {
            const n = nextTerminalNumber(groups);
            groups.push(makeLeafGroup(s.id, `Terminal ${n}`));
          }
        } else {
          groups = current.groups
            .map((group) => pruneDeadSessionsFromGroup(group, liveIds))
            .filter(isTerminalGroup);
        }

        const activeGroupId =
          (current.activeGroupId && groups.some((g) => g.id === current.activeGroupId)
            ? current.activeGroupId
            : groups[groups.length - 1]?.id) ?? null;
        const focusedId = nextFocusedSessionId(groups, activeGroupId, current.focusedSessionId);

        return {
          workspaces: mergeWorkspaceState(state.workspaces, workspaceId, {
            sessions,
            notificationsBySessionId,
            activeSessionId: focusedId,
            groups,
            activeGroupId,
            focusedSessionId: focusedId,
            loading: false,
            error: undefined,
            pendingStartupPreset: hasSessions ? null : current.pendingStartupPreset,
            ...(hasSessions ? { isOpen: true, layoutMode: restoredMode } : {}),
          }),
        };
      });
    } catch (error) {
      set((state) => ({
        workspaces: mergeWorkspaceState(state.workspaces, workspaceId, {
          loading: false,
          error: String(error),
        }),
      }));
    }
  },

  hydrateNotifications: async (workspaceId) => {
    const requestId =
      (get().workspaces[workspaceId]?.notificationHydrationRequestId ?? 0) + 1;
    set((state) => ({
      workspaces: mergeWorkspaceState(state.workspaces, workspaceId, {
        notificationHydrating: true,
        notificationHydrationRequestId: requestId,
        notificationTouchedAll: false,
        notificationTouchedSessionIds: {},
      }),
    }));
    try {
      const notifications = await getTerminalSessionGateway().terminalListNotifications(workspaceId);
      set((state) => {
        const current = state.workspaces[workspaceId] ?? defaultWorkspaceState();
        if (current.notificationHydrationRequestId !== requestId) {
          return state;
        }
        const liveIds = new Set(current.sessions.map((session) => session.id));
        const hydrated = indexNotificationsBySession(notifications, liveIds);
        return {
          workspaces: mergeWorkspaceState(state.workspaces, workspaceId, {
            notificationsBySessionId: resolveHydratedNotifications(current, hydrated, liveIds),
            notificationHydrating: false,
            notificationHydrationRequestId: requestId,
            notificationTouchedAll: false,
            notificationTouchedSessionIds: {},
          }),
        };
      });
    } catch (error) {
      console.warn(`Failed to hydrate terminal notifications for ${workspaceId}:`, error);
      set((state) => ({
        workspaces:
          (state.workspaces[workspaceId]?.notificationHydrationRequestId ?? 0) !== requestId
            ? state.workspaces
            : mergeWorkspaceState(state.workspaces, workspaceId, {
                notificationHydrating: false,
                notificationHydrationRequestId: requestId,
                notificationTouchedAll: false,
                notificationTouchedSessionIds: {},
              }),
      }));
    }
  },

  applyNotification: (workspaceId, notification) => {
    set((state) => {
      const current = state.workspaces[workspaceId] ?? defaultWorkspaceState();
      if (!current.sessions.some((session) => session.id === notification.sessionId)) {
        return state;
      }
      return {
        workspaces: mergeWorkspaceState(state.workspaces, workspaceId, {
          notificationsBySessionId: {
            ...current.notificationsBySessionId,
            [notification.sessionId]: notification,
          },
          ...withNotificationHydrationTouch(current, notification.sessionId),
        }),
      };
    });
  },

  clearNotificationLocal: (workspaceId, sessionId) => {
    set((state) => {
      const current = state.workspaces[workspaceId] ?? defaultWorkspaceState();
      const notificationsBySessionId = clearNotificationRecord(
        current.notificationsBySessionId,
        sessionId ?? null,
      );
      const hydrationTouch = withNotificationHydrationTouch(current, sessionId ?? null);
      if (
        notificationsBySessionId === current.notificationsBySessionId
        && !hasNotificationHydrationTouchChange(current, hydrationTouch)
      ) {
        return state;
      }
      return {
        workspaces: mergeWorkspaceState(state.workspaces, workspaceId, {
          notificationsBySessionId,
          ...hydrationTouch,
        }),
      };
    });
  },

  clearNotification: async (workspaceId, sessionId) => {
    get().clearNotificationLocal(workspaceId, sessionId ?? null);
    try {
      await getTerminalSessionGateway().terminalClearNotification(workspaceId, sessionId ?? null);
    } catch (error) {
      console.warn(`Failed to clear terminal notification for ${workspaceId}:`, error);
      await get().hydrateNotifications(workspaceId);
    }
  },

  syncNotificationFocus: async (workspaceId, sessionId, windowFocused) => {
    if (windowFocused && workspaceId && sessionId) {
      get().clearNotificationLocal(workspaceId, sessionId);
    }
    try {
      await getTerminalSessionGateway().terminalSetNotificationFocus(
        workspaceId,
        sessionId,
        windowFocused,
      );
    } catch (error) {
      console.warn("Failed to sync terminal notification focus:", error);
      if (workspaceId) {
        await get().hydrateNotifications(workspaceId);
      }
    }
  },

  handleSessionExit: (workspaceId, sessionId) => {
    set((state) => {
      const workspace = state.workspaces[workspaceId] ?? defaultWorkspaceState();
      const sessions = workspace.sessions.filter((session) => session.id !== sessionId);
      const notificationsBySessionId = clearNotificationRecord(
        workspace.notificationsBySessionId,
        sessionId,
      );

      const groups = workspace.groups
        .map((group) => pruneSessionFromGroup(group, sessionId))
        .filter(isTerminalGroup);

      const noSessionsLeft = sessions.length === 0;
      const isTerminalMode = workspace.layoutMode === "terminal" || workspace.layoutMode === "split";
      if (noSessionsLeft && isTerminalMode) {
        getTerminalSessionGateway().writeStoredLayoutMode(workspaceId, "chat");
      }

      const activeGroupId =
        (workspace.activeGroupId && groups.some((g) => g.id === workspace.activeGroupId)
          ? workspace.activeGroupId
          : groups[groups.length - 1]?.id) ?? null;
      const focusedId = nextFocusedSessionId(
        groups,
        activeGroupId,
        workspace.focusedSessionId === sessionId ? null : workspace.focusedSessionId,
      );

      const broadcastGroupId =
        workspace.broadcastGroupId && groups.some((g) => g.id === workspace.broadcastGroupId)
          ? workspace.broadcastGroupId
          : null;
      // Closing the last live session should queue the saved preset for the
      // next explicit open, not immediately recreate the terminal.
      const pendingStartupPreset = noSessionsLeft
        ? workspace.pendingStartupPreset ?? pendingStartupPresetFor(workspace.startupPreset)
        : workspace.pendingStartupPreset;

      return {
        workspaces: mergeWorkspaceState(state.workspaces, workspaceId, {
          isOpen: noSessionsLeft ? false : workspace.isOpen,
          sessions,
          notificationsBySessionId,
          ...withNotificationHydrationTouch(workspace, sessionId),
          activeSessionId: focusedId,
          groups,
          activeGroupId,
          focusedSessionId: focusedId,
          broadcastGroupId,
          pendingStartupPreset,
          ...(noSessionsLeft && isTerminalMode ? { layoutMode: "chat" as LayoutMode } : {}),
        }),
      };
    });
  },

  splitSession: async (workspaceId, sessionId, direction, cols = DEFAULT_COLS, rows = DEFAULT_ROWS) => {
    const workspace = get().workspaces[workspaceId] ?? defaultWorkspaceState();
    const group = findGroupForSession(workspace.groups, sessionId);
    if (!group) return;

    try {
      const created = await getTerminalSessionGateway().terminalCreateSession(workspaceId, cols, rows);
      set((state) => {
        const current = state.workspaces[workspaceId] ?? defaultWorkspaceState();
        const sessions = [
          ...current.sessions.filter((s) => s.id !== created.id),
          created,
        ];

        const splitContainer: SplitNode = {
          type: "split",
          id: getTerminalSessionGateway().createTerminalSplitId(),
          direction,
          ratio: 0.5,
          children: [
            { type: "leaf", sessionId },
            { type: "leaf", sessionId: created.id },
          ],
        };

        const groups = current.groups.map((g) => {
          if (g.id !== group.id) return g;
          return {
            ...g,
            root: replaceLeafInTree(g.root, sessionId, splitContainer),
            sessionMeta: {
              ...(g.sessionMeta ?? {}),
              [created.id]: defaultSessionMeta(),
            },
          };
        });

        return {
          workspaces: mergeWorkspaceState(state.workspaces, workspaceId, {
            sessions,
            activeSessionId: created.id,
            groups,
            activeGroupId: group.id,
            focusedSessionId: created.id,
          }),
        };
      });
    } catch (error) {
      set((state) => ({
        workspaces: mergeWorkspaceState(state.workspaces, workspaceId, {
          error: String(error),
        }),
      }));
    }
  },

  setFocusedSession: (workspaceId, sessionId) => {
    set((state) => {
      const workspace = state.workspaces[workspaceId] ?? defaultWorkspaceState();
      if (!workspace.sessions.some((s) => s.id === sessionId)) return state;
      const group = findGroupForSession(workspace.groups, sessionId);
      return {
        workspaces: mergeWorkspaceState(state.workspaces, workspaceId, {
          activeSessionId: sessionId,
          focusedSessionId: sessionId,
          ...(group ? { activeGroupId: group.id } : {}),
        }),
      };
    });
  },

  setActiveGroup: (workspaceId, groupId) => {
    set((state) => {
      const workspace = state.workspaces[workspaceId] ?? defaultWorkspaceState();
      const group = workspace.groups.find((g) => g.id === groupId);
      if (!group) return state;
      const focusedId = nextFocusedSessionId([group], groupId, workspace.focusedSessionId);
      return {
        workspaces: mergeWorkspaceState(state.workspaces, workspaceId, {
          activeGroupId: groupId,
          focusedSessionId: focusedId,
          activeSessionId: focusedId,
        }),
      };
    });
  },

  updateGroupRatio: (workspaceId, groupId, containerId, ratio) => {
    set((state) => {
      const workspace = state.workspaces[workspaceId] ?? defaultWorkspaceState();
      const groups = workspace.groups.map((g) => {
        if (g.id !== groupId) return g;
        return { ...g, root: updateRatioInTree(g.root, containerId, ratio) };
      });
      return {
        workspaces: mergeWorkspaceState(state.workspaces, workspaceId, { groups }),
      };
    });
  },

  renameGroup: (workspaceId, groupId, name) => {
    const trimmed = name.trim();
    if (!trimmed) return;
    set((state) => {
      const workspace = state.workspaces[workspaceId] ?? defaultWorkspaceState();
      const groups = workspace.groups.map((g) =>
        g.id === groupId ? { ...g, name: trimmed } : g,
      );
      return {
        workspaces: mergeWorkspaceState(state.workspaces, workspaceId, { groups }),
      };
    });
  },

  reorderGroups: (workspaceId, fromIndex, toIndex) => {
    if (fromIndex === toIndex) return;
    set((state) => {
      const workspace = state.workspaces[workspaceId] ?? defaultWorkspaceState();
      const groups = reorderTerminalGroups(workspace.groups, fromIndex, toIndex);
      return {
        workspaces: mergeWorkspaceState(state.workspaces, workspaceId, { groups }),
      };
    });
  },

  updateSessionHarness: (workspaceId, sessionId, harnessId, harnessName, autoDetected) => {
    set((state) => {
      const workspace = state.workspaces[workspaceId] ?? defaultWorkspaceState();
      const groups = workspace.groups.map((g) => {
        if (!collectSessionIds(g.root).includes(sessionId)) return g;

        const previousMeta = getSessionMeta(g, sessionId);
        let nextGroup = setSessionMeta(g, sessionId, {
          ...previousMeta,
          harnessId,
          harnessName,
          autoDetectedHarness: autoDetected,
        });

        if (collectSessionIds(g.root).length === 1) {
          if (harnessId && harnessName) {
            nextGroup = {
              ...nextGroup,
              name: nextHarnessName(harnessName, harnessId, g.id, workspace.groups),
            };
          } else if (previousMeta.autoDetectedHarness) {
            nextGroup = {
              ...nextGroup,
              name: `Terminal ${nextTerminalNumber(workspace.groups.filter((other) => other.id !== g.id))}`,
            };
          }
        }
        return nextGroup;
      });
      return {
        workspaces: mergeWorkspaceState(state.workspaces, workspaceId, { groups }),
      };
    });
  },

  toggleBroadcast: (workspaceId, groupId) => {
    set((state) => {
      const current = state.workspaces[workspaceId] ?? defaultWorkspaceState();
      const next = current.broadcastGroupId === groupId ? null : groupId;
      return {
        workspaces: mergeWorkspaceState(state.workspaces, workspaceId, {
          broadcastGroupId: next,
        }),
      };
    });
  },

  createMultiSessionGroup: async (workspaceId, harnesses, worktreeConfig, cols = DEFAULT_COLS, rows = DEFAULT_ROWS) => {
    if (harnesses.length === 0) return null;

    set((state) => ({
      workspaces: mergeWorkspaceState(state.workspaces, workspaceId, {
        isOpen: true,
        loading: true,
        error: undefined,
      }),
    }));

    // Track created worktrees for cleanup on failure
    const createdWorktrees: WorktreeSessionInfo[] = [];
    const effectiveWorktreeConfig =
      worktreeConfig?.enabled && worktreeConfig.repoPath
        ? worktreeConfig
        : null;
    const worktreeRepoPath = effectiveWorktreeConfig?.repoPath ?? null;

    try {
      // Phase 1: Create worktrees sequentially if configured (git locks prevent parallelism)
      const worktreeRunId = effectiveWorktreeConfig && worktreeRepoPath
        ? getTerminalSessionGateway().createTerminalWorktreeRunId()
        : null;
      if (effectiveWorktreeConfig && worktreeRepoPath && worktreeRunId) {
        const branchPrefix = effectiveWorktreeConfig.branchPrefix?.trim() || "panes/preset";
        for (let i = 0; i < harnesses.length; i++) {
          const logicalSessionId = harnesses[i]?.harnessId
            ? `${harnesses[i].harnessId}-${i + 1}`
            : `session-${i + 1}`;
          const branch = buildStartupWorktreeBranch(
            branchPrefix,
            worktreeRunId,
            logicalSessionId,
            i,
          );
          const worktreePath = buildStartupWorktreePath(
            worktreeRepoPath,
            effectiveWorktreeConfig.baseDir,
            worktreeRunId,
            logicalSessionId,
            i,
          );
          await getTerminalSessionGateway().addGitWorktree(
            worktreeRepoPath,
            worktreePath,
            branch,
            effectiveWorktreeConfig.baseBranch ?? null,
          );
          createdWorktrees.push({ repoPath: worktreeRepoPath, worktreePath, branch });
        }
      }

      // Phase 2: Create terminal sessions (with CWD override if worktrees are active)
      const creationResults = await Promise.allSettled(
        harnesses.map((_h, i) => {
          const cwd = createdWorktrees[i]?.worktreePath ?? undefined;
          return getTerminalSessionGateway().terminalCreateSession(workspaceId, cols, rows, cwd);
        }),
      );
      const created = creationResults
        .filter((result): result is PromiseFulfilledResult<TerminalSession> => result.status === "fulfilled")
        .map((result) => result.value);
      const firstFailure = creationResults.find(
        (result): result is PromiseRejectedResult => result.status === "rejected",
      );
      if (firstFailure) {
        await Promise.allSettled(
          created.map((session) =>
            getTerminalSessionGateway().terminalCloseSession(workspaceId, session.id)
          ),
        );
        throw firstFailure.reason;
      }

      const sessionIds = created.map((s) => s.id);
      const root = buildGridSplitTree(
        sessionIds,
        getTerminalSessionGateway().createTerminalSplitId,
      );

      const groupId = getTerminalSessionGateway().createTerminalGroupId();

      // Build worktree map keyed by session ID
      const sessionMeta = Object.fromEntries(
        sessionIds.map((sid, i) => [
          sid,
          defaultSessionMeta({
            harnessId: harnesses[i]?.harnessId ?? null,
            harnessName: harnesses[i]?.name ?? null,
            autoDetectedHarness: false,
            launchHarnessOnCreate: true,
            worktree: createdWorktrees[i] ?? null,
          }),
        ]),
      );

      set((state) => {
        const current = state.workspaces[workspaceId] ?? defaultWorkspaceState();
        const groupName = harnesses.length === 1
          ? nextHarnessName(harnesses[0].name, harnesses[0].harnessId, groupId, current.groups)
          : `${harnesses.length} agents`;
        const newGroup: TerminalGroup = {
          id: groupId,
          root,
          name: groupName,
          sessionMeta,
          worktreeConfig: effectiveWorktreeConfig,
        };
        const sessions = [
          ...current.sessions.filter((s) => !sessionIds.includes(s.id)),
          ...created,
        ];
        const groups = [...current.groups, newGroup];
        return {
          workspaces: mergeWorkspaceState(state.workspaces, workspaceId, {
            isOpen: true,
            sessions,
            groups,
            activeGroupId: groupId,
            activeSessionId: sessionIds[0],
            focusedSessionId: sessionIds[0],
            pendingStartupPreset: null,
            loading: false,
            error: undefined,
          }),
        };
      });

      return { groupId, sessionIds };
    } catch (error) {
      let message = String(error);

      // Clean up any worktrees created before the failure
      if (createdWorktrees.length > 0) {
        const cleanupFailures = await removeWorktreesSequential(createdWorktrees);
        if (cleanupFailures.length > 0) {
          message = `${message}. Cleanup failed for ${cleanupFailures.length} worktree(s): ${cleanupFailures.join("; ")}`;
        }
      }
      set((state) => ({
        workspaces: mergeWorkspaceState(state.workspaces, workspaceId, {
          loading: false,
          error: message,
        }),
      }));
      return null;
    }
  },

  getGroupWorktrees: (workspaceId, groupId) => {
    const workspace = get().workspaces[workspaceId] ?? defaultWorkspaceState();
    const group = workspace.groups.find((item) => item.id === groupId);
    return group ? getGroupWorktreesFromMeta(group) : [];
  },

  removeGroupWorktrees: async (workspaceId, worktrees) => {
    if (worktrees.length === 0) return;

    const failures = await removeWorktreesSequential(worktrees);
    if (failures.length === 0) return;

    const message = `Failed to remove ${failures.length} worktree(s): ${failures.join("; ")}`;
    set((state) => ({
      workspaces: mergeWorkspaceState(state.workspaces, workspaceId, {
        error: message,
      }),
    }));
    throw new Error(message);
  },
}));

import type {
  GitWorktree,
  TerminalExitEvent,
  TerminalForegroundChangedEvent,
  Repo,
  TerminalNotification,
  TerminalNotificationClearedEvent,
  TerminalOutputReadyEvent,
  TerminalRendererDiagnostics,
  TerminalResumeSession,
  TerminalSession,
  WorkspaceStartupPreset,
} from "../../../types";
import type { LayoutMode } from "../domain/terminalLayout";

export type TerminalSessionUnlisten = () => void;

export interface TerminalSessionGateway {
  addGitWorktree(
    repoPath: string,
    worktreePath: string,
    branchName: string,
    baseRef?: string | null,
  ): Promise<GitWorktree>;
  createTerminalGroupId(): string;
  createTerminalSplitId(): string;
  createTerminalWorktreeRunId(): string;
  getTerminalAcceleratedRendering(): Promise<boolean>;
  getRepos(workspaceId: string): Promise<Repo[]>;
  getWorkspaceStartupPreset(workspaceId: string): Promise<WorkspaceStartupPreset | null>;
  launchHarness(harnessId: string): Promise<string>;
  readStoredLayoutMode(workspaceId: string): LayoutMode;
  removeGitWorktree(
    repoPath: string,
    worktreePath: string,
    force: boolean,
    branchName?: string | null,
    deleteBranch?: boolean,
  ): Promise<void>;
  setTerminalAcceleratedRendering(enabled: boolean): Promise<boolean>;
  terminalClearNotification(workspaceId: string, sessionId?: string | null): Promise<void>;
  terminalCloseSession(workspaceId: string, sessionId: string): Promise<void>;
  terminalCloseWorkspaceSessions(workspaceId: string): Promise<void>;
  terminalCreateSession(
    workspaceId: string,
    cols: number,
    rows: number,
    cwd?: string | null,
  ): Promise<TerminalSession>;
  terminalDrainOutput(
    workspaceId: string,
    sessionId: string,
    fromSeq: number | null,
    targetBytes: number,
  ): Promise<TerminalResumeSession>;
  terminalGetRendererDiagnostics(
    workspaceId: string,
    sessionId: string,
  ): Promise<TerminalRendererDiagnostics>;
  terminalListNotifications(workspaceId: string): Promise<TerminalNotification[]>;
  terminalListSessions(workspaceId: string): Promise<TerminalSession[]>;
  terminalResize(
    workspaceId: string,
    sessionId: string,
    cols: number,
    rows: number,
    pixelWidth: number,
    pixelHeight: number,
  ): Promise<void>;
  terminalResumeSession(
    workspaceId: string,
    sessionId: string,
    fromSeq?: number | null,
  ): Promise<TerminalResumeSession>;
  terminalSetNotificationFocus(
    workspaceId: string | null,
    sessionId: string | null,
    windowFocused: boolean,
  ): Promise<void>;
  terminalWrite(workspaceId: string, sessionId: string, data: string): Promise<void>;
  terminalWriteBytes(workspaceId: string, sessionId: string, data: number[]): Promise<void>;
  listenTerminalExit(
    workspaceId: string,
    onEvent: (event: TerminalExitEvent) => void,
  ): Promise<TerminalSessionUnlisten>;
  listenTerminalForegroundChanged(
    workspaceId: string,
    onEvent: (event: TerminalForegroundChangedEvent) => void,
  ): Promise<TerminalSessionUnlisten>;
  listenTerminalNotification(
    workspaceId: string,
    onEvent: (event: TerminalNotification) => void,
  ): Promise<TerminalSessionUnlisten>;
  listenTerminalNotificationCleared(
    workspaceId: string,
    onEvent: (event: TerminalNotificationClearedEvent) => void,
  ): Promise<TerminalSessionUnlisten>;
  listenTerminalOutput(
    workspaceId: string,
    onEvent: (event: TerminalOutputReadyEvent) => void,
  ): Promise<TerminalSessionUnlisten>;
  writeCommandToNewSession(
    workspaceId: string,
    sessionId: string,
    command: string,
  ): Promise<void>;
  writeStoredLayoutMode(workspaceId: string, mode: LayoutMode): void;
}

let configuredTerminalSessionGateway: TerminalSessionGateway | null = null;

export function configureTerminalSessionGateway(gateway: TerminalSessionGateway): void {
  configuredTerminalSessionGateway = gateway;
}

export function getTerminalSessionGateway(): TerminalSessionGateway {
  if (!configuredTerminalSessionGateway) {
    throw new Error("TerminalSessionGateway has not been configured.");
  }
  return configuredTerminalSessionGateway;
}

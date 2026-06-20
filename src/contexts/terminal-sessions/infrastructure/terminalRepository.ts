import * as ipcModule from "../../../lib/ipc";
import type {
  TerminalExitEvent,
  TerminalForegroundChangedEvent,
  TerminalNotification,
  TerminalNotificationClearedEvent,
  TerminalOutputReadyEvent,
} from "../../../types";
import type { TerminalSessionGateway } from "../application/terminalSessionGateway";
import {
  createTerminalGroupId,
  createTerminalSplitId,
  createTerminalWorktreeRunId,
} from "./terminalIdGenerator";
import {
  readStoredLayoutMode,
  writeStoredLayoutMode,
} from "./terminalLayoutStorage";

const { ipc } = ipcModule;

type TerminalUnlisten = () => void;
type WriteCommandToNewSession = (
  workspaceId: string,
  sessionId: string,
  command: string,
) => Promise<void>;

function writeCommandToNewSession(
  workspaceId: string,
  sessionId: string,
  command: string,
): Promise<void> {
  const helper = (ipcModule as { writeCommandToNewSession?: WriteCommandToNewSession })
    .writeCommandToNewSession;
  if (helper) {
    return helper(workspaceId, sessionId, command);
  }
  return ipc.terminalWrite(workspaceId, sessionId, `${command}\r`);
}

function listenTerminalOutput(
  workspaceId: string,
  onEvent: (event: TerminalOutputReadyEvent) => void,
): Promise<TerminalUnlisten> {
  const listener = (ipcModule as {
    listenTerminalOutput?: (
      workspaceId: string,
      onEvent: (event: TerminalOutputReadyEvent) => void,
    ) => Promise<TerminalUnlisten>;
  }).listenTerminalOutput;
  if (!listener) {
    return Promise.reject(new Error("listenTerminalOutput is unavailable."));
  }
  return listener(workspaceId, onEvent);
}

function listenTerminalExit(
  workspaceId: string,
  onEvent: (event: TerminalExitEvent) => void,
): Promise<TerminalUnlisten> {
  const listener = (ipcModule as {
    listenTerminalExit?: (
      workspaceId: string,
      onEvent: (event: TerminalExitEvent) => void,
    ) => Promise<TerminalUnlisten>;
  }).listenTerminalExit;
  if (!listener) {
    return Promise.reject(new Error("listenTerminalExit is unavailable."));
  }
  return listener(workspaceId, onEvent);
}

function listenTerminalForegroundChanged(
  workspaceId: string,
  onEvent: (event: TerminalForegroundChangedEvent) => void,
): Promise<TerminalUnlisten> {
  const listener = (ipcModule as {
    listenTerminalForegroundChanged?: (
      workspaceId: string,
      onEvent: (event: TerminalForegroundChangedEvent) => void,
    ) => Promise<TerminalUnlisten>;
  }).listenTerminalForegroundChanged;
  if (!listener) {
    return Promise.reject(new Error("listenTerminalForegroundChanged is unavailable."));
  }
  return listener(workspaceId, onEvent);
}

function listenTerminalNotification(
  workspaceId: string,
  onEvent: (event: TerminalNotification) => void,
): Promise<TerminalUnlisten> {
  const listener = (ipcModule as {
    listenTerminalNotification?: (
      workspaceId: string,
      onEvent: (event: TerminalNotification) => void,
    ) => Promise<TerminalUnlisten>;
  }).listenTerminalNotification;
  if (!listener) {
    return Promise.reject(new Error("listenTerminalNotification is unavailable."));
  }
  return listener(workspaceId, onEvent);
}

function listenTerminalNotificationCleared(
  workspaceId: string,
  onEvent: (event: TerminalNotificationClearedEvent) => void,
): Promise<TerminalUnlisten> {
  const listener = (ipcModule as {
    listenTerminalNotificationCleared?: (
      workspaceId: string,
      onEvent: (event: TerminalNotificationClearedEvent) => void,
    ) => Promise<TerminalUnlisten>;
  }).listenTerminalNotificationCleared;
  if (!listener) {
    return Promise.reject(new Error("listenTerminalNotificationCleared is unavailable."));
  }
  return listener(workspaceId, onEvent);
}

export const terminalRepository = {
  addGitWorktree: ipc.addGitWorktree,
  getRepos: ipc.getRepos,
  getWorkspaceStartupPreset: ipc.getWorkspaceStartupPreset,
  launchHarness: ipc.launchHarness,
  getTerminalAcceleratedRendering: ipc.getTerminalAcceleratedRendering,
  removeGitWorktree: ipc.removeGitWorktree,
  setTerminalAcceleratedRendering: ipc.setTerminalAcceleratedRendering,
  terminalClearNotification: ipc.terminalClearNotification,
  terminalCloseSession: ipc.terminalCloseSession,
  terminalCloseWorkspaceSessions: ipc.terminalCloseWorkspaceSessions,
  terminalCreateSession: ipc.terminalCreateSession,
  terminalDrainOutput: ipc.terminalDrainOutput,
  terminalGetRendererDiagnostics: ipc.terminalGetRendererDiagnostics,
  terminalListNotifications: ipc.terminalListNotifications,
  terminalListSessions: ipc.terminalListSessions,
  terminalResumeSession: ipc.terminalResumeSession,
  terminalResize: ipc.terminalResize,
  terminalSetNotificationFocus: ipc.terminalSetNotificationFocus,
  terminalWrite: ipc.terminalWrite,
  terminalWriteBytes: ipc.terminalWriteBytes,
  listenTerminalExit,
  listenTerminalForegroundChanged,
  listenTerminalNotification,
  listenTerminalNotificationCleared,
  listenTerminalOutput,
  writeCommandToNewSession,
};

export const terminalSessionGateway: TerminalSessionGateway = {
  addGitWorktree: terminalRepository.addGitWorktree,
  createTerminalGroupId,
  createTerminalSplitId,
  createTerminalWorktreeRunId,
  getTerminalAcceleratedRendering: terminalRepository.getTerminalAcceleratedRendering,
  getRepos: terminalRepository.getRepos,
  getWorkspaceStartupPreset: terminalRepository.getWorkspaceStartupPreset,
  launchHarness: terminalRepository.launchHarness,
  readStoredLayoutMode,
  removeGitWorktree: terminalRepository.removeGitWorktree,
  setTerminalAcceleratedRendering: terminalRepository.setTerminalAcceleratedRendering,
  terminalClearNotification: terminalRepository.terminalClearNotification,
  terminalCloseSession: terminalRepository.terminalCloseSession,
  terminalCloseWorkspaceSessions: terminalRepository.terminalCloseWorkspaceSessions,
  terminalCreateSession: terminalRepository.terminalCreateSession,
  terminalDrainOutput: terminalRepository.terminalDrainOutput,
  terminalGetRendererDiagnostics: terminalRepository.terminalGetRendererDiagnostics,
  terminalListNotifications: terminalRepository.terminalListNotifications,
  terminalListSessions: terminalRepository.terminalListSessions,
  terminalResize: terminalRepository.terminalResize,
  terminalResumeSession: terminalRepository.terminalResumeSession,
  terminalSetNotificationFocus: terminalRepository.terminalSetNotificationFocus,
  terminalWrite: terminalRepository.terminalWrite,
  terminalWriteBytes: terminalRepository.terminalWriteBytes,
  listenTerminalExit: terminalRepository.listenTerminalExit,
  listenTerminalForegroundChanged: terminalRepository.listenTerminalForegroundChanged,
  listenTerminalNotification: terminalRepository.listenTerminalNotification,
  listenTerminalNotificationCleared: terminalRepository.listenTerminalNotificationCleared,
  listenTerminalOutput: terminalRepository.listenTerminalOutput,
  writeCommandToNewSession: terminalRepository.writeCommandToNewSession,
  writeStoredLayoutMode,
};

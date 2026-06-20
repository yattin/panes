import * as ipcModule from "../../../lib/ipc";
import type { ThreadUpdatedEvent } from "../../../lib/ipc";
import type { ThreadGateway } from "../application/threadGateway";
import {
  clearLastActiveThreadId,
  readLastActiveThreadId,
  writeLastActiveThreadId,
} from "./threadPreferencesStorage";

const { ipc } = ipcModule;

type ThreadUpdatedUnlisten = () => void;

function listenThreadUpdated(
  onEvent: (event: ThreadUpdatedEvent) => void,
): Promise<ThreadUpdatedUnlisten> {
  const listener = (ipcModule as {
    listenThreadUpdated?: (
      onEvent: (event: ThreadUpdatedEvent) => void,
    ) => Promise<ThreadUpdatedUnlisten>;
  }).listenThreadUpdated;
  if (!listener) {
    return Promise.reject(new Error("listenThreadUpdated is unavailable."));
  }
  return listener(onEvent);
}

export const threadRepository = {
  archiveThread: ipc.archiveThread,
  attachCodexRemoteThread: ipc.attachCodexRemoteThread,
  attachOpenCodeRemoteSession: ipc.attachOpenCodeRemoteSession,
  compactCodexThread: ipc.compactCodexThread,
  createThread: ipc.createThread,
  forkCodexThread: ipc.forkCodexThread,
  listArchivedThreads: ipc.listArchivedThreads,
  listThreads: ipc.listThreads,
  renameThread: ipc.renameThread,
  restoreThread: ipc.restoreThread,
  rollbackCodexThread: ipc.rollbackCodexThread,
  listenThreadUpdated,
};

export const threadGateway: ThreadGateway = {
  archiveThread: threadRepository.archiveThread,
  attachCodexRemoteThread: threadRepository.attachCodexRemoteThread,
  attachOpenCodeRemoteSession: threadRepository.attachOpenCodeRemoteSession,
  clearLastActiveThreadId,
  compactCodexThread: threadRepository.compactCodexThread,
  createThread: threadRepository.createThread,
  forkCodexThread: threadRepository.forkCodexThread,
  listArchivedThreads: threadRepository.listArchivedThreads,
  listThreads: threadRepository.listThreads,
  readLastActiveThreadId,
  renameThread: threadRepository.renameThread,
  restoreThread: threadRepository.restoreThread,
  rollbackCodexThread: threadRepository.rollbackCodexThread,
  writeLastActiveThreadId,
};

import * as ipcModule from "../../../lib/ipc";
import type { StreamEvent } from "../../../types";
import type { ChatGateway, ChatTurnFinishedEvent } from "../application/chatGateway";
import { chatRuntime } from "./chatRuntime";
import { chatTelemetry } from "./chatTelemetry";

const { ipc } = ipcModule;

type ThreadEventUnlisten = () => void;
type ChatTurnFinishedUnlisten = () => void;

function listenThreadEvents(
  threadId: string,
  onEvent: (event: StreamEvent) => void,
): Promise<ThreadEventUnlisten> {
  const listener = (ipcModule as {
    listenThreadEvents?: (
      threadId: string,
      onEvent: (event: StreamEvent) => void,
    ) => Promise<ThreadEventUnlisten>;
  }).listenThreadEvents;
  if (!listener) {
    return Promise.reject(new Error("listenThreadEvents is unavailable."));
  }
  return listener(threadId, onEvent);
}

function listenChatTurnFinished(
  onEvent: (event: ChatTurnFinishedEvent) => void,
): Promise<ChatTurnFinishedUnlisten> {
  const listener = (ipcModule as {
    listenChatTurnFinished?: (
      onEvent: (event: ChatTurnFinishedEvent) => void,
    ) => Promise<ChatTurnFinishedUnlisten>;
  }).listenChatTurnFinished;
  if (!listener) {
    return Promise.reject(new Error("listenChatTurnFinished is unavailable."));
  }
  return listener(onEvent);
}

export const chatRepository = {
  cancelTurn: ipc.cancelTurn,
  compactNativeThread: ipc.compactNativeThread,
  confirmWorkspaceThread: ipc.confirmWorkspaceThread,
  getContextMaxTokens: ipc.getContextMaxTokens,
  getActionOutput: ipc.getActionOutput,
  getNativeHistoryTokens: ipc.getNativeHistoryTokens,
  getOpenCodeRuntimeCatalog: ipc.getOpenCodeRuntimeCatalog,
  getThreadMessagesWindow: ipc.getThreadMessagesWindow,
  listenChatTurnFinished,
  listenThreadEvents,
  listCodexApps: ipc.listCodexApps,
  listCodexRemoteThreads: ipc.listCodexRemoteThreads,
  listCodexSkills: ipc.listCodexSkills,
  listOpenCodeRemoteSessions: ipc.listOpenCodeRemoteSessions,
  prewarmEngine: ipc.prewarmEngine,
  readAttachmentPreview: ipc.readAttachmentPreview,
  respondApproval: ipc.respondApproval,
  savePastedImageAttachment: ipc.savePastedImageAttachment,
  searchMessages: ipc.searchMessages,
  sendMessage: ipc.sendMessage,
  setThreadCodexConfig: ipc.setThreadCodexConfig,
  setThreadExecutionPolicy: ipc.setThreadExecutionPolicy,
  setThreadOpenCodeConfig: ipc.setThreadOpenCodeConfig,
  setThreadReasoningEffort: ipc.setThreadReasoningEffort,
  startCodexReview: ipc.startCodexReview,
  steerMessage: ipc.steerMessage,
  syncThreadFromEngine: ipc.syncThreadFromEngine,
};

export const chatGateway: ChatGateway = {
  cancelTurn: chatRepository.cancelTurn,
  clearTimer: chatRuntime.clearTimer,
  compactNativeThread: chatRepository.compactNativeThread,
  confirmWorkspaceThread: chatRepository.confirmWorkspaceThread,
  createId: chatRuntime.createId,
  epochSecondsOrMillisecondsToIso: chatRuntime.epochSecondsOrMillisecondsToIso,
  getActionOutput: chatRepository.getActionOutput,
  getContextMaxTokens: chatRepository.getContextMaxTokens,
  getNativeHistoryTokens: chatRepository.getNativeHistoryTokens,
  getOpenCodeRuntimeCatalog: chatRepository.getOpenCodeRuntimeCatalog,
  getThreadMessagesWindow: chatRepository.getThreadMessagesWindow,
  listenChatTurnFinished: chatRepository.listenChatTurnFinished,
  listenThreadEvents: chatRepository.listenThreadEvents,
  listCodexApps: chatRepository.listCodexApps,
  listCodexRemoteThreads: chatRepository.listCodexRemoteThreads,
  listCodexSkills: chatRepository.listCodexSkills,
  listOpenCodeRemoteSessions: chatRepository.listOpenCodeRemoteSessions,
  nowIso: chatRuntime.nowIso,
  performanceNow: chatRuntime.performanceNow,
  prewarmEngine: chatRepository.prewarmEngine,
  readAttachmentPreview: chatRepository.readAttachmentPreview,
  recordMetric: chatTelemetry.recordMetric,
  respondApproval: chatRepository.respondApproval,
  scheduleAfterPaint: chatRuntime.scheduleAfterPaint,
  savePastedImageAttachment: chatRepository.savePastedImageAttachment,
  searchMessages: chatRepository.searchMessages,
  sendMessage: chatRepository.sendMessage,
  setThreadCodexConfig: chatRepository.setThreadCodexConfig,
  setThreadExecutionPolicy: chatRepository.setThreadExecutionPolicy,
  setThreadOpenCodeConfig: chatRepository.setThreadOpenCodeConfig,
  setThreadReasoningEffort: chatRepository.setThreadReasoningEffort,
  setTimer: chatRuntime.setTimer,
  startCodexReview: chatRepository.startCodexReview,
  steerMessage: chatRepository.steerMessage,
  syncThreadFromEngine: chatRepository.syncThreadFromEngine,
  wallClockNow: chatRuntime.wallClockNow,
};

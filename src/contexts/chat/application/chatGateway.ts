import type {
  ActionOutputPayload,
  ApprovalResponse,
  AttachmentPreview,
  ChatAttachment,
  ChatEngineId,
  ChatInputItem,
  CodexApp,
  CodexApprovalsReviewer,
  CodexReviewDelivery,
  CodexReviewTarget,
  CodexRemoteThreadPage,
  CodexSkill,
  MessageWindow,
  MessageWindowCursor,
  OpenCodeRemoteSessionPage,
  OpenCodeRuntimeCatalog,
  SearchResult,
  StreamEvent,
  Thread,
} from "../../../types";

export type ChatTimerHandle = ReturnType<typeof globalThis.setTimeout>;

export type ChatMetricName =
  | "chat.turn.first_shell.ms"
  | "chat.turn.first_content.ms"
  | "chat.turn.first_text.ms"
  | "chat.stream.flush.ms"
  | "chat.stream.events_per_sec"
  | "chat.markdown.worker.ms"
  | "chat.render.commit.ms";

export interface CodexRemoteThreadListOptions {
  cursor?: string | null;
  limit?: number | null;
  searchTerm?: string | null;
  archived?: boolean | null;
}

export interface OpenCodeRemoteSessionListOptions {
  cursor?: string | null;
  limit?: number | null;
  searchTerm?: string | null;
  archived?: boolean | null;
}

export interface ChatTurnFinishedEvent {
  threadId: string;
  workspaceId: string;
  engineId: ChatEngineId;
  threadTitle: string;
  status: "completed" | "interrupted" | "error";
  preview?: string | null;
}

export interface ThreadCodexConfigPatch {
  personality?: string | null;
  serviceTier?: string | null;
  outputSchema?: unknown;
}

export interface ThreadExecutionPolicyRequest {
  approvalPolicy?: unknown;
  sandboxMode?: string | null;
  allowNetwork?: boolean | null;
  permissionProfile?: Record<string, unknown> | null;
  approvalsReviewer?: CodexApprovalsReviewer | null;
}

export interface ThreadOpenCodeConfigPatch {
  agent?: string | null;
}

export interface ChatGateway {
  cancelTurn(threadId: string): Promise<void>;
  clearTimer(timer: ChatTimerHandle): void;
  compactNativeThread(engineThreadId: string): Promise<[number, number]>;
  confirmWorkspaceThread(threadId: string, writableRoots: string[]): Promise<void>;
  createId(): string;
  epochSecondsOrMillisecondsToIso(value: number): string | null;
  getActionOutput(messageId: string, actionId: string): Promise<ActionOutputPayload>;
  getContextMaxTokens(): Promise<number>;
  getNativeHistoryTokens(engineThreadId: string): Promise<number>;
  getOpenCodeRuntimeCatalog(cwd: string): Promise<OpenCodeRuntimeCatalog>;
  getThreadMessagesWindow(
    threadId: string,
    cursor?: MessageWindowCursor | null,
    limit?: number | null,
  ): Promise<MessageWindow>;
  listenThreadEvents(
    threadId: string,
    onEvent: (event: StreamEvent) => void,
  ): Promise<() => void>;
  listenChatTurnFinished(onEvent: (event: ChatTurnFinishedEvent) => void): Promise<() => void>;
  listCodexRemoteThreads(
    workspaceId: string,
    options?: CodexRemoteThreadListOptions,
  ): Promise<CodexRemoteThreadPage>;
  listCodexApps(): Promise<CodexApp[]>;
  listCodexSkills(cwd: string): Promise<CodexSkill[]>;
  listOpenCodeRemoteSessions(
    workspaceId: string,
    options?: OpenCodeRemoteSessionListOptions,
  ): Promise<OpenCodeRemoteSessionPage>;
  nowIso(): string;
  performanceNow(): number;
  prewarmEngine(engineId: string): Promise<void>;
  readAttachmentPreview(filePath: string, mimeType: string): Promise<AttachmentPreview | null>;
  recordMetric(name: ChatMetricName, value: number, meta?: Record<string, unknown>): void;
  respondApproval(
    threadId: string,
    approvalId: string,
    response: ApprovalResponse,
  ): Promise<void>;
  scheduleAfterPaint(callback: (timestamp: number) => void): void;
  savePastedImageAttachment(
    fileName: string,
    mimeType: string,
    dataBase64: string,
  ): Promise<ChatAttachment>;
  sendMessage(
    threadId: string,
    message: string,
    modelId?: string | null,
    reasoningEffort?: string | null,
    attachments?: ChatAttachment[] | null,
    inputItems?: ChatInputItem[] | null,
    planMode?: boolean | null,
    clientTurnId?: string | null,
  ): Promise<string>;
  setThreadCodexConfig(threadId: string, patch: ThreadCodexConfigPatch): Promise<Thread>;
  setThreadExecutionPolicy(
    threadId: string,
    patch: ThreadExecutionPolicyRequest,
  ): Promise<Thread>;
  setThreadOpenCodeConfig(threadId: string, patch: ThreadOpenCodeConfigPatch): Promise<Thread>;
  setThreadReasoningEffort(
    threadId: string,
    reasoningEffort: string | null,
    modelId?: string | null,
  ): Promise<void>;
  setTimer(callback: () => void, delayMs: number): ChatTimerHandle;
  searchMessages(workspaceId: string, query: string): Promise<SearchResult[]>;
  startCodexReview(
    threadId: string,
    target: CodexReviewTarget,
    delivery: CodexReviewDelivery,
  ): Promise<Thread>;
  steerMessage(
    threadId: string,
    message: string,
    attachments?: ChatAttachment[] | null,
    inputItems?: ChatInputItem[] | null,
    planMode?: boolean | null,
  ): Promise<void>;
  syncThreadFromEngine(threadId: string): Promise<Thread>;
  wallClockNow(): number;
}

let configuredChatGateway: ChatGateway | null = null;

export function configureChatGateway(gateway: ChatGateway): void {
  configuredChatGateway = gateway;
}

export function getChatGateway(): ChatGateway {
  if (!configuredChatGateway) {
    throw new Error("ChatGateway has not been configured.");
  }
  return configuredChatGateway;
}

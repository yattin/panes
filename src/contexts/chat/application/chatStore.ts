import { create } from "zustand";
import { useThreadStore } from "../../threads/application/threadStore";
import {
  type AssistantMessageTarget,
  appendSteerBlockToAssistantMessage,
  applyHydrationWindow,
  collapseTrailingSteerMessages,
  compactTrailingStreamingAssistantMessages,
  hasDeferredActionOutput,
  normalizeActionOutputStream,
  normalizeMessages,
  patchActionBlock,
  removeSteerBlock,
  resolveApprovalDecision,
  resolveApprovalInMessages,
  resolveAssistantMessageIndex,
  summarizeMessageForMemory,
  trimActionOutputChunks,
  upsertBlock,
  upsertNoticeBlock,
} from "../domain/chatMessages";
import {
  applyRuntimeStateFromEvent,
  enqueueStreamEvent,
  eventHasVisibleAssistantContent,
  isThreadTurnActive,
  mapUsageLimitsFromEvent,
} from "../domain/chatStreamEvents";
import { getChatGateway, type ChatTimerHandle } from "./chatGateway";
import type {
  ApprovalResponse,
  ActionBlock,
  ApprovalBlock,
  AttachmentBlock,
  ChatAttachment,
  ChatInputItem,
  ContentBlock,
  ContextUsage,
  MentionBlock,
  Message,
  MessageWindowCursor,
  SkillBlock,
  SteerBlock,
  StreamEvent,
  ThreadStatus
} from "../../../types";

interface ChatState {
  threadId: string | null;
  messages: Message[];
  olderCursor: MessageWindowCursor | null;
  hasOlderMessages: boolean;
  loadingOlderMessages: boolean;
  olderLoadBlockedUntil: number;
  status: ThreadStatus;
  streaming: boolean;
  usageLimits: ContextUsage | null;
  error?: string;
  unlisten?: () => void;
  setActiveThread: (threadId: string | null) => Promise<void>;
  loadOlderMessages: () => Promise<void>;
  send: (
    message: string,
    options?: {
      threadIdOverride?: string;
      modelId?: string | null;
      engineId?: string | null;
      reasoningEffort?: string | null;
      attachments?: ChatAttachment[];
      inputItems?: ChatInputItem[];
      planMode?: boolean;
    },
  ) => Promise<boolean>;
  steer: (
    message: string,
    options?: {
      threadIdOverride?: string;
      attachments?: ChatAttachment[];
      inputItems?: ChatInputItem[];
      planMode?: boolean;
    },
  ) => Promise<boolean>;
  cancel: () => Promise<void>;
  respondApproval: (approvalId: string, response: ApprovalResponse) => Promise<void>;
  hydrateActionOutput: (messageId: string, actionId: string) => Promise<void>;
}

let activeThreadBindSeq = 0;
const STREAM_EVENT_BATCH_WINDOW_MS = 16;
const STREAM_EVENT_QUEUE_FLUSH_THRESHOLD = 500;

/**
 * Background listeners for threads that are still streaming when the user switches away.
 * Keeps the event stream alive so events are not lost. On switch-back, the DB state
 * will include all events that arrived while the thread was in the background.
 * Cleaned up on TurnCompleted or when the thread is explicitly cancelled.
 */
const backgroundStreamListeners = new Map<string, () => void>();

function cleanupBackgroundListener(threadId: string) {
  const cleanup = backgroundStreamListeners.get(threadId);
  if (cleanup) {
    cleanup();
    backgroundStreamListeners.delete(threadId);
  }
}
const MESSAGE_WINDOW_INITIAL_LIMIT = 80;
const OLDER_MESSAGES_RETRY_BACKOFF_MS = 2_000;

interface PendingTurnMeta {
  turnEngineId?: string | null;
  turnModelId?: string | null;
  turnReasoningEffort?: string | null;
  clientTurnId?: string | null;
  assistantMessageId?: string | null;
  startedAt: number;
  firstShellRecorded: boolean;
  firstContentRecorded: boolean;
  firstTextRecorded: boolean;
}

const pendingTurnMetaByThread = new Map<string, PendingTurnMeta>();
const inflightActionOutputHydration = new Map<string, Promise<void>>();

function isCodexThreadSyncRequired(metadata: Record<string, unknown> | undefined): boolean {
  return metadata?.codexSyncRequired === true;
}

function recordPendingTurnMetric(
  threadId: string,
  flag: keyof Pick<
    PendingTurnMeta,
    "firstShellRecorded" | "firstContentRecorded" | "firstTextRecorded"
  >,
  metricName:
    | "chat.turn.first_shell.ms"
    | "chat.turn.first_content.ms"
    | "chat.turn.first_text.ms",
) {
  const pendingTurnMeta = pendingTurnMetaByThread.get(threadId);
  if (!pendingTurnMeta || pendingTurnMeta[flag]) {
    return;
  }

  pendingTurnMeta[flag] = true;
  getChatGateway().recordMetric(metricName, getChatGateway().performanceNow() - pendingTurnMeta.startedAt, {
    threadId,
    clientTurnId: pendingTurnMeta.clientTurnId ?? undefined,
    engineId: pendingTurnMeta.turnEngineId ?? undefined,
    modelId: pendingTurnMeta.turnModelId ?? undefined,
  });
}

function schedulePendingTurnShellMetric(threadId: string, clientTurnId: string) {
  getChatGateway().scheduleAfterPaint(() => {
    const pendingTurnMeta = pendingTurnMetaByThread.get(threadId);
    if (!pendingTurnMeta || pendingTurnMeta.clientTurnId !== clientTurnId) {
      return;
    }
    recordPendingTurnMetric(threadId, "firstShellRecorded", "chat.turn.first_shell.ms");
  });
}

function recordPendingTurnLatencyMetrics(threadId: string, event: StreamEvent) {
  if (eventHasVisibleAssistantContent(event)) {
    recordPendingTurnMetric(threadId, "firstContentRecorded", "chat.turn.first_content.ms");
  }

  if (event.type === "TextDelta" && String(event.content ?? "").length > 0) {
    recordPendingTurnMetric(threadId, "firstTextRecorded", "chat.turn.first_text.ms");
  }
}

function createStreamingAssistantMessage(
  threadId: string,
  options?: {
    id?: string;
    clientTurnId?: string | null;
  },
): Message {
  const pendingTurnMeta = pendingTurnMetaByThread.get(threadId);
  return {
    id: options?.id ?? getChatGateway().createId(),
    threadId,
    role: "assistant",
    clientTurnId: options?.clientTurnId ?? pendingTurnMeta?.clientTurnId ?? null,
    turnEngineId: pendingTurnMeta?.turnEngineId ?? null,
    turnModelId: pendingTurnMeta?.turnModelId ?? null,
    turnReasoningEffort: pendingTurnMeta?.turnReasoningEffort ?? null,
    status: "streaming",
    schemaVersion: 1,
    blocks: [],
    createdAt: getChatGateway().nowIso(),
    hydration: "full",
    hasDeferredContent: false,
  };
}

function createOptimisticUserMessage(
  threadId: string,
  message: string,
  options?: {
    attachments?: ChatAttachment[];
    inputItems?: ChatInputItem[];
    planMode?: boolean;
  },
): Message {
  const attachments = options?.attachments ?? [];
  const inputItems = options?.inputItems ?? [];
  const planMode = options?.planMode ?? false;
  const userBlocks: ContentBlock[] = [];

  for (const inputItem of inputItems) {
    if (inputItem.type === "skill") {
      userBlocks.push({
        type: "skill",
        name: inputItem.name,
        path: inputItem.path,
      });
    } else if (inputItem.type === "mention") {
      userBlocks.push({
        type: "mention",
        name: inputItem.name,
        path: inputItem.path,
      });
    }
  }

  for (const attachment of attachments) {
    userBlocks.push({
      type: "attachment",
      fileName: attachment.fileName,
      filePath: attachment.filePath,
      sizeBytes: attachment.sizeBytes,
      mimeType: attachment.mimeType,
    });
  }

  userBlocks.push({ type: "text", content: message, planMode: planMode || undefined });

  return {
    id: getChatGateway().createId(),
    threadId,
    role: "user",
    content: message,
    blocks: userBlocks,
    status: "completed",
    schemaVersion: 1,
    createdAt: getChatGateway().nowIso(),
    hydration: "full",
    hasDeferredContent: false,
  };
}

function createSteerBlock(
  message: string,
  options?: {
    attachments?: ChatAttachment[];
    inputItems?: ChatInputItem[];
    planMode?: boolean;
    steerId?: string;
  },
): SteerBlock {
  const attachments = (options?.attachments ?? []).map<AttachmentBlock>((attachment) => ({
    type: "attachment",
    fileName: attachment.fileName,
    filePath: attachment.filePath,
    sizeBytes: attachment.sizeBytes,
    mimeType: attachment.mimeType,
  }));
  const skills: SkillBlock[] = [];
  const mentions: MentionBlock[] = [];

  for (const inputItem of options?.inputItems ?? []) {
    if (inputItem.type === "skill") {
      skills.push({
        type: "skill",
        name: inputItem.name,
        path: inputItem.path,
      });
    } else if (inputItem.type === "mention") {
      mentions.push({
        type: "mention",
        name: inputItem.name,
        path: inputItem.path,
      });
    }
  }

  return {
    type: "steer",
    steerId: options?.steerId ?? getChatGateway().createId(),
    content: message,
    planMode: options?.planMode || undefined,
    attachments: attachments.length > 0 ? attachments : undefined,
    skills: skills.length > 0 ? skills : undefined,
    mentions: mentions.length > 0 ? mentions : undefined,
  };
}

function resolveActiveAssistantTarget(threadId: string): AssistantMessageTarget {
  const pendingTurnMeta = pendingTurnMetaByThread.get(threadId);
  return {
    clientTurnId: pendingTurnMeta?.clientTurnId ?? null,
    assistantMessageId: pendingTurnMeta?.assistantMessageId ?? null,
  };
}

function appendSteerBlockToActiveAssistant(
  messages: Message[],
  threadId: string,
  steerBlock: SteerBlock,
): Message[] {
  const { messages: ensuredMessages, assistantIndex } = ensureAssistantMessage(
    messages,
    threadId,
    resolveActiveAssistantTarget(threadId),
  );
  const assistant = ensuredMessages[assistantIndex];
  const nextAssistant = appendSteerBlockToAssistantMessage(assistant, steerBlock);
  if (nextAssistant === assistant) {
    return ensuredMessages;
  }

  return [
    ...ensuredMessages.slice(0, assistantIndex),
    nextAssistant,
    ...ensuredMessages.slice(assistantIndex + 1),
  ];
}

function isThreadStatusStreaming(status: ThreadStatus): boolean {
  return isThreadTurnActive(status);
}

function ensureAssistantMessage(
  messages: Message[],
  threadId: string,
  target: AssistantMessageTarget,
): { messages: Message[]; assistantIndex: number } {
  const compactedMessages = compactTrailingStreamingAssistantMessages(messages, target);
  const existingIndex = resolveAssistantMessageIndex(compactedMessages, target);
  if (existingIndex >= 0) {
    return {
      messages: compactedMessages,
      assistantIndex: existingIndex,
    };
  }

  const assistantMessage = createStreamingAssistantMessage(threadId, {
    id: target.assistantMessageId ?? undefined,
    clientTurnId: target.clientTurnId ?? null,
  });
  return {
    messages: [...compactedMessages, assistantMessage],
    assistantIndex: compactedMessages.length,
  };
}

function resolveAssistantTargetFromEvent(
  threadId: string,
  event: StreamEvent,
): AssistantMessageTarget {
  const pendingTurnMeta = pendingTurnMetaByThread.get(threadId);
  const eventClientTurnId =
    event.type === "TurnStarted" && typeof event.client_turn_id === "string"
      ? event.client_turn_id
      : null;

  return {
    clientTurnId: eventClientTurnId ?? pendingTurnMeta?.clientTurnId ?? null,
    assistantMessageId:
      eventClientTurnId && pendingTurnMeta?.clientTurnId === eventClientTurnId
        ? pendingTurnMeta.assistantMessageId ?? null
        : pendingTurnMeta?.assistantMessageId ?? null,
  };
}

function applyStreamEvent(messages: Message[], event: StreamEvent, threadId: string): Message[] {
  if (event.type === "UsageLimitsUpdated") {
    return messages;
  }

  if (event.type === "ApprovalResolved") {
    return resolveApprovalInMessages(messages, String(event.approval_id ?? ""));
  }

  const assistantTarget = resolveAssistantTargetFromEvent(threadId, event);
  const { messages: ensuredMessages, assistantIndex } = ensureAssistantMessage(
    messages,
    threadId,
    assistantTarget,
  );
  let next = ensuredMessages;
  const currentAssistant = next[assistantIndex];
  const assistant: Message = { ...currentAssistant };
  const existingBlocks = currentAssistant.blocks ?? [];
  assistant.blocks = existingBlocks;

  if (event.type === "TurnStarted" && typeof event.client_turn_id === "string") {
    assistant.clientTurnId = event.client_turn_id;
  }

  // Stamp durationMs on the last thinking block when a non-thinking event arrives
  if (event.type !== "ThinkingDelta") {
    const blocks = assistant.blocks ?? [];
    const last = blocks[blocks.length - 1];
    if (last?.type === "thinking" && last.startedAt != null && last.durationMs == null) {
      assistant.blocks = [
        ...blocks.slice(0, -1),
        { ...last, durationMs: getChatGateway().wallClockNow() - last.startedAt },
      ];
    }
  }

  if (event.type === "TextDelta") {
    const blocks = assistant.blocks ?? [];
    const delta = String(event.content ?? "");
    if (!delta) {
      return next;
    }
    const last = blocks[blocks.length - 1];
    if (last?.type === "text") {
      assistant.blocks = [
        ...blocks.slice(0, -1),
        {
          ...last,
          content: `${last.content}${delta}`,
        },
      ];
    } else {
      assistant.blocks = [...blocks, { type: "text", content: delta }];
    }
  }

  if (event.type === "ThinkingDelta") {
    const blocks = assistant.blocks ?? [];
    const delta = String(event.content ?? "");
    if (!delta) {
      return next;
    }
    const last = blocks[blocks.length - 1];
    if (last?.type === "thinking") {
      assistant.blocks = [
        ...blocks.slice(0, -1),
        {
          ...last,
          content: `${last.content}${delta}`,
        },
      ];
    } else {
      assistant.blocks = [
        ...blocks,
        { type: "thinking" as const, content: delta, startedAt: getChatGateway().wallClockNow() },
      ];
    }
  }

  if (event.type === "ActionStarted") {
    const blocks = assistant.blocks ?? [];
    assistant.blocks = upsertBlock(blocks, {
      type: "action",
      actionId: String(event.action_id),
      engineActionId: event.engine_action_id as string | undefined,
      actionType: String(event.action_type ?? "other") as ActionBlock["actionType"],
      summary: String(event.summary ?? ""),
      displayLabel:
        typeof event.display_label === "string" && event.display_label.trim()
          ? event.display_label
          : undefined,
      displaySubtitle:
        typeof event.display_subtitle === "string" && event.display_subtitle.trim()
          ? event.display_subtitle
          : undefined,
      details: (event.details as Record<string, unknown>) ?? {},
      outputChunks: [],
      outputDeferred: false,
      outputDeferredLoaded: true,
      status: "running"
    });
  }

  if (event.type === "ActionOutputDelta") {
    const actionId = String(event.action_id ?? "");
    const stream = normalizeActionOutputStream(event.stream);
    const content = String(event.content ?? "");
    if (actionId && content) {
      const blocks = assistant.blocks ?? [];
      assistant.blocks = patchActionBlock(blocks, actionId, (block) => {
        const details = (block.details ?? {}) as Record<string, unknown>;
        const previousChunk = block.outputChunks[block.outputChunks.length - 1];
        const mergedChunks =
          previousChunk && previousChunk.stream === stream
            ? [
                ...block.outputChunks.slice(0, -1),
                {
                  ...previousChunk,
                  content: `${previousChunk.content}${content}`,
                },
              ]
            : [
                ...block.outputChunks,
                {
                  stream,
                  content,
                },
              ];
        const { chunks: nextOutputChunks, truncated } = trimActionOutputChunks(mergedChunks);
        const shouldMarkTruncated =
          truncated &&
          !("outputTruncated" in details && details.outputTruncated === true);
        const nextDetails = shouldMarkTruncated
          ? {
              ...details,
              outputTruncated: true,
            }
          : details;

        if (nextOutputChunks === block.outputChunks && nextDetails === block.details) {
          return block;
        }

        return {
          ...block,
          outputChunks: nextOutputChunks,
          details: nextDetails,
          outputDeferred: false,
          outputDeferredLoaded: true,
        };
      });
    }
  }

  if (event.type === "ActionProgressUpdated") {
    const actionId = String(event.action_id ?? "");
    const progressMessage = String(event.message ?? "");
    if (actionId && progressMessage) {
      const blocks = assistant.blocks ?? [];
      assistant.blocks = patchActionBlock(blocks, actionId, (block) => {
        const details = (block.details ?? {}) as Record<string, unknown>;
        if (
          details.progressKind === "mcp" &&
          typeof details.progressMessage === "string" &&
          details.progressMessage === progressMessage
        ) {
          return block;
        }

        return {
          ...block,
          details: {
            ...details,
            progressKind: "mcp",
            progressMessage,
          },
        };
      });
    }
  }

  if (event.type === "ActionCompleted") {
    const blocks = assistant.blocks ?? [];
    const actionId = String(event.action_id ?? "");
    assistant.blocks = patchActionBlock(blocks, actionId, (block) => {
      const result = (event.result as Record<string, unknown> | undefined) ?? {};
      return {
        ...block,
        status: result.success ? "done" : "error",
        result: {
          success: Boolean(result.success),
          output: result.output as string | undefined,
          error: result.error as string | undefined,
          diff: result.diff as string | undefined,
          durationMs: Number(result.durationMs ?? result.duration_ms ?? 0)
        }
      };
    });
  }

  if (event.type === "ApprovalRequested") {
    const blocks = assistant.blocks ?? [];
    assistant.blocks = upsertBlock(blocks, {
      type: "approval",
      approvalId: String(event.approval_id),
      actionType: String(event.action_type ?? "other") as ApprovalBlock["actionType"],
      summary: String(event.summary ?? ""),
      details: (event.details as Record<string, unknown>) ?? {},
      status: "pending"
    });
  }

  if (event.type === "DiffUpdated") {
    const blocks = assistant.blocks ?? [];
    const scope = String(event.scope ?? "turn") as "turn" | "file" | "workspace";
    const diff = String(event.diff ?? "");

    let existingDiffIndex = -1;
    for (let index = blocks.length - 1; index >= 0; index -= 1) {
      const block = blocks[index];
      if (block.type === "diff" && block.scope === scope) {
        existingDiffIndex = index;
        break;
      }
    }

    if (existingDiffIndex >= 0) {
      const existingBlock = blocks[existingDiffIndex];
      if (existingBlock.type === "diff") {
        const nextBlocks: ContentBlock[] = [];
        blocks.forEach((block, index) => {
          if (block.type === "diff" && block.scope === scope) {
            if (index === existingDiffIndex) {
              nextBlocks.push({
                ...existingBlock,
                diff,
              });
            }
            return;
          }
          nextBlocks.push(block);
        });
        if (nextBlocks.length !== blocks.length || existingBlock.diff !== diff) {
          assistant.blocks = nextBlocks;
        }
      }
    } else {
      assistant.blocks = [
        ...blocks,
        {
          type: "diff",
          diff,
          scope,
        },
      ];
    }
  }

  if (event.type === "ModelRerouted") {
    const fromModel = String(event.from_model ?? "").trim();
    const toModel = String(event.to_model ?? "").trim();
    const reason = String(event.reason ?? "").trim();
    if (toModel) {
      assistant.turnModelId = toModel;
      assistant.blocks = upsertNoticeBlock(assistant.blocks ?? [], {
        type: "notice",
        kind: "model_rerouted",
        level: "info",
        title: "Model rerouted",
        message: `Switched from ${fromModel || "the requested model"} to ${toModel}${reason ? ` (${reason})` : ""}.`,
      });
    }
  }

  if (event.type === "Notice") {
    assistant.blocks = upsertNoticeBlock(assistant.blocks ?? [], {
      type: "notice",
      kind: String(event.kind ?? "notice"),
      level:
        event.level === "warning" || event.level === "error"
          ? event.level
          : "info",
      title: String(event.title ?? "Notice"),
      message: String(event.message ?? ""),
    });
  }

  if (event.type === "Error") {
    const blocks = assistant.blocks ?? [];
    assistant.blocks = [...blocks, { type: "error", message: String(event.message ?? "Unknown error") }];
    if (!event.recoverable) {
      assistant.status = "error";
    }
  }

  if (event.type === "TurnCompleted") {
    const status = String(event.status ?? "completed");
    if (status === "failed") {
      assistant.status = "error";
    } else if (status === "interrupted") {
      assistant.status = "interrupted";
    } else {
      assistant.status = "completed";
    }
  }

  assistant.hydration = "full";
  assistant.hasDeferredContent = hasDeferredActionOutput(assistant.blocks);

  const blocksChanged = assistant.blocks !== existingBlocks;
  const statusChanged = assistant.status !== currentAssistant.status;
  const metadataChanged =
    assistant.clientTurnId !== currentAssistant.clientTurnId ||
    assistant.turnEngineId !== currentAssistant.turnEngineId ||
    assistant.turnModelId !== currentAssistant.turnModelId ||
    assistant.turnReasoningEffort !== currentAssistant.turnReasoningEffort ||
    assistant.hydration !== currentAssistant.hydration ||
    assistant.hasDeferredContent !== currentAssistant.hasDeferredContent;

  if (!blocksChanged && !statusChanged && !metadataChanged) {
    return next;
  }

  next = [
    ...next.slice(0, assistantIndex),
    assistant,
    ...next.slice(assistantIndex + 1),
  ];
  return next;
}

export const useChatStore = create<ChatState>((set, get) => ({
  threadId: null,
  messages: [],
  olderCursor: null,
  hasOlderMessages: false,
  loadingOlderMessages: false,
  olderLoadBlockedUntil: 0,
  status: "idle",
  streaming: false,
  usageLimits: null,
  setActiveThread: async (threadId) => {
    const currentThreadId = get().threadId;
    const currentUnlisten = get().unlisten;
    if (threadId && threadId === currentThreadId && currentUnlisten) {
      return;
    }

    activeThreadBindSeq += 1;
    const bindSeq = activeThreadBindSeq;

    // Tear down the current listener. If the thread was still streaming,
    // install a lightweight background listener that watches for TurnCompleted
    // so the thread status updates correctly when the user switches back.
    if (currentUnlisten) {
      currentUnlisten();
    }
    if (currentThreadId && get().streaming) {
      cleanupBackgroundListener(currentThreadId);
      getChatGateway().listenThreadEvents(currentThreadId, (event) => {
        if (event.type === "TurnCompleted") {
          cleanupBackgroundListener(currentThreadId!);
        }
      }).then((unsub) => {
        // If the user already switched back to this thread, don't register
        if (useChatStore.getState().threadId === currentThreadId) {
          unsub();
          return;
        }
        const existing = backgroundStreamListeners.get(currentThreadId!);
        if (existing) {
          // Another background listener was set up in the meantime
          unsub();
        } else {
          backgroundStreamListeners.set(currentThreadId!, unsub);
        }
      });
    }

    if (!threadId) {
      if (bindSeq !== activeThreadBindSeq) {
        return;
      }

      set({
        threadId: null,
        messages: [],
        olderCursor: null,
        hasOlderMessages: false,
        loadingOlderMessages: false,
        olderLoadBlockedUntil: 0,
        streaming: false,
        status: "idle",
        usageLimits: null,
        unlisten: undefined,
      });
      return;
    }

    try {
      // Clean up any background listener for this thread before re-subscribing
      cleanupBackgroundListener(threadId);

      const threadState = useThreadStore.getState();
      let activeThread = threadState.threads.find((thread) => thread.id === threadId);
      if (activeThread?.engineId === "codex" && isCodexThreadSyncRequired(activeThread.engineMetadata)) {
        try {
          const syncedThread = await getChatGateway().syncThreadFromEngine(threadId);
          threadState.applyThreadUpdateLocal(syncedThread);
          activeThread = syncedThread;
        } catch (error) {
          console.warn(`Failed to sync Codex thread ${threadId}:`, error);
        }
      }

      const messageWindow = await getChatGateway().getThreadMessagesWindow(
        threadId,
        null,
        MESSAGE_WINDOW_INITIAL_LIMIT,
      );
      let messages = normalizeMessages(messageWindow.messages);
      const olderCursor = messageWindow.nextCursor;
      messages = applyHydrationWindow(messages);
      if (bindSeq !== activeThreadBindSeq) {
        return;
      }

      const queuedStreamEvents: StreamEvent[] = [];
      let streamFlushTimer: ChatTimerHandle | null = null;
      let streamFlushInProgress = false;
      let eventRateWindowStartedAt = getChatGateway().performanceNow();
      let eventRateWindowCount = 0;

      const emitEventRateMetric = (now: number) => {
        const elapsedMs = now - eventRateWindowStartedAt;
        if (elapsedMs <= 0 || eventRateWindowCount <= 0) {
          eventRateWindowStartedAt = now;
          eventRateWindowCount = 0;
          return;
        }
        const eventsPerSecond = (eventRateWindowCount * 1000) / elapsedMs;
        getChatGateway().recordMetric("chat.stream.events_per_sec", eventsPerSecond, {
          threadId,
          events: eventRateWindowCount,
          windowMs: elapsedMs,
        });
        eventRateWindowStartedAt = now;
        eventRateWindowCount = 0;
      };

      const flushQueuedStreamEvents = () => {
        if (streamFlushInProgress) {
          return;
        }
        if (streamFlushTimer !== null) {
          getChatGateway().clearTimer(streamFlushTimer);
          streamFlushTimer = null;
        }
        if (queuedStreamEvents.length === 0) {
          return;
        }

        streamFlushInProgress = true;
        const batch = queuedStreamEvents.splice(0, queuedStreamEvents.length);
        const flushStartedAt = getChatGateway().performanceNow();
        try {
          set((state) => {
            if (bindSeq !== activeThreadBindSeq || state.threadId !== threadId) {
              return state;
            }

            let nextMessages = state.messages;
            let nextStreaming = state.streaming;
            let nextStatus = state.status;
            let nextUsageLimits = state.usageLimits;
            let hydrationRecalcRequired = false;
            for (const queuedEvent of batch) {
              if (queuedEvent.type === "UsageLimitsUpdated") {
                nextUsageLimits = mapUsageLimitsFromEvent(
                  queuedEvent,
                  getChatGateway().epochSecondsOrMillisecondsToIso,
                );
                continue;
              }
              const previousLength = nextMessages.length;
              nextMessages = applyStreamEvent(nextMessages, queuedEvent, state.threadId);
              if (nextMessages.length !== previousLength) {
                hydrationRecalcRequired = true;
              }
              const nextRuntimeState = applyRuntimeStateFromEvent(
                nextStatus,
                nextStreaming,
                queuedEvent,
              );
              nextStatus = nextRuntimeState.status;
              nextStreaming = nextRuntimeState.streaming;
              if (queuedEvent.type === "TurnCompleted") {
                pendingTurnMetaByThread.delete(threadId);
              }
            }
            if (hydrationRecalcRequired) {
              nextMessages = applyHydrationWindow(nextMessages);
            }

            if (
              nextMessages === state.messages &&
              nextStatus === state.status &&
              nextStreaming === state.streaming &&
              nextUsageLimits === state.usageLimits
            ) {
              return state;
            }

            return {
              ...state,
              messages: nextMessages,
              status: nextStatus,
              streaming: nextStreaming,
              usageLimits: nextUsageLimits,
            };
          });
        } finally {
          streamFlushInProgress = false;
        }
        getChatGateway().recordMetric("chat.stream.flush.ms", getChatGateway().performanceNow() - flushStartedAt, {
          threadId,
          batchSize: batch.length,
        });

        if (queuedStreamEvents.length > 0) {
          scheduleStreamFlush();
        }
      };

      const scheduleStreamFlush = () => {
        if (streamFlushTimer !== null) {
          return;
        }
        streamFlushTimer = getChatGateway().setTimer(() => {
          streamFlushTimer = null;
          flushQueuedStreamEvents();
        }, STREAM_EVENT_BATCH_WINDOW_MS);
      };

      const unlistenStream = await getChatGateway().listenThreadEvents(threadId, (event) => {
        if (bindSeq !== activeThreadBindSeq) {
          return;
        }
        const pendingTurnMeta = pendingTurnMetaByThread.get(threadId);
        if (
          pendingTurnMeta &&
          event.type === "TurnStarted" &&
          typeof event.client_turn_id === "string" &&
          event.client_turn_id.length > 0
        ) {
          pendingTurnMeta.clientTurnId = event.client_turn_id;
        }
        if (pendingTurnMeta && event.type === "ModelRerouted") {
          const reroutedModelId =
            typeof event.to_model === "string" ? event.to_model.trim() : "";
          if (reroutedModelId) {
            pendingTurnMeta.turnModelId = reroutedModelId;
          }
        }
        recordPendingTurnLatencyMetrics(threadId, event);
        enqueueStreamEvent(queuedStreamEvents, event);
        eventRateWindowCount += 1;
        const now = getChatGateway().performanceNow();
        if (now - eventRateWindowStartedAt >= 1000) {
          emitEventRateMetric(now);
        }
        if (event.type === "TurnCompleted") {
          flushQueuedStreamEvents();
          emitEventRateMetric(getChatGateway().performanceNow());
          return;
        }
        if (queuedStreamEvents.length >= STREAM_EVENT_QUEUE_FLUSH_THRESHOLD) {
          flushQueuedStreamEvents();
          return;
        }
        scheduleStreamFlush();
      });

      const unlisten = () => {
        if (streamFlushTimer !== null) {
          getChatGateway().clearTimer(streamFlushTimer);
          streamFlushTimer = null;
        }
        queuedStreamEvents.length = 0;
        emitEventRateMetric(getChatGateway().performanceNow());
        unlistenStream();
      };

      if (bindSeq !== activeThreadBindSeq) {
        unlisten();
        return;
      }

      const threadStatus = activeThread?.status ?? "idle";
      const currentState = get();
      if (currentState.threadId === threadId && currentState.streaming) {
        if (currentState.unlisten) {
          unlisten();
        }
        set({
          olderCursor,
          hasOlderMessages: olderCursor !== null,
          loadingOlderMessages: false,
          olderLoadBlockedUntil: 0,
          unlisten: currentState.unlisten ?? unlisten,
          error: undefined,
        });
        return;
      }

      set({
        threadId,
        messages,
        olderCursor,
        hasOlderMessages: olderCursor !== null,
        loadingOlderMessages: false,
        olderLoadBlockedUntil: 0,
        unlisten,
        error: undefined,
        streaming: isThreadStatusStreaming(threadStatus),
        status: threadStatus,
        usageLimits: null,
      });
    } catch (error) {
      if (bindSeq !== activeThreadBindSeq) {
        return;
      }
      set({
        threadId,
        messages: [],
        olderCursor: null,
        hasOlderMessages: false,
        loadingOlderMessages: false,
        olderLoadBlockedUntil: 0,
        usageLimits: null,
        error: String(error),
      });
    }
  },
  loadOlderMessages: async () => {
    const state = get();
    const threadId = state.threadId;
    const cursor = state.olderCursor;
    if (
      !threadId ||
      !cursor ||
      state.loadingOlderMessages ||
      state.olderLoadBlockedUntil > getChatGateway().wallClockNow()
    ) {
      return;
    }

    set((current) => {
      if (
        current.threadId !== threadId ||
        current.loadingOlderMessages ||
        current.olderCursor !== cursor
      ) {
        return current;
      }
      return {
        ...current,
        loadingOlderMessages: true,
      };
    });

    try {
      const olderWindow = await getChatGateway().getThreadMessagesWindow(
        threadId,
        cursor,
        MESSAGE_WINDOW_INITIAL_LIMIT,
      );
      const olderMessages = normalizeMessages(olderWindow.messages, {
        collapseTrailingSteers: false,
      }).map((message) =>
        summarizeMessageForMemory(message),
      );
      set((current) => {
        if (current.threadId !== threadId) {
          return current;
        }
        const nextCursor = olderWindow.nextCursor;
        const mergedMessages = collapseTrailingSteerMessages([
          ...olderMessages,
          ...current.messages,
        ]);
        return {
          ...current,
          messages: applyHydrationWindow(mergedMessages),
          olderCursor: nextCursor,
          hasOlderMessages: nextCursor !== null,
          loadingOlderMessages: false,
          olderLoadBlockedUntil: 0,
        };
      });
    } catch (error) {
      const retryAt = getChatGateway().wallClockNow() + OLDER_MESSAGES_RETRY_BACKOFF_MS;
      set((current) => {
        if (current.threadId !== threadId) {
          return current;
        }
        return {
          ...current,
          loadingOlderMessages: false,
          olderLoadBlockedUntil: retryAt,
          error: String(error),
        };
      });
    }
  },
  send: async (message, options) => {
    const state = get();
    if (state.streaming) {
      set({ error: "A turn is already in progress for this thread." });
      return false;
    }

    const threadId = options?.threadIdOverride ?? state.threadId;
    if (!threadId) {
      set({ error: "No active thread selected" });
      return false;
    }
    const startedAt = getChatGateway().performanceNow();
    const clientTurnId = getChatGateway().createId();
    const optimisticAssistantMessageId = getChatGateway().createId();
    pendingTurnMetaByThread.set(threadId, {
      turnEngineId: options?.engineId ?? null,
      turnModelId: options?.modelId ?? null,
      turnReasoningEffort: options?.reasoningEffort ?? null,
      clientTurnId,
      assistantMessageId: optimisticAssistantMessageId,
      startedAt,
      firstShellRecorded: false,
      firstContentRecorded: false,
      firstTextRecorded: false,
    });

    const attachments = options?.attachments ?? [];
    const inputItems = options?.inputItems ?? [];
    const planMode = options?.planMode ?? false;
    const userMessage = createOptimisticUserMessage(threadId, message, {
      attachments,
      inputItems,
      planMode,
    });
    const optimisticAssistantMessage = createStreamingAssistantMessage(threadId, {
      id: optimisticAssistantMessageId,
      clientTurnId,
    });

    set((state) => ({
      messages: applyHydrationWindow([
        ...state.messages,
        userMessage,
        optimisticAssistantMessage,
      ]),
      status: "streaming",
      streaming: true,
      error: undefined
    }));
    schedulePendingTurnShellMetric(threadId, clientTurnId);

    try {
      await getChatGateway().sendMessage(
        threadId,
        message,
        options?.modelId ?? null,
        options?.reasoningEffort ?? null,
        attachments.length > 0 ? attachments : null,
        inputItems.length > 0 ? inputItems : null,
        planMode,
        clientTurnId,
      );
      return true;
    } catch (error) {
      pendingTurnMetaByThread.delete(threadId);
      set((state) => ({
        messages: state.messages.filter(
          (item) => item.id !== userMessage.id && item.id !== optimisticAssistantMessage.id,
        ),
        status: "error",
        streaming: false,
        error: String(error),
      }));
      return false;
    }
  },
  steer: async (message, options) => {
    const state = get();
    if (!state.streaming) {
      set({ error: "No turn is currently in progress for this thread." });
      return false;
    }

    const threadId = options?.threadIdOverride ?? state.threadId;
    if (!threadId) {
      set({ error: "No active thread selected" });
      return false;
    }

    if (options?.threadIdOverride && options.threadIdOverride !== state.threadId) {
      set({ error: "Cannot steer a thread that is not currently active" });
      return false;
    }

    const attachments = options?.attachments ?? [];
    const inputItems = options?.inputItems ?? [];
    const planMode = options?.planMode ?? false;
    const steerBlock = createSteerBlock(message, {
      attachments,
      inputItems,
      planMode,
    });

    set((current) => ({
      messages: applyHydrationWindow(
        appendSteerBlockToActiveAssistant(current.messages, threadId, steerBlock),
      ),
      error: undefined,
    }));

    try {
      await getChatGateway().steerMessage(
        threadId,
        message,
        attachments.length > 0 ? attachments : null,
        inputItems.length > 0 ? inputItems : null,
        planMode,
      );
      return true;
    } catch (error) {
      set((current) => ({
        messages: applyHydrationWindow(removeSteerBlock(current.messages, steerBlock.steerId)),
        error: String(error),
      }));
      return false;
    }
  },
  cancel: async () => {
    const threadId = get().threadId;
    if (!threadId) {
      return;
    }

    try {
      await getChatGateway().cancelTurn(threadId);
      pendingTurnMetaByThread.delete(threadId);
      // Remove the trailing assistant message if it has no meaningful content
      // (e.g. only thinking blocks with no text, or completely empty)
      const messages = get().messages;
      const last = messages[messages.length - 1];
      const lastHasContent = last?.role === "assistant" && (last.blocks ?? []).some((b) => {
        if (b.type === "text") return Boolean(b.content?.trim());
        if (b.type === "action" || b.type === "diff" || b.type === "code" || b.type === "approval") return true;
        return false;
      });
      const nextMessages = last?.role === "assistant" && !lastHasContent
        ? messages.slice(0, -1)
        : messages;
      set({ status: "idle", streaming: false, messages: nextMessages });
    } catch (error) {
      set({ error: String(error) });
    }
  },
  respondApproval: async (approvalId, response) => {
    const threadId = get().threadId;
    if (!threadId) {
      set({ error: "No active thread selected" });
      return;
    }

    // Apply optimistic update BEFORE the IPC call
    const decision = resolveApprovalDecision(response);
    const responseData = typeof response === "object" && response !== null && !Array.isArray(response)
      ? response as Record<string, unknown>
      : undefined;
    const previousMessages = get().messages;
    set((state) => {
      const nextMessages = resolveApprovalInMessages(state.messages, approvalId, decision, responseData);
      if (nextMessages === state.messages) {
        return state;
      }
      return { ...state, messages: nextMessages };
    });

    try {
      await getChatGateway().respondApproval(threadId, approvalId, response);
    } catch (error) {
      // Roll back the optimistic update on failure
      set({ messages: previousMessages, error: String(error) });
    }
  },
  hydrateActionOutput: async (messageId, actionId) => {
    const requestKey = `${messageId}::${actionId}`;
    const existingRequest = inflightActionOutputHydration.get(requestKey);
    if (existingRequest) {
      await existingRequest;
      return;
    }

    const request = (async () => {
      const payload = await getChatGateway().getActionOutput(messageId, actionId);
      if (!payload.found) {
        throw new Error("Action output not found.");
      }

      const normalizedChunks: ActionBlock["outputChunks"] = payload.outputChunks.map((chunk) => ({
        stream: normalizeActionOutputStream(chunk.stream),
        content: String(chunk.content ?? ""),
      }));
      const { chunks: trimmedChunks, truncated: trimmedByFrontend } =
        trimActionOutputChunks(normalizedChunks);

      set((state) => {
        const messageIndex = state.messages.findIndex((message) => message.id === messageId);
        if (messageIndex < 0) {
          return state;
        }

        const message = state.messages[messageIndex];
        const blocks = message.blocks;
        if (!blocks || blocks.length === 0) {
          return state;
        }

        const nextBlocks = patchActionBlock(blocks, actionId, (block) => {
          if (
            block.outputDeferred !== true &&
            block.outputDeferredLoaded === true &&
            block.outputChunks.length > 0
          ) {
            return block;
          }

          const details = (block.details ?? {}) as Record<string, unknown>;
          const shouldMarkTruncated =
            (payload.truncated || trimmedByFrontend) &&
            !("outputTruncated" in details && details.outputTruncated === true);
          const nextDetails = shouldMarkTruncated
            ? {
                ...details,
                outputTruncated: true,
              }
            : details;

          return {
            ...block,
            details: nextDetails,
            outputChunks: trimmedChunks,
            outputDeferred: false,
            outputDeferredLoaded: true,
          };
        });

        if (nextBlocks === blocks) {
          return state;
        }

        const nextMessages = [...state.messages];
        nextMessages[messageIndex] = {
          ...message,
          blocks: nextBlocks,
          hasDeferredContent: hasDeferredActionOutput(nextBlocks),
        };

        return {
          ...state,
          messages: nextMessages,
        };
      });
    })();

    inflightActionOutputHydration.set(requestKey, request);
    try {
      await request;
    } finally {
      inflightActionOutputHydration.delete(requestKey);
    }
  },
}));

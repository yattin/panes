import type {
  ActionBlock,
  ApprovalBlock,
  ApprovalResponse,
  AttachmentBlock,
  ContentBlock,
  MentionBlock,
  Message,
  NoticeBlock,
  SkillBlock,
  SteerBlock,
} from "../../../types";

const MAX_FULLY_HYDRATED_MESSAGES = 80;
const ACTION_OUTPUT_MAX_CHARS = 80_000;
const ACTION_OUTPUT_TRIM_TARGET_CHARS = 48_000;
const ACTION_OUTPUT_MAX_CHUNKS = 160;

export interface AssistantMessageTarget {
  clientTurnId?: string | null;
  assistantMessageId?: string | null;
}

export function resolveApprovalDecision(response: ApprovalResponse): ApprovalBlock["decision"] {
  if ("decision" in response && typeof response.decision === "string") {
    const decision = String(response.decision).trim();
    if (decision === "deny") {
      return "decline";
    }
    if (decision === "acceptForSession") {
      return "accept_for_session";
    }
    return decision as ApprovalBlock["decision"];
  }

  if ("action" in response && typeof response.action === "string") {
    const action = String(response.action).trim();
    if (action === "accept" || action === "decline" || action === "cancel") {
      return action;
    }
    return "custom";
  }

  if ("permissions" in response) {
    const scope = response.scope;
    const permissions =
      typeof response.permissions === "object" &&
      response.permissions !== null &&
      !Array.isArray(response.permissions)
        ? (response.permissions as Record<string, unknown>)
        : null;
    const hasGrantedPermission =
      permissions !== null &&
      Object.values(permissions).some((value) => {
        if (Array.isArray(value)) {
          return value.length > 0;
        }
        if (typeof value === "object" && value !== null) {
          return Object.values(value as Record<string, unknown>).some((nested) => {
            if (Array.isArray(nested)) {
              return nested.length > 0;
            }
            if (typeof nested === "object" && nested !== null) {
              return Object.keys(nested as Record<string, unknown>).length > 0;
            }
            return nested === true || (typeof nested === "string" && nested.toLowerCase() !== "none");
          });
        }
        return value === true || (typeof value === "string" && value.toLowerCase() !== "none");
      });

    if (!hasGrantedPermission) {
      return "decline";
    }
    if (scope === "session") {
      return "accept_for_session";
    }
    return "accept";
  }

  return "custom";
}

export function normalizeActionOutputStream(
  stream: unknown,
): ActionBlock["outputChunks"][number]["stream"] {
  return stream === "stderr" || stream === "stdin" ? stream : "stdout";
}

export function resolveApprovalInMessages(
  messages: Message[],
  approvalId: string,
  decision?: ApprovalBlock["decision"],
  responseData?: Record<string, unknown>,
): Message[] {
  for (let messageIndex = 0; messageIndex < messages.length; messageIndex += 1) {
    const message = messages[messageIndex];
    const blocks = message.blocks;
    if (!blocks || blocks.length === 0) {
      continue;
    }

    const approvalIndex = blocks.findIndex(
      (block) => block.type === "approval" && block.approvalId === approvalId,
    );
    if (approvalIndex < 0) {
      continue;
    }

    const approvalBlock = blocks[approvalIndex] as ApprovalBlock;
    if (
      approvalBlock.status === "answered" &&
      (decision === undefined || approvalBlock.decision === decision)
    ) {
      return messages;
    }

    const nextBlocks = [...blocks];
    nextBlocks[approvalIndex] = {
      ...approvalBlock,
      status: "answered" as const,
      ...(decision !== undefined ? { decision } : {}),
      ...(responseData !== undefined ? { responseData } : {}),
    };

    const nextMessages = [...messages];
    nextMessages[messageIndex] = {
      ...message,
      blocks: nextBlocks,
    };
    return nextMessages;
  }

  return messages;
}

export function trimActionOutputChunks(
  chunks: ActionBlock["outputChunks"],
): {
  chunks: ActionBlock["outputChunks"];
  truncated: boolean;
} {
  if (chunks.length === 0) {
    return { chunks, truncated: false };
  }

  let nextChunks = chunks;
  let truncated = false;

  if (nextChunks.length > ACTION_OUTPUT_MAX_CHUNKS) {
    nextChunks = nextChunks.slice(nextChunks.length - ACTION_OUTPUT_MAX_CHUNKS);
    truncated = true;
  }

  let totalChars = 0;
  for (const chunk of nextChunks) {
    totalChars += chunk.content.length;
  }

  if (totalChars <= ACTION_OUTPUT_MAX_CHARS) {
    return { chunks: nextChunks, truncated };
  }

  truncated = true;
  let charsToTrim = totalChars - ACTION_OUTPUT_TRIM_TARGET_CHARS;
  const trimmedChunks = [...nextChunks];
  let startIndex = 0;

  while (charsToTrim > 0 && startIndex < trimmedChunks.length) {
    const currentChunk = trimmedChunks[startIndex];
    const currentLength = currentChunk.content.length;
    if (currentLength <= charsToTrim) {
      charsToTrim -= currentLength;
      startIndex += 1;
      continue;
    }
    trimmedChunks[startIndex] = {
      ...currentChunk,
      content: currentChunk.content.slice(charsToTrim),
    };
    charsToTrim = 0;
  }

  return {
    chunks: trimmedChunks.slice(startIndex),
    truncated,
  };
}

export function patchActionBlock(
  blocks: ContentBlock[],
  actionId: string,
  updater: (block: ActionBlock) => ActionBlock,
): ContentBlock[] {
  const blockIndex = blocks.findIndex(
    (block) => block.type === "action" && block.actionId === actionId,
  );
  if (blockIndex < 0) {
    return blocks;
  }

  const current = blocks[blockIndex] as ActionBlock;
  const nextBlock = updater(current);
  if (nextBlock === current) {
    return blocks;
  }

  const nextBlocks = [...blocks];
  nextBlocks[blockIndex] = nextBlock;
  return nextBlocks;
}

function createSteerBlockFromMessage(message: Message): SteerBlock {
  const blocks = Array.isArray(message.blocks) ? message.blocks : [];
  const content =
    typeof message.content === "string" && message.content.length > 0
      ? message.content
      : blocks
          .filter((block): block is Extract<ContentBlock, { type: "text" }> => block.type === "text")
          .map((block) => block.content)
          .join("\n");
  const attachments = blocks.filter(
    (block): block is AttachmentBlock => block.type === "attachment",
  );
  const skills = blocks.filter((block): block is SkillBlock => block.type === "skill");
  const mentions = blocks.filter((block): block is MentionBlock => block.type === "mention");
  const planMode = blocks.some(
    (block) => block.type === "text" && Boolean(block.planMode),
  );

  return {
    type: "steer",
    steerId: message.id,
    content,
    planMode: planMode || undefined,
    attachments: attachments.length > 0 ? attachments : undefined,
    skills: skills.length > 0 ? skills : undefined,
    mentions: mentions.length > 0 ? mentions : undefined,
  };
}

function messageHasSteerMarker(message: Message): boolean {
  return (message.blocks ?? []).some(
    (block) => block.type === "text" && block.isSteer === true,
  );
}

export function appendSteerBlockToAssistantMessage(message: Message, steerBlock: SteerBlock): Message {
  const existingBlocks = message.blocks ?? [];
  if (
    existingBlocks.some(
      (block) => block.type === "steer" && block.steerId === steerBlock.steerId,
    )
  ) {
    return message;
  }

  const blocks = [...existingBlocks, steerBlock];
  return {
    ...message,
    blocks,
    hydration: "full",
    hasDeferredContent: hasDeferredActionOutput(blocks),
  };
}

export function removeSteerBlock(messages: Message[], steerId: string): Message[] {
  let nextMessages = messages;
  for (let index = 0; index < messages.length; index += 1) {
    const message = messages[index];
    const blocks = message.blocks ?? [];
    const nextBlocks = blocks.filter(
      (block) => !(block.type === "steer" && block.steerId === steerId),
    );
    if (nextBlocks.length === blocks.length) {
      continue;
    }

    if (nextMessages === messages) {
      nextMessages = [...messages];
    }
    nextMessages[index] = {
      ...message,
      blocks: nextBlocks,
      hydration: "full",
      hasDeferredContent: hasDeferredActionOutput(nextBlocks),
    };
  }

  return nextMessages;
}

export function collapseTrailingSteerMessages(messages: Message[]): Message[] {
  let changed = false;
  const collapsed: Message[] = [];

  for (let index = 0; index < messages.length; index += 1) {
    const message = messages[index];
    const previous = collapsed[collapsed.length - 1];
    const nextMessage = messages[index + 1];

    const isSteerCandidate =
      message.role === "user" &&
      previous?.role === "assistant" &&
      messageHasSteerMarker(message) &&
      nextMessage?.role !== "assistant";

    if (isSteerCandidate) {
      collapsed[collapsed.length - 1] = appendSteerBlockToAssistantMessage(
        previous,
        createSteerBlockFromMessage(message),
      );
      changed = true;
      continue;
    }

    collapsed.push(message);
  }

  return changed ? collapsed : messages;
}

function hasRenderableAssistantContent(message: Message): boolean {
  if (typeof message.content === "string" && message.content.trim().length > 0) {
    return true;
  }

  const blocks = message.blocks;
  if (!Array.isArray(blocks) || blocks.length === 0) {
    return false;
  }

  return blocks.some((block) => {
    if (block.type === "text" || block.type === "thinking") {
      return Boolean(block.content?.trim());
    }
    return true;
  });
}

export function resolveAssistantMessageIndex(
  messages: Message[],
  target: AssistantMessageTarget,
): number {
  if (target.assistantMessageId) {
    const byIdIndex = messages.findIndex((message) => message.id === target.assistantMessageId);
    if (byIdIndex >= 0) {
      return byIdIndex;
    }
  }

  if (target.clientTurnId) {
    for (let index = messages.length - 1; index >= 0; index -= 1) {
      const message = messages[index];
      if (message.role === "assistant" && message.clientTurnId === target.clientTurnId) {
        return index;
      }
    }
  }

  for (let index = messages.length - 1; index >= 0; index -= 1) {
    const message = messages[index];
    if (message.role === "assistant" && message.status === "streaming") {
      return index;
    }
  }

  return -1;
}

export function compactTrailingStreamingAssistantMessages(
  messages: Message[],
  target: AssistantMessageTarget,
): Message[] {
  if (messages.length < 2) {
    return messages;
  }

  let trailingStart = messages.length;
  while (trailingStart > 0) {
    const message = messages[trailingStart - 1];
    if (message.role !== "assistant" || message.status !== "streaming") {
      break;
    }
    trailingStart -= 1;
  }

  const trailingCount = messages.length - trailingStart;
  if (trailingCount <= 1) {
    return messages;
  }

  const trailingMessages = messages.slice(trailingStart);
  let keepIndex = -1;

  if (target.assistantMessageId) {
    keepIndex = trailingMessages.findIndex((message) => message.id === target.assistantMessageId);
  }
  if (keepIndex < 0 && target.clientTurnId) {
    keepIndex = trailingMessages.findIndex(
      (message) => message.clientTurnId === target.clientTurnId,
    );
  }
  if (keepIndex < 0) {
    keepIndex = trailingMessages.length - 1;
    for (let index = trailingMessages.length - 1; index >= 0; index -= 1) {
      if (hasRenderableAssistantContent(trailingMessages[index])) {
        keepIndex = index;
        break;
      }
    }
  }

  return [...messages.slice(0, trailingStart), trailingMessages[keepIndex]];
}

export function upsertBlock(blocks: ContentBlock[], block: ContentBlock): ContentBlock[] {
  if (block.type === "action") {
    const idx = blocks.findIndex(
      (b) => b.type === "action" && (b as ActionBlock).actionId === block.actionId
    );
    if (idx >= 0) {
      const next = [...blocks];
      next[idx] = block;
      return next;
    }
  }

  if (block.type === "approval") {
    const idx = blocks.findIndex(
      (b) => b.type === "approval" && (b as ApprovalBlock).approvalId === block.approvalId
    );
    if (idx >= 0) {
      const next = [...blocks];
      next[idx] = block;
      return next;
    }
  }

  return [...blocks, block];
}

export function upsertNoticeBlock(blocks: ContentBlock[], block: NoticeBlock): ContentBlock[] {
  const idx = blocks.findIndex(
    (candidate) =>
      candidate.type === "notice" &&
      (candidate as NoticeBlock).kind === block.kind,
  );
  if (idx >= 0) {
    const next = [...blocks];
    next[idx] = block;
    return next;
  }

  return [block, ...blocks];
}

function normalizeBlocks(blocks?: ContentBlock[]): ContentBlock[] | undefined {
  if (!Array.isArray(blocks)) {
    return blocks;
  }

  const normalized: ContentBlock[] = [];
  for (const block of blocks) {
    const last = normalized[normalized.length - 1];
    if (block.type === "text" && last?.type === "text") {
      normalized[normalized.length - 1] = {
        ...last,
        content: `${last.content}${block.content ?? ""}`
      };
      continue;
    }
    if (block.type === "thinking" && last?.type === "thinking") {
      normalized[normalized.length - 1] = {
        ...last,
        content: `${last.content}${block.content ?? ""}`
      };
      continue;
    }
    normalized.push(block);
  }

  return normalized;
}

export function hasDeferredActionOutput(blocks?: ContentBlock[]): boolean {
  if (!Array.isArray(blocks)) {
    return false;
  }
  return blocks.some((block) => block.type === "action" && block.outputDeferred === true);
}

export function markMessageAsFullyHydrated(message: Message): Message {
  const hasDeferredContent = hasDeferredActionOutput(message.blocks);
  if (message.hydration === "full" && message.hasDeferredContent === hasDeferredContent) {
    return message;
  }

  return {
    ...message,
    hydration: "full",
    hasDeferredContent,
  };
}

export function summarizeActionBlockForMemory(block: ActionBlock): ActionBlock {
  const hasOutput =
    block.outputChunks.length > 0 ||
    (typeof block.result?.output === "string" && block.result.output.length > 0) ||
    block.outputDeferred === true;
  if (!hasOutput) {
    return block;
  }

  let nextResult = block.result;
  if (block.result && typeof block.result.output === "string") {
    nextResult = {
      ...block.result,
      output: undefined,
    };
  }

  if (
    block.outputDeferred === true &&
    block.outputDeferredLoaded === false &&
    block.outputChunks.length === 0 &&
    nextResult === block.result
  ) {
    return block;
  }

  return {
    ...block,
    outputChunks: [],
    outputDeferred: true,
    outputDeferredLoaded: false,
    result: nextResult,
  };
}

export function summarizeMessageForMemory(message: Message): Message {
  const sourceBlocks = message.blocks;
  let nextBlocks = sourceBlocks;

  if (Array.isArray(sourceBlocks) && sourceBlocks.length > 0) {
    for (let index = 0; index < sourceBlocks.length; index += 1) {
      const block = sourceBlocks[index];
      if (block.type !== "action") {
        continue;
      }

      const summarizedBlock = summarizeActionBlockForMemory(block);
      if (summarizedBlock === block) {
        continue;
      }

      if (nextBlocks === sourceBlocks) {
        nextBlocks = [...sourceBlocks];
      }
      (nextBlocks as ContentBlock[])[index] = summarizedBlock;
    }
  }

  const hasDeferredContent = hasDeferredActionOutput(nextBlocks);
  if (
    nextBlocks === sourceBlocks &&
    message.hydration === "summary" &&
    message.hasDeferredContent === hasDeferredContent
  ) {
    return message;
  }

  return {
    ...message,
    hydration: "summary",
    hasDeferredContent,
    blocks: nextBlocks,
  };
}

export function applyHydrationWindow(messages: Message[]): Message[] {
  if (messages.length === 0) {
    return messages;
  }

  const summarizeUntil = Math.max(0, messages.length - MAX_FULLY_HYDRATED_MESSAGES);
  let nextMessages = messages;
  for (let index = 0; index < messages.length; index += 1) {
    const message = messages[index];
    const nextMessage =
      index < summarizeUntil
        ? summarizeMessageForMemory(message)
        : message.hydration === "summary"
          ? message
          : markMessageAsFullyHydrated(message);
    if (nextMessage !== message) {
      if (nextMessages === messages) {
        nextMessages = [...messages];
      }
      nextMessages[index] = nextMessage;
    }
  }

  return nextMessages;
}

export function normalizeMessages(
  messages: Message[],
  options?: { collapseTrailingSteers?: boolean },
): Message[] {
  let nextMessages = messages;
  for (let index = 0; index < nextMessages.length; index += 1) {
    const message = nextMessages[index];
    const normalizedBlocks = normalizeBlocks(message.blocks);
    const normalizedContent =
      message.role === "user" && normalizedBlocks && typeof message.content === "string"
        ? undefined
        : message.content;
    const normalizedMessage =
      normalizedBlocks === message.blocks && normalizedContent === message.content
        ? message
        : {
            ...message,
            content: normalizedContent,
            blocks: normalizedBlocks,
          };
    const nextMessage = markMessageAsFullyHydrated(normalizedMessage);
    if (nextMessage !== message) {
      if (nextMessages === messages) {
        nextMessages = [...messages];
      }
      nextMessages[index] = nextMessage;
    }
  }

  return options?.collapseTrailingSteers === false
    ? nextMessages
    : collapseTrailingSteerMessages(nextMessages);
}

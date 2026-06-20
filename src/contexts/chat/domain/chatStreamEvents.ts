import type { ContextUsage, StreamEvent, ThreadStatus } from "../../../types";

const CONTEXT_WINDOW_BASELINE_TOKENS = 12_000;

export function isThreadTurnActive(status: ThreadStatus): boolean {
  return status === "streaming" || status === "awaiting_approval";
}

export function eventHasVisibleAssistantContent(event: StreamEvent): boolean {
  switch (event.type) {
    case "TextDelta":
      return String(event.content ?? "").length > 0;
    case "ThinkingDelta":
      return String(event.content ?? "").length > 0;
    case "ActionStarted":
    case "ActionOutputDelta":
    case "ActionCompleted":
    case "ApprovalRequested":
    case "DiffUpdated":
    case "ActionProgressUpdated":
    case "ModelRerouted":
    case "Notice":
    case "Error":
      return true;
    default:
      return false;
  }
}

export function applyRuntimeStateFromEvent(
  status: ThreadStatus,
  streaming: boolean,
  event: StreamEvent,
): { status: ThreadStatus; streaming: boolean } {
  if (event.type === "UsageLimitsUpdated") {
    return { status, streaming };
  }

  if (event.type === "ApprovalRequested") {
    return { status: "awaiting_approval", streaming: true };
  }

  if (event.type === "ApprovalResolved") {
    return { status: "streaming", streaming: true };
  }

  if (event.type === "Error" && !event.recoverable) {
    return { status: "error", streaming: false };
  }

  if (event.type === "TurnCompleted") {
    const completionStatus = String(event.status ?? "completed");
    if (completionStatus === "failed") {
      return { status: "error", streaming: false };
    }
    if (completionStatus === "interrupted") {
      return { status: "idle", streaming: false };
    }
    return { status: "completed", streaming: false };
  }

  if (event.type === "TurnStarted" || eventHasVisibleAssistantContent(event)) {
    return { status: "streaming", streaming: true };
  }

  return { status, streaming };
}

export function enqueueStreamEvent(queue: StreamEvent[], event: StreamEvent): void {
  const previous = queue[queue.length - 1];
  if (!previous) {
    queue.push(event);
    return;
  }

  if (previous.type === "TextDelta" && event.type === "TextDelta") {
    queue[queue.length - 1] = {
      ...previous,
      content: `${previous.content}${event.content}`,
    };
    return;
  }

  if (previous.type === "ThinkingDelta" && event.type === "ThinkingDelta") {
    queue[queue.length - 1] = {
      ...previous,
      content: `${previous.content}${event.content}`,
    };
    return;
  }

  if (
    previous.type === "ActionOutputDelta" &&
    event.type === "ActionOutputDelta" &&
    previous.action_id === event.action_id &&
    previous.stream === event.stream
  ) {
    queue[queue.length - 1] = {
      ...previous,
      content: `${previous.content}${event.content}`,
    };
    return;
  }

  if (
    previous.type === "ActionProgressUpdated" &&
    event.type === "ActionProgressUpdated" &&
    previous.action_id === event.action_id
  ) {
    queue[queue.length - 1] = event;
    return;
  }

  if (
    previous.type === "DiffUpdated" &&
    event.type === "DiffUpdated" &&
    previous.scope === event.scope
  ) {
    queue[queue.length - 1] = event;
    return;
  }

  if (previous.type === "UsageLimitsUpdated" && event.type === "UsageLimitsUpdated") {
    queue[queue.length - 1] = event;
    return;
  }

  queue.push(event);
}

export type UsageResetTimestampFormatter = (value: number) => string | null;

function toResetTimestamp(
  value: number | null | undefined,
  formatTimestamp: UsageResetTimestampFormatter,
): string | null {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return null;
  }
  return formatTimestamp(value);
}

function calculateContextPercentRemaining(
  currentTokens: number | null,
  maxContextTokens: number | null,
): number | null {
  if (
    typeof currentTokens !== "number" ||
    !Number.isFinite(currentTokens) ||
    typeof maxContextTokens !== "number" ||
    !Number.isFinite(maxContextTokens)
  ) {
    return null;
  }

  if (maxContextTokens <= CONTEXT_WINDOW_BASELINE_TOKENS) {
    return 0;
  }

  const effectiveWindow = maxContextTokens - CONTEXT_WINDOW_BASELINE_TOKENS;
  const usedTokens = Math.max(0, currentTokens - CONTEXT_WINDOW_BASELINE_TOKENS);
  const remainingTokens = Math.max(0, effectiveWindow - usedTokens);

  return Math.max(
    0,
    Math.min(100, Math.round((remainingTokens / effectiveWindow) * 100)),
  );
}

export function mapUsageLimitsFromEvent(
  event: Extract<StreamEvent, { type: "UsageLimitsUpdated" }>,
  formatResetTimestamp: UsageResetTimestampFormatter,
): ContextUsage | null {
  const usage = event.usage ?? {};
  const currentTokensRaw = usage.current_tokens;
  const maxContextTokensRaw = usage.max_context_tokens;
  const contextPercentRaw = usage.context_window_percent;
  const fiveHourPercentRaw = usage.five_hour_percent;
  const weeklyPercentRaw = usage.weekly_percent;

  const currentTokens =
    typeof currentTokensRaw === "number" ? Math.max(0, Math.round(currentTokensRaw)) : null;
  const maxContextTokens =
    typeof maxContextTokensRaw === "number" ? Math.max(0, Math.round(maxContextTokensRaw)) : null;
  const hasContextMetrics = currentTokens !== null || maxContextTokens !== null;

  let contextPercent = calculateContextPercentRemaining(currentTokens, maxContextTokens);
  if (contextPercent === null && typeof contextPercentRaw === "number") {
    contextPercent = Math.round(contextPercentRaw);
  }
  if (contextPercent !== null && !Number.isFinite(contextPercent)) {
    contextPercent = null;
  }

  const hasAnyMetric =
    hasContextMetrics ||
    typeof contextPercentRaw === "number" ||
    typeof fiveHourPercentRaw === "number" ||
    typeof weeklyPercentRaw === "number";
  if (!hasAnyMetric) {
    return null;
  }

  const toRemainingPercent = (
    usedPercent: number | null | undefined,
  ): number | null => {
    if (typeof usedPercent !== "number" || !Number.isFinite(usedPercent)) {
      return null;
    }
    const used = Math.max(0, Math.min(100, Math.round(usedPercent)));
    return 100 - used;
  };

  return {
    currentTokens,
    maxContextTokens,
    contextPercent:
      contextPercent === null ? null : Math.max(0, Math.min(100, contextPercent)),
    windowFiveHourPercent: toRemainingPercent(fiveHourPercentRaw),
    windowWeeklyPercent: toRemainingPercent(weeklyPercentRaw),
    windowFiveHourResetsAt: toResetTimestamp(
      usage.five_hour_resets_at,
      formatResetTimestamp,
    ),
    windowWeeklyResetsAt: toResetTimestamp(
      usage.weekly_resets_at,
      formatResetTimestamp,
    ),
  };
}

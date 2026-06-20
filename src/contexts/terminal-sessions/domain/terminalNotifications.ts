import type { TerminalNotification } from "../../../types";

export interface TerminalNotificationHydrationState {
  notificationsBySessionId: Record<string, TerminalNotification>;
  notificationHydrating?: boolean;
  notificationTouchedAll?: boolean;
  notificationTouchedSessionIds?: Record<string, true>;
}

export interface TerminalNotificationHydrationTouch {
  notificationHydrating: boolean;
  notificationTouchedAll: boolean;
  notificationTouchedSessionIds: Record<string, true>;
}

export function pruneNotificationsByLiveSessions(
  notificationsBySessionId: Record<string, TerminalNotification>,
  liveIds: Set<string>,
): Record<string, TerminalNotification> {
  const nextEntries = Object.entries(notificationsBySessionId).filter(([sessionId]) =>
    liveIds.has(sessionId),
  );
  if (nextEntries.length === Object.keys(notificationsBySessionId).length) {
    return notificationsBySessionId;
  }
  return Object.fromEntries(nextEntries);
}

export function clearNotificationRecord(
  notificationsBySessionId: Record<string, TerminalNotification>,
  sessionId?: string | null,
): Record<string, TerminalNotification> {
  if (!sessionId) {
    if (Object.keys(notificationsBySessionId).length === 0) {
      return notificationsBySessionId;
    }
    return {};
  }
  if (!(sessionId in notificationsBySessionId)) {
    return notificationsBySessionId;
  }
  const { [sessionId]: _removed, ...rest } = notificationsBySessionId;
  return rest;
}

export function indexNotificationsBySession(
  notifications: TerminalNotification[],
  liveIds: Set<string>,
): Record<string, TerminalNotification> {
  const indexed: Record<string, TerminalNotification> = {};
  for (const notification of notifications) {
    if (!liveIds.has(notification.sessionId)) {
      continue;
    }
    const current = indexed[notification.sessionId];
    if (!current || notification.createdAt > current.createdAt) {
      indexed[notification.sessionId] = notification;
    }
  }
  return indexed;
}

export function withNotificationHydrationTouch(
  workspace: TerminalNotificationHydrationState,
  sessionId?: string | null,
): TerminalNotificationHydrationTouch {
  if (!workspace.notificationHydrating) {
    return {
      notificationHydrating: false,
      notificationTouchedAll: false,
      notificationTouchedSessionIds: {},
    };
  }
  if (!sessionId) {
    return {
      notificationHydrating: true,
      notificationTouchedAll: true,
      notificationTouchedSessionIds: workspace.notificationTouchedSessionIds ?? {},
    };
  }
  return {
    notificationHydrating: true,
    notificationTouchedAll: workspace.notificationTouchedAll ?? false,
    notificationTouchedSessionIds: {
      ...(workspace.notificationTouchedSessionIds ?? {}),
      [sessionId]: true,
    },
  };
}

export function hasNotificationHydrationTouchChange(
  workspace: TerminalNotificationHydrationState,
  nextTouch: TerminalNotificationHydrationTouch,
): boolean {
  if ((workspace.notificationHydrating ?? false) !== nextTouch.notificationHydrating) {
    return true;
  }
  if ((workspace.notificationTouchedAll ?? false) !== nextTouch.notificationTouchedAll) {
    return true;
  }

  const currentTouchedSessionIds = workspace.notificationTouchedSessionIds ?? {};
  const nextTouchedSessionIds = nextTouch.notificationTouchedSessionIds ?? {};
  const currentKeys = Object.keys(currentTouchedSessionIds);
  const nextKeys = Object.keys(nextTouchedSessionIds);
  if (currentKeys.length !== nextKeys.length) {
    return true;
  }

  return nextKeys.some((sessionId) => !currentTouchedSessionIds[sessionId]);
}

export function resolveHydratedNotifications(
  workspace: TerminalNotificationHydrationState,
  hydrated: Record<string, TerminalNotification>,
  liveIds: Set<string>,
): Record<string, TerminalNotification> {
  if (workspace.notificationTouchedAll) {
    return pruneNotificationsByLiveSessions(workspace.notificationsBySessionId, liveIds);
  }

  const touchedSessionIds = Object.keys(workspace.notificationTouchedSessionIds ?? {});
  if (touchedSessionIds.length === 0) {
    return hydrated;
  }

  const merged = { ...hydrated };
  for (const sessionId of touchedSessionIds) {
    if (!liveIds.has(sessionId)) {
      delete merged[sessionId];
      continue;
    }
    const current = workspace.notificationsBySessionId[sessionId];
    if (current) {
      merged[sessionId] = current;
    } else {
      delete merged[sessionId];
    }
  }

  return merged;
}

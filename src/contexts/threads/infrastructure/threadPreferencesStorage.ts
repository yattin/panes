const LAST_THREAD_KEY = "panes:lastActiveThreadId";

export function readLastActiveThreadId(): string | null {
  try {
    return localStorage.getItem(LAST_THREAD_KEY);
  } catch {
    return null;
  }
}

export function writeLastActiveThreadId(threadId: string): void {
  try {
    localStorage.setItem(LAST_THREAD_KEY, threadId);
  } catch {
    // localStorage unavailable or full; last active thread persistence is best-effort.
  }
}

export function clearLastActiveThreadId(): void {
  try {
    localStorage.removeItem(LAST_THREAD_KEY);
  } catch {
    // localStorage unavailable; ignore persistence failure.
  }
}

export type ChatTimerHandle = ReturnType<typeof globalThis.setTimeout>;

export const chatRuntime = {
  clearTimer(timer: ChatTimerHandle): void {
    globalThis.clearTimeout(timer);
  },

  createId(): string {
    return crypto.randomUUID();
  },

  nowIso(): string {
    return new Date().toISOString();
  },

  epochSecondsOrMillisecondsToIso(value: number): string | null {
    if (!Number.isFinite(value)) {
      return null;
    }
    const normalized = value < 10_000_000_000 ? value * 1000 : value;
    const date = new Date(normalized);
    if (Number.isNaN(date.getTime())) {
      return null;
    }
    return date.toISOString();
  },

  performanceNow(): number {
    return performance.now();
  },

  scheduleAfterPaint(callback: (timestamp: number) => void): void {
    if (typeof globalThis.requestAnimationFrame === "function") {
      globalThis.requestAnimationFrame(callback);
      return;
    }
    globalThis.setTimeout(() => callback(performance.now()), 0);
  },

  setTimer(callback: () => void, delayMs: number): ChatTimerHandle {
    return globalThis.setTimeout(callback, delayMs);
  },

  wallClockNow(): number {
    return Date.now();
  },
};

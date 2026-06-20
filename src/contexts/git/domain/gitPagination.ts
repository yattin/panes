export interface GitPage<T> {
  entries: T[];
  offset: number;
  limit: number;
  total: number;
  hasMore: boolean;
}

function safeNonNegativeInteger(value: unknown, fallback: number): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return fallback;
  }
  return Math.max(0, Math.floor(value));
}

export function normalizeGitPage<T>(
  page: Partial<GitPage<T>>,
  fallbackOffset: number,
  fallbackLimit: number,
): GitPage<T> {
  const entries = Array.isArray(page.entries) ? page.entries : [];
  const offset = safeNonNegativeInteger(page.offset, fallbackOffset);
  const limit = safeNonNegativeInteger(page.limit, fallbackLimit);
  const nextOffset = offset + entries.length;
  const total = safeNonNegativeInteger(page.total, nextOffset);
  const hasMore = typeof page.hasMore === "boolean" ? page.hasMore : nextOffset < total;

  return {
    entries,
    offset,
    limit,
    total,
    hasMore,
  };
}

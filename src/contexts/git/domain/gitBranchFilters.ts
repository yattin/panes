export function normalizeGitBranchSearch(query: string): string {
  return query.trim();
}

export function gitBranchSearchParam(query: string): string | undefined {
  const normalized = normalizeGitBranchSearch(query);
  return normalized ? normalized : undefined;
}

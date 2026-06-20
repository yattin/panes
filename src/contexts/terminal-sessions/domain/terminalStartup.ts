import type {
  SplitNode,
  TerminalGroup,
  WorktreeSessionInfo,
  WorkspaceStartupGroup,
  WorkspaceStartupPreset,
  WorkspaceStartupSplitNode,
  WorkspaceStartupWorktreeConfig,
} from "../../../types";
import { collectSessionIds } from "./terminalSplitTree";

export function pendingStartupPresetFor(
  preset: WorkspaceStartupPreset | null,
): WorkspaceStartupPreset | null {
  return preset?.terminal?.groups.length ? preset : null;
}

export function materializeStartupSplitNode(
  node: WorkspaceStartupSplitNode,
  sessionIdMap: Record<string, string>,
  createSplitId: () => string,
): SplitNode {
  if (node.type === "leaf") {
    return {
      type: "leaf",
      sessionId: sessionIdMap[node.sessionId],
    };
  }

  return {
    type: "split",
    id: createSplitId(),
    direction: node.direction,
    ratio: Math.max(0.1, Math.min(0.9, node.ratio)),
    children: [
      materializeStartupSplitNode(node.children[0], sessionIdMap, createSplitId),
      materializeStartupSplitNode(node.children[1], sessionIdMap, createSplitId),
    ],
  };
}

export function serializeRuntimeSplitNode(
  node: SplitNode,
  runtimeToPresetSessionId: Record<string, string>,
): WorkspaceStartupSplitNode {
  if (node.type === "leaf") {
    return {
      type: "leaf",
      sessionId: runtimeToPresetSessionId[node.sessionId] ?? node.sessionId,
    };
  }

  return {
    type: "split",
    direction: node.direction,
    ratio: Math.max(0.1, Math.min(0.9, node.ratio)),
    children: [
      serializeRuntimeSplitNode(node.children[0], runtimeToPresetSessionId),
      serializeRuntimeSplitNode(node.children[1], runtimeToPresetSessionId),
    ],
  };
}

export function slugifySegment(value: string): string {
  const normalized = value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9._/-]+/g, "-")
    .replace(/\/+/g, "/")
    .replace(/^-+|-+$/g, "");
  return normalized || "session";
}

export function isAbsolutePath(path: string): boolean {
  return path.startsWith("/") || /^[A-Za-z]:[\\/]/.test(path);
}

export function trimRelativePath(path: string): string {
  const normalized = path.trim().replace(/\\/g, "/");
  if (!normalized || normalized === ".") {
    return ".";
  }
  return normalized.replace(/^\.\/+/, "").replace(/\/+$/, "") || ".";
}

export function joinPath(basePath: string, childPath: string): string {
  const normalizedChild = trimRelativePath(childPath);
  if (normalizedChild === ".") {
    return basePath;
  }
  return `${basePath.replace(/\/+$/, "")}/${normalizedChild}`;
}

export function resolveWorktreeBaseDir(repoPath: string, baseDir?: string | null): string {
  if (!baseDir || baseDir.trim() === "") {
    return `${repoPath.replace(/\/+$/, "")}/.panes/worktrees`;
  }
  return isAbsolutePath(baseDir) ? baseDir : joinPath(repoPath, baseDir);
}

export function summarizeWarnings(warnings: string[]): string | null {
  if (warnings.length === 0) {
    return null;
  }
  if (warnings.length === 1) {
    return warnings[0];
  }
  return `${warnings[0]} (+${warnings.length - 1} more)`;
}

export function getGroupWorktreesFromMeta(group: TerminalGroup): WorktreeSessionInfo[] {
  return collectSessionIds(group.root)
    .map((sessionId) => group.sessionMeta?.[sessionId]?.worktree ?? null)
    .filter((worktree): worktree is WorktreeSessionInfo => worktree !== null);
}

export function inferWorktreeConfig(group: TerminalGroup): WorkspaceStartupWorktreeConfig | null {
  if (group.worktreeConfig) {
    return group.worktreeConfig;
  }

  const worktrees = getGroupWorktreesFromMeta(group);
  const first = worktrees[0];
  if (!first) {
    return null;
  }

  const repoPrefix = `${first.repoPath.replace(/\/+$/, "")}/`;
  const runSegment = first.worktreePath.slice(repoPrefix.length).split("/").slice(0, -2).join("/");
  const baseDir = runSegment
    ? first.worktreePath
        .slice(repoPrefix.length)
        .split("/")
        .slice(0, -2)
        .join("/")
    : ".panes/worktrees";

  return {
    enabled: true,
    repoMode: "fixed_repo",
    repoPath: first.repoPath,
    baseDir,
    branchPrefix: first.branch.split("/").slice(0, -2).join("/") || "panes/preset",
  };
}

export function resolveSessionStartupCwd(
  workspaceRoot: string,
  session: WorkspaceStartupGroup["sessions"][number],
  worktreePath: string | null,
): string | null {
  const cwd = session.cwd.trim();
  const cwdBase = session.cwdBase ?? "workspace";

  if (cwdBase === "absolute") {
    return cwd;
  }
  if (cwdBase === "worktree") {
    return worktreePath ? (cwd === "." ? worktreePath : joinPath(worktreePath, cwd)) : null;
  }
  return cwd === "." ? workspaceRoot : joinPath(workspaceRoot, cwd);
}

export function buildStartupWorktreeBranch(
  branchPrefix: string,
  runId: string,
  logicalSessionId: string,
  index: number,
): string {
  return `${branchPrefix.replace(/\/+$/, "")}/${runId}/${slugifySegment(logicalSessionId || `session-${index + 1}`)}`;
}

export function buildStartupWorktreePath(
  repoPath: string,
  baseDir: string | null | undefined,
  runId: string,
  logicalSessionId: string,
  index: number,
): string {
  const basePath = resolveWorktreeBaseDir(repoPath, baseDir);
  return `${basePath.replace(/\/+$/, "")}/${runId}/${slugifySegment(logicalSessionId || `session-${index + 1}`)}`;
}

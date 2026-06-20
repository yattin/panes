import type { Repo } from "../../../types";
import {
  compareRepoRoots,
  isWithinRoot,
  normalizeAbsolutePath,
} from "./pathRoots";
import {
  DISALLOWED_LOCAL_PREFIX_CHAR_RE,
  TEXT_LINK_PATTERN,
  isLocalFileLinkSyntax,
  parseLocalAbsolutePathTarget,
  parseLocalRelativePathTarget,
  parseLocalUrlTarget,
  trimLinkText,
  tryParseUrl,
} from "./localFileLinkPatterns";

const EXTERNAL_PROTOCOLS = new Set(["http:", "https:", "mailto:", "tel:"]);
type LinkRepoRoot = Pick<Repo, "id" | "path"> & Partial<Pick<Repo, "isActive">>;

export interface LinkResolutionContext {
  workspaceRoot: string | null;
  repos: LinkRepoRoot[];
  activeRepoId?: string | null;
}

export interface ResolvedLocalFileLink {
  rootPath: string;
  filePath: string;
  absolutePath: string;
  line?: number;
  column?: number;
}

export interface TextLinkMatch {
  text: string;
  startIndex: number;
  endIndex: number;
  kind: LinkTargetKind;
}

export type LinkTargetKind = "local" | "external" | "other";

export function classifyLinkTarget(rawTarget: string): LinkTargetKind {
  if (isLocalFileLinkSyntax(rawTarget)) {
    return "local";
  }

  const url = tryParseUrl(rawTarget);
  if (url && EXTERNAL_PROTOCOLS.has(url.protocol)) {
    return "external";
  }

  return "other";
}

function isEmbeddedEmailPathCandidate(trimmedText: string): boolean {
  const firstSlashIndex = trimmedText.search(/[\\/]/);
  if (firstSlashIndex < 0) {
    return false;
  }
  return trimmedText.slice(0, firstSlashIndex).includes("@");
}

export function extractTextLinkMatches(text: string): TextLinkMatch[] {
  const matches: TextLinkMatch[] = [];
  for (const match of text.matchAll(TEXT_LINK_PATTERN)) {
    const rawText = match[0];
    const startIndex = match.index ?? 0;
    const trimmedText = trimLinkText(rawText);
    if (!trimmedText) {
      continue;
    }

    const kind = classifyLinkTarget(trimmedText);
    if (kind === "other") {
      continue;
    }
    if (
      kind === "local" &&
      (
        isEmbeddedEmailPathCandidate(trimmedText) ||
        (
          startIndex > 0 &&
          DISALLOWED_LOCAL_PREFIX_CHAR_RE.test(text[startIndex - 1] ?? "")
        )
      )
    ) {
      continue;
    }

    matches.push({
      text: trimmedText,
      startIndex,
      endIndex: startIndex + trimmedText.length,
      kind,
    });
  }
  return matches;
}

function getOrderedRelativeRoots(context: LinkResolutionContext): string[] {
  const repos = context.repos.slice();
  const activeRepo = context.activeRepoId
    ? repos.find((repo) => repo.id === context.activeRepoId) ?? null
    : null;
  const activeRepos = repos
    .filter((repo) => repo.id !== activeRepo?.id && repo.isActive)
    .sort((left, right) => compareRepoRoots(left, right, null));
  const remainingRepos = repos
    .filter((repo) => repo.id !== activeRepo?.id && !repo.isActive)
    .sort((left, right) => compareRepoRoots(left, right, null));
  const roots = [
    ...(activeRepo ? [activeRepo] : []),
    ...activeRepos,
    ...remainingRepos,
  ].map((repo) => normalizeAbsolutePath(repo.path));

  if (context.workspaceRoot) {
    roots.push(normalizeAbsolutePath(context.workspaceRoot));
  }

  return [...new Set(roots)];
}

export function resolveLocalFileLinkTarget(
  rawTarget: string,
  context: LinkResolutionContext,
): ResolvedLocalFileLink | null {
  const absoluteTarget = parseLocalAbsolutePathTarget(rawTarget) ?? parseLocalUrlTarget(rawTarget);

  const workspaceRoot = context.workspaceRoot ? normalizeAbsolutePath(context.workspaceRoot) : null;
  if (absoluteTarget) {
    const candidateRoots = context.repos
      .slice()
      .sort((left, right) => compareRepoRoots(left, right, context.activeRepoId))
      .map((repo) => normalizeAbsolutePath(repo.path));

    if (workspaceRoot) {
      candidateRoots.push(workspaceRoot);
    }

    const absolutePath = normalizeAbsolutePath(absoluteTarget.path);
    const matchedRoot = candidateRoots.find((root) => isWithinRoot(absolutePath, root));
    if (!matchedRoot) {
      return null;
    }

    const relativePath = absolutePath.slice(matchedRoot.length).replace(/^\/+/, "");
    if (!relativePath) {
      return null;
    }

    return {
      rootPath: matchedRoot,
      filePath: relativePath,
      absolutePath,
      line: absoluteTarget.reveal?.line,
      column: absoluteTarget.reveal?.column ?? undefined,
    };
  }

  const relativeTarget = parseLocalRelativePathTarget(rawTarget);
  if (!relativeTarget) {
    return null;
  }

  for (const root of getOrderedRelativeRoots(context)) {
    const absolutePath = normalizeAbsolutePath(`${root}/${relativeTarget.path}`);
    if (!isWithinRoot(absolutePath, root)) {
      continue;
    }

    return {
      rootPath: root,
      filePath: relativeTarget.path,
      absolutePath,
      line: relativeTarget.reveal?.line,
      column: relativeTarget.reveal?.column ?? undefined,
    };
  }

  return null;
}

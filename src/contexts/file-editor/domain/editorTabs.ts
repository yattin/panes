import type {
  EditorRevealLocation,
  EditorRevealRequest,
  EditorRenderMode,
  EditorTab,
  GitFileCompare,
  Repo,
} from "../../../types";
import {
  isWithinRoot,
  resolveAbsoluteFilePath,
  resolveOwningRepoForAbsolutePath,
  resolveRelativePathWithinRoot,
} from "../../file-navigation/domain/pathRoots";
import { isMarkdownPreviewFile } from "./editorFileTypes";

type RepoRoot = Pick<Repo, "id" | "path">;

export interface ResolvedFileContext {
  absolutePath: string;
  gitRepoPath: string | null;
  gitFilePath: string | null;
}

export interface RenameRetargetContext {
  rootPath: string;
  oldPath: string;
  newPath: string;
  repos: RepoRoot[];
  activeRepoId: string | null;
}

export function remapAbsolutePathForRename(
  absolutePath: string,
  oldAbsolutePath: string,
  newAbsolutePath: string,
): string | null {
  if (!isWithinRoot(absolutePath, oldAbsolutePath)) {
    return null;
  }

  if (absolutePath === oldAbsolutePath) {
    return newAbsolutePath;
  }

  const suffix = absolutePath.slice(oldAbsolutePath.length).replace(/^\/+/, "");
  return suffix ? resolveAbsoluteFilePath(newAbsolutePath, suffix) : newAbsolutePath;
}

export function createPlainTab(
  id: string,
  workspaceId: string | null,
  rootPath: string,
  filePath: string,
  resolved: ResolvedFileContext,
): EditorTab {
  return {
    id,
    workspaceId,
    rootPath,
    absolutePath: resolved.absolutePath,
    filePath,
    gitRepoPath: resolved.gitRepoPath,
    gitFilePath: resolved.gitFilePath,
    fileName: filePath.split("/").pop() ?? filePath,
    content: "",
    savedContent: "",
    isDirty: false,
    isLoading: true,
    isBinary: false,
    renderMode: "plain-editor",
    gitContext: null,
    pendingReveal: null,
  };
}

export function applyGitCompare(tab: EditorTab, compare: GitFileCompare): EditorTab {
  const preserveDirtyContent = tab.isDirty;
  const content = preserveDirtyContent ? tab.content : compare.modifiedContent;
  const savedContent = preserveDirtyContent ? tab.savedContent : compare.modifiedContent;

  return {
    ...tab,
    content,
    savedContent,
    isDirty: preserveDirtyContent ? tab.content !== tab.savedContent : false,
    isLoading: false,
    isBinary: compare.isBinary,
    renderMode: "git-diff-editor",
    gitContext: compare,
    pendingReveal: null,
    loadError: undefined,
  };
}

export function createRevealRequest(
  reveal: EditorRevealLocation | null | undefined,
  createNonce: () => string,
): EditorRevealRequest | null {
  if (!reveal) {
    return null;
  }

  return {
    line: reveal.line,
    column: reveal.column ?? null,
    nonce: createNonce(),
  };
}

export function toPlainEditorTab(
  tab: EditorTab,
  pendingReveal: EditorRevealRequest | null,
): EditorTab {
  return {
    ...tab,
    renderMode: "plain-editor",
    gitContext: null,
    pendingReveal,
    loadError: undefined,
  };
}

export function defaultOpenRenderMode(
  filePath: string,
  pendingReveal: EditorRevealRequest | null,
): EditorRenderMode {
  if (!pendingReveal && isMarkdownPreviewFile(filePath)) {
    return "markdown-preview";
  }
  return "plain-editor";
}

export function toOpenedFileTab(
  tab: EditorTab,
  pendingReveal: EditorRevealRequest | null,
  renderMode: EditorRenderMode,
): EditorTab {
  return {
    ...tab,
    renderMode,
    gitContext: null,
    pendingReveal: renderMode === "plain-editor" ? pendingReveal : null,
    loadError: undefined,
  };
}

export function toMarkdownPreviewTab(tab: EditorTab): EditorTab {
  if (tab.isBinary) {
    return toPlainEditorTab(tab, null);
  }

  return {
    ...tab,
    renderMode: "markdown-preview",
    gitContext: null,
    pendingReveal: null,
    loadError: undefined,
  };
}

export function retargetEditorTabAfterRename(
  tab: EditorTab,
  context: RenameRetargetContext,
): EditorTab {
  const oldAbsolutePath = resolveAbsoluteFilePath(context.rootPath, context.oldPath);
  const newAbsolutePath = resolveAbsoluteFilePath(context.rootPath, context.newPath);
  const nextAbsolutePath = remapAbsolutePathForRename(
    tab.absolutePath,
    oldAbsolutePath,
    newAbsolutePath,
  );
  if (!nextAbsolutePath) {
    return tab;
  }

  const nextRootPath =
    remapAbsolutePathForRename(tab.rootPath, oldAbsolutePath, newAbsolutePath) ??
    tab.rootPath;
  const nextGitRepoPath = tab.gitRepoPath
    ? remapAbsolutePathForRename(tab.gitRepoPath, oldAbsolutePath, newAbsolutePath) ??
      tab.gitRepoPath
    : null;
  const nextFilePath = resolveRelativePathWithinRoot(
    nextAbsolutePath,
    nextRootPath,
  );
  if (nextFilePath === null) {
    return tab;
  }

  const ownership =
    !nextGitRepoPath || nextGitRepoPath === tab.gitRepoPath
      ? resolveOwningRepoForAbsolutePath(
          nextAbsolutePath,
          context.repos,
          context.activeRepoId,
        )
      : null;
  const resolvedGitRepoPath = nextGitRepoPath ?? ownership?.repo.path ?? null;
  const resolvedGitFilePath = resolvedGitRepoPath
    ? resolveRelativePathWithinRoot(nextAbsolutePath, resolvedGitRepoPath)
    : ownership?.filePath ?? null;

  return {
    ...tab,
    rootPath: nextRootPath,
    absolutePath: nextAbsolutePath,
    filePath: nextFilePath,
    fileName: nextFilePath.split("/").pop() ?? nextFilePath,
    gitRepoPath: resolvedGitRepoPath,
    gitFilePath:
      resolvedGitFilePath && resolvedGitFilePath.length > 0
        ? resolvedGitFilePath
        : null,
  };
}

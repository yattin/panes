import { useFileStore } from "../../file-editor/application/fileStore";
import { openExternalUrl } from "../../shell-ui/application/externalLinks";
import { useWorkspaceStore } from "../../workspaces/application/workspaceStore";
import { showWorkspaceEditorForFileLink } from "../../workspace-panes/application/workspacePaneNavigation";
import {
  classifyLinkTarget,
  extractTextLinkMatches,
  resolveLocalFileLinkTarget,
  type LinkTargetKind,
  type ResolvedLocalFileLink,
  type TextLinkMatch,
} from "../domain/fileLinkResolution";

export type { LinkResolutionContext } from "../domain/fileLinkResolution";
export type { LinkTargetKind, ResolvedLocalFileLink, TextLinkMatch };
export type LinkNavigationResult = "internal" | "external" | "ignored";

export interface LinkNavigationOptions {
  shiftKey: boolean;
  sourceLeafId?: string | null;
}

export function getWorkspacePaneLeafIdFromEventTarget(target: EventTarget | null): string | null {
  const element = target instanceof Element
    ? target
    : target instanceof Node
      ? target.parentElement
      : null;
  const leaf = element?.closest("[data-workspace-pane-leaf-id]");
  return leaf instanceof HTMLElement ? leaf.dataset.workspacePaneLeafId ?? null : null;
}

export async function navigateLinkTarget(
  rawTarget: string,
  options: LinkNavigationOptions,
): Promise<LinkNavigationResult> {
  if (!options.shiftKey) {
    return "ignored";
  }

  const workspaceState = useWorkspaceStore.getState();
  const activeWorkspaceId = workspaceState.activeWorkspaceId;
  const activeWorkspace = activeWorkspaceId
    ? workspaceState.workspaces.find((workspace) => workspace.id === activeWorkspaceId) ?? null
    : null;
  const repos = activeWorkspaceId
    ? workspaceState.repos.filter((repo) => repo.workspaceId === activeWorkspaceId)
    : workspaceState.repos;

  const localTarget = resolveLocalFileLinkTarget(rawTarget, {
    workspaceRoot: activeWorkspace?.rootPath ?? null,
    repos,
    activeRepoId: workspaceState.activeRepoId,
  });

  if (localTarget) {
    const reveal = localTarget.line
      ? {
          line: localTarget.line,
          column: localTarget.column,
        }
      : null;

    await useFileStore
      .getState()
      .openFileAtLocation(localTarget.rootPath, localTarget.filePath, reveal);

    if (activeWorkspaceId) {
      showWorkspaceEditorForFileLink(activeWorkspaceId, options.sourceLeafId ?? null);
    }

    return "internal";
  }

  if (classifyLinkTarget(rawTarget) === "external") {
    await openExternalUrl(rawTarget);
    return "external";
  }

  return "ignored";
}

export {
  classifyLinkTarget,
  extractTextLinkMatches,
  resolveLocalFileLinkTarget,
};

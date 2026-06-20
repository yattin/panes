import type { MouseEvent } from "react";

import { t } from "../../../i18n";
import { useFileStore } from "../../file-editor/application/fileStore";
import { useTerminalStore } from "../../terminal-sessions/application/terminalStore";
import { toast } from "../../shell-ui/application/toastStore";
import { useUiStore } from "../../shell-ui/application/uiStore";
import { getFileNavigationGateway } from "./fileNavigationGateway";

export interface EditorFileReferenceContext {
  workspaceId: string | null;
  preferredRepoPath?: string | null;
  currentCwd?: string | null;
}

export async function openEditorFileReference(
  rawReference: string,
  context: EditorFileReferenceContext,
): Promise<boolean> {
  if (!context.workspaceId) {
    return false;
  }

  const resolved = await getFileNavigationGateway().resolveEditorFileReference(
    context.workspaceId,
    rawReference,
    context.preferredRepoPath,
    context.currentCwd,
  );
  if (!resolved) {
    toast.warning(t("common:fileReferences.resolveFailed", { reference: rawReference }));
    return false;
  }

  await useFileStore.getState().openFile(resolved.repoPath, resolved.filePath);
  useUiStore.getState().setExplorerOpen(false);
  await useTerminalStore.getState().setLayoutMode(context.workspaceId, "editor");
  return true;
}

export function handleEditorFileReferenceClick(
  event: MouseEvent<HTMLElement>,
  rawReference: string,
  context: EditorFileReferenceContext,
): void {
  event.preventDefault();
  if (!event.shiftKey) {
    return;
  }
  void openEditorFileReference(rawReference, context);
}

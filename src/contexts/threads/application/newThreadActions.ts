import { t } from "../../../i18n";
import { useChatStore } from "../../chat/application/chatStore";
import { useTerminalStore } from "../../terminal-sessions/application/terminalStore";
import { useUiStore } from "../../shell-ui/application/uiStore";
import { useWorkspaceStore } from "../../workspaces/application/workspaceStore";
import { resolveNewThreadTargetLayoutMode } from "../../workspace-panes/domain/newThreadLayout";
import {
  applyWorkspaceLayoutMode,
  getWorkspacePaneLayoutMode,
} from "../../workspace-panes/application/workspacePaneNavigation";
import { useThreadStore } from "./threadStore";

export async function createAndActivateWorkspaceThread(
  workspaceId: string | null | undefined,
): Promise<string | null> {
  if (!workspaceId) {
    return null;
  }

  const workspaceStore = useWorkspaceStore.getState();
  const activeWorkspaceId = workspaceStore.activeWorkspaceId;
  const terminalStore = useTerminalStore.getState();
  const currentLayoutMode =
    (activeWorkspaceId ? getWorkspacePaneLayoutMode(activeWorkspaceId) : null) ??
    (activeWorkspaceId
      ? terminalStore.workspaces[activeWorkspaceId]?.layoutMode
      : terminalStore.workspaces[workspaceId]?.layoutMode) ?? null;
  const targetLayoutMode = resolveNewThreadTargetLayoutMode(currentLayoutMode);

  useUiStore.getState().setActiveView("chat");

  if (activeWorkspaceId !== workspaceId) {
    await workspaceStore.setActiveWorkspace(workspaceId);
  }

  applyWorkspaceLayoutMode(workspaceId, targetLayoutMode);
  useWorkspaceStore.getState().setActiveRepo(null, { remember: false });

  const threadId = await useThreadStore.getState().createThread({
    workspaceId,
    repoId: null,
    title: t("app:sidebar.newThreadTitle"),
  });

  if (!threadId) {
    return null;
  }

  await useChatStore.getState().setActiveThread(threadId);
  return threadId;
}

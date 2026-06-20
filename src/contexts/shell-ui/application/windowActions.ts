import { useFileStore } from "../../file-editor/application/fileStore";
import { useTerminalStore } from "../../terminal-sessions/application/terminalStore";
import { useWorkspaceStore } from "../../workspaces/application/workspaceStore";
import { getShellUiGateway } from "./shellUiGateway";
export { shouldHandleAppShortcutWhileTerminalFocused } from "../domain/appShortcuts";

export function isLinuxDesktop(): boolean {
  return typeof navigator !== "undefined"
    && navigator.platform.toLowerCase().includes("linux")
    && getShellUiGateway().isTauriRuntime();
}

export function isWindowsDesktop(): boolean {
  return typeof navigator !== "undefined"
    && navigator.platform.toLowerCase().startsWith("win")
    && getShellUiGateway().isTauriRuntime();
}

export function isMacDesktop(): boolean {
  return typeof navigator !== "undefined"
    && navigator.platform.startsWith("Mac")
    && getShellUiGateway().isTauriRuntime();
}

export function usesCustomWindowFrame(): boolean {
  return isLinuxDesktop() || isWindowsDesktop();
}

export function isTerminalInputFocused(doc: Document | undefined = globalThis.document): boolean {
  const activeElement = doc?.activeElement;
  return typeof activeElement === "object"
    && activeElement !== null
    && "classList" in activeElement
    && typeof activeElement.classList.contains === "function"
    && activeElement.classList.contains("xterm-helper-textarea");
}

export async function closeCurrentWindow(): Promise<void> {
  const gateway = getShellUiGateway();
  try {
    await gateway.closeNativeWindow();
  } catch (error) {
    if (import.meta.env.DEV) {
      console.warn("[windowActions] Failed to close current window, forcing destroy", error);
    }
    await gateway.destroyNativeWindow();
  }
}

export async function minimizeCurrentWindow(): Promise<void> {
  const gateway = getShellUiGateway();
  try {
    await gateway.minimizeNativeWindow();
  } catch (error) {
    if (import.meta.env.DEV) {
      console.warn("[windowActions] Failed to minimize current window, falling back to hide", error);
    }
    await gateway.hideNativeWindow();
  }
}

export async function toggleCurrentWindowMaximize(): Promise<void> {
  await getShellUiGateway().toggleNativeWindowMaximize();
}

export async function toggleWindowFullscreen(): Promise<void> {
  const gateway = getShellUiGateway();
  const isFullscreen = await gateway.isNativeWindowFullscreen();
  await gateway.setNativeWindowFullscreen(!isFullscreen);
}

export async function requestWindowClose(): Promise<void> {
  const wsId = useWorkspaceStore.getState().activeWorkspaceId;
  const wsState = wsId ? useTerminalStore.getState().workspaces[wsId] : undefined;
  const fileState = useFileStore.getState();

  if (wsState?.layoutMode === "editor" && fileState.activeTabId) {
    fileState.requestCloseTab(fileState.activeTabId);
    return;
  }

  await closeCurrentWindow();
}

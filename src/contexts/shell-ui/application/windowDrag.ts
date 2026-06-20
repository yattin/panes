import {
  getShellUiGateway,
  type WindowResizeDirection,
} from "./shellUiGateway";

const INTERACTIVE = "button, input, textarea, select, a, .dropdown-menu, .no-drag";

export type { WindowResizeDirection };

function isInteractive(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return false;
  return target.closest(INTERACTIVE) !== null;
}

function reportWindowActionError(action: string, error: unknown) {
  if (import.meta.env.DEV) {
    console.warn(`[windowDrag] Failed to ${action}`, error);
  }
}

export function handleDragMouseDown(e: React.MouseEvent) {
  if (e.button !== 0) return;
  if (isInteractive(e.target)) return;
  const gateway = getShellUiGateway();
  if (!gateway.isTauriRuntime()) return;
  gateway.startNativeWindowDrag().catch((error) => {
    reportWindowActionError("start dragging window", error);
  });
}

export function handleDragDoubleClick(e: React.MouseEvent) {
  if (isInteractive(e.target)) return;
  const gateway = getShellUiGateway();
  if (!gateway.isTauriRuntime()) return;
  gateway.toggleNativeWindowMaximize().catch((error) => {
    reportWindowActionError("toggle maximize window", error);
  });
}

export function handleResizeMouseDown(
  direction: WindowResizeDirection,
  e: React.MouseEvent,
) {
  if (e.button !== 0) return;
  e.preventDefault();
  const gateway = getShellUiGateway();
  if (!gateway.isTauriRuntime()) return;
  gateway.startNativeWindowResizeDrag(direction).catch((error) => {
    reportWindowActionError(`start resize dragging window (${direction})`, error);
  });
}

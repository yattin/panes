import * as ipcModule from "../../../lib/ipc";

const { ipc } = ipcModule;

type MenuActionUnlisten = () => void;

function listenMenuAction(onEvent: (action: string) => void): Promise<MenuActionUnlisten> {
  const listener = (ipcModule as {
    listenMenuAction?: (onEvent: (action: string) => void) => Promise<MenuActionUnlisten>;
  }).listenMenuAction;
  if (!listener) {
    return Promise.reject(new Error("listenMenuAction is unavailable."));
  }
  return listener(onEvent);
}

export const shellNativeRepository = {
  listenMenuAction,
  showAgentNotification: ipc.showAgentNotification,
};

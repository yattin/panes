import type { EngineHealth, EngineInfo } from "../../../types";
import * as ipcModule from "../../../lib/ipc";
import type { EngineRuntimeUpdatedEvent } from "../../../types";
import type { EngineGateway } from "../application/engineGateway";

const { ipc } = ipcModule;

type EngineRuntimeUpdatedUnlisten = () => void;

export async function listEngines(): Promise<EngineInfo[]> {
  return ipc.listEngines();
}

export async function getEngineHealth(engineId: string): Promise<EngineHealth> {
  return ipc.engineHealth(engineId);
}

export function listenEngineRuntimeUpdated(
  onEvent: (event: EngineRuntimeUpdatedEvent) => void,
): Promise<EngineRuntimeUpdatedUnlisten> {
  const listener = (ipcModule as {
    listenEngineRuntimeUpdated?: (
      onEvent: (event: EngineRuntimeUpdatedEvent) => void,
    ) => Promise<EngineRuntimeUpdatedUnlisten>;
  }).listenEngineRuntimeUpdated;
  if (!listener) {
    return Promise.reject(new Error("listenEngineRuntimeUpdated is unavailable."));
  }
  return listener(onEvent);
}

export const engineRepository: EngineGateway = {
  getEngineHealth,
  listEngines,
};

import { ipc } from "../../../lib/ipc";
import type { HelperStatus, KeepAwakeState, PowerSettings, PowerSettingsInput } from "../../../types";
import type { PowerManagementGateway } from "../application/powerManagementGateway";

export async function getKeepAwakeState(): Promise<KeepAwakeState> {
  return ipc.getKeepAwakeState();
}

export async function setKeepAwakeEnabled(enabled: boolean): Promise<KeepAwakeState> {
  return ipc.setKeepAwakeEnabled(enabled);
}

export async function getPowerSettings(): Promise<PowerSettings> {
  return ipc.getPowerSettings();
}

export async function setPowerSettings(input: PowerSettingsInput): Promise<KeepAwakeState> {
  return ipc.setPowerSettings(input);
}

export async function getHelperStatus(): Promise<HelperStatus> {
  return ipc.getHelperStatus();
}

export async function registerKeepAwakeHelper(): Promise<HelperStatus> {
  return ipc.registerKeepAwakeHelper();
}

export const powerManagementRepository: PowerManagementGateway = {
  getHelperStatus,
  getKeepAwakeState,
  getPowerSettings,
  registerKeepAwakeHelper,
  setKeepAwakeEnabled,
  setPowerSettings,
};

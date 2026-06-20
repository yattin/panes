import { relaunch } from "@tauri-apps/plugin-process";
import { check } from "@tauri-apps/plugin-updater";
import type { AvailableUpdate, UpdateGateway } from "../application/updateGateway";

export async function checkForAvailableUpdate(): Promise<AvailableUpdate | null> {
  return check();
}

export async function relaunchAfterUpdate(): Promise<void> {
  await relaunch();
}

export const tauriUpdateClient: UpdateGateway = {
  checkForAvailableUpdate,
  relaunchAfterUpdate,
};

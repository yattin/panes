import { ipc } from "../../../lib/ipc";
import type {
  TerminalNotificationIntegrationId,
  TerminalNotificationSettings,
} from "../../../types";
import type { TerminalNotificationSettingsGateway } from "../application/terminalNotificationSettingsGateway";

export async function getTerminalNotificationSettings(): Promise<TerminalNotificationSettings> {
  return ipc.getAgentNotificationSettings();
}

export async function setChatNotificationsEnabled(enabled: boolean): Promise<boolean> {
  return ipc.setChatNotificationsEnabled(enabled);
}

export async function setTerminalNotificationsEnabled(enabled: boolean): Promise<boolean> {
  return ipc.setTerminalNotificationsEnabled(enabled);
}

export async function setNotificationSound(sound: string): Promise<string> {
  return ipc.setNotificationSound(sound);
}

export async function previewNotificationSound(sound: string): Promise<void> {
  await ipc.previewNotificationSound(sound);
}

export async function installTerminalNotificationIntegration(
  integration: TerminalNotificationIntegrationId,
): Promise<TerminalNotificationSettings> {
  return ipc.installTerminalNotificationIntegration(integration);
}

export const terminalNotificationSettingsRepository: TerminalNotificationSettingsGateway = {
  getTerminalNotificationSettings,
  installTerminalNotificationIntegration,
  previewNotificationSound,
  setChatNotificationsEnabled,
  setNotificationSound,
  setTerminalNotificationsEnabled,
};

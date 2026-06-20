import type {
  TerminalNotificationIntegrationId,
  TerminalNotificationSettings,
} from "../../../types";

export interface TerminalNotificationSettingsGateway {
  getTerminalNotificationSettings(): Promise<TerminalNotificationSettings>;
  installTerminalNotificationIntegration(
    integration: TerminalNotificationIntegrationId,
  ): Promise<TerminalNotificationSettings>;
  previewNotificationSound(sound: string): Promise<void>;
  setChatNotificationsEnabled(enabled: boolean): Promise<boolean>;
  setNotificationSound(sound: string): Promise<string>;
  setTerminalNotificationsEnabled(enabled: boolean): Promise<boolean>;
}

let configuredTerminalNotificationSettingsGateway:
  | TerminalNotificationSettingsGateway
  | null = null;

export function configureTerminalNotificationSettingsGateway(
  gateway: TerminalNotificationSettingsGateway,
): void {
  configuredTerminalNotificationSettingsGateway = gateway;
}

export function getTerminalNotificationSettingsGateway(): TerminalNotificationSettingsGateway {
  if (!configuredTerminalNotificationSettingsGateway) {
    throw new Error("TerminalNotificationSettingsGateway has not been configured.");
  }
  return configuredTerminalNotificationSettingsGateway;
}

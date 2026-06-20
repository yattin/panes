import type {
  TerminalNotificationIntegrationId,
  TerminalNotificationSettings,
} from "../../../types";

export interface TerminalNotificationSettingsStoreState {
  settings: TerminalNotificationSettings | null;
  loading: boolean;
  loadedOnce: boolean;
  modalOpen: boolean;
  updatingChatEnabled: boolean;
  updatingTerminalEnabled: boolean;
  installingIntegration: TerminalNotificationIntegrationId | null;
  load: () => Promise<TerminalNotificationSettings | null>;
  refresh: () => Promise<TerminalNotificationSettings | null>;
  openModal: () => void;
  closeModal: () => void;
  toggle: () => Promise<TerminalNotificationSettings | null>;
  setChatEnabled: (enabled: boolean) => Promise<TerminalNotificationSettings | null>;
  setTerminalEnabled: (enabled: boolean) => Promise<TerminalNotificationSettings | null>;
  disableAll: () => Promise<TerminalNotificationSettings | null>;
  setNotificationSound: (sound: string) => Promise<void>;
  previewSound: (sound: string) => Promise<void>;
  installIntegration: (
    integration: TerminalNotificationIntegrationId,
  ) => Promise<TerminalNotificationSettings | null>;
}

export function patchTerminalNotificationSettings(
  current: TerminalNotificationSettings | null,
  patch: Partial<TerminalNotificationSettings>,
): TerminalNotificationSettings | null {
  if (!current) {
    return null;
  }
  return {
    ...current,
    ...patch,
  };
}

export function normalizeNotificationSound(sound: string): string | null {
  return sound === "none" ? null : sound;
}

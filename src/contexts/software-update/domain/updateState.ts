export type UpdateStatus =
  | "idle"
  | "checking"
  | "available"
  | "downloading"
  | "ready"
  | "error";

export interface UpdateState {
  status: UpdateStatus;
  version: string | null;
  error: string | null;
  /** True after user clicks "Not now"; hides the update dot until next app launch. */
  snoozed: boolean;

  checkForUpdate: () => Promise<void>;
  downloadAndInstall: () => Promise<void>;
  resetToIdle: () => void;
  snooze: () => void;
}

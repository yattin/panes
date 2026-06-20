import type { HelperStatus, KeepAwakeState, PowerSettings, PowerSettingsInput } from "../../../types";

export interface KeepAwakeStoreState {
  state: KeepAwakeState | null;
  loading: boolean;
  loadedOnce: boolean;
  powerSettingsLoading: boolean;
  powerSettingsLoaded: boolean;
  load: () => Promise<KeepAwakeState | null>;
  refresh: () => Promise<KeepAwakeState | null>;
  toggle: () => Promise<KeepAwakeState | null>;
  powerSettings: PowerSettings | null;
  powerSettingsOpen: boolean;
  loadPowerSettings: () => Promise<PowerSettings | null>;
  savePowerSettings: (input: PowerSettingsInput) => Promise<KeepAwakeState | null>;
  openPowerSettings: () => void;
  closePowerSettings: () => void;
  helperStatus: HelperStatus | null;
  helperLoading: boolean;
  loadHelperStatus: () => Promise<HelperStatus | null>;
  registerHelper: () => Promise<HelperStatus | null>;
}

export function canToggleKeepAwake(state: KeepAwakeState | null | undefined) {
  return state?.supported !== false || state?.enabled === true;
}

export function hasClosedDisplayLimitation(state: KeepAwakeState) {
  return state.supportsClosedDisplay === false && state.closedDisplayActive === false;
}

import { create } from "zustand";
import { t } from "../../../i18n";
import { toast } from "../../shell-ui/application/toastStore";
import type { KeepAwakeState } from "../../../types";
import {
  canToggleKeepAwake,
  hasClosedDisplayLimitation,
  type KeepAwakeStoreState,
} from "../domain/keepAwake";
import { getPowerManagementGateway } from "./powerManagementGateway";

const KEEP_AWAKE_TOAST_KEYS = {
  enabled: "app:commandPalette.toasts.keepAwakeEnabled",
  enabledLimited: "app:commandPalette.toasts.keepAwakeEnabledLimited",
  disabled: "app:commandPalette.toasts.keepAwakeDisabled",
  unsupported: "app:commandPalette.toasts.keepAwakeUnsupported",
  enableFailed: "app:commandPalette.toasts.keepAwakeEnableFailed",
  disableFailed: "app:commandPalette.toasts.keepAwakeDisableFailed",
  settingsSaved: "app:commandPalette.toasts.powerSettingsSaved",
  settingsSaveFailed: "app:commandPalette.toasts.powerSettingsSaveFailed",
  helperInstallSuccess: "app:commandPalette.toasts.helperInstallSuccess",
  helperInstallFailed: "app:commandPalette.toasts.helperInstallFailed",
  helperApprovalRequired: "app:commandPalette.toasts.helperApprovalRequired",
} as const;

function showKeepAwakeToast(nextState: KeepAwakeState, targetEnabled: boolean) {
  if (!nextState.supported) {
    toast.warning(t(KEEP_AWAKE_TOAST_KEYS.unsupported));
    return;
  }

  if (targetEnabled && (!nextState.enabled || !nextState.active)) {
    toast.error(t(KEEP_AWAKE_TOAST_KEYS.enableFailed));
    return;
  }

  if (!targetEnabled && (nextState.enabled || nextState.active)) {
    toast.error(t(KEEP_AWAKE_TOAST_KEYS.disableFailed));
    return;
  }

  if (targetEnabled && hasClosedDisplayLimitation(nextState)) {
    toast.warning(t(KEEP_AWAKE_TOAST_KEYS.enabledLimited));
    return;
  }

  toast.success(t(targetEnabled ? KEEP_AWAKE_TOAST_KEYS.enabled : KEEP_AWAKE_TOAST_KEYS.disabled));
}

let pendingKeepAwakeState: Promise<KeepAwakeState | null> | null = null;
let keepAwakeRequestId = 0;
let keepAwakeLastAppliedRequestId = 0;
let keepAwakePendingRequests = 0;
let keepAwakeMutationId = 0;
let keepAwakePendingMutations = 0;
let powerSettingsLoadRequestId = 0;

function beginKeepAwakeRequest(set: (partial: Partial<KeepAwakeStoreState>) => void) {
  keepAwakePendingRequests += 1;
  set({ loading: true });
  keepAwakeRequestId += 1;
  return keepAwakeRequestId;
}

function finishKeepAwakeRequest(set: (partial: Partial<KeepAwakeStoreState>) => void) {
  keepAwakePendingRequests = Math.max(0, keepAwakePendingRequests - 1);
  set({ loading: keepAwakePendingRequests > 0 });
}

function beginKeepAwakeMutation(set: (partial: Partial<KeepAwakeStoreState>) => void) {
  keepAwakeMutationId += 1;
  keepAwakePendingMutations += 1;
  return {
    requestId: beginKeepAwakeRequest(set),
    mutationId: keepAwakeMutationId,
  };
}

function finishKeepAwakeMutation(set: (partial: Partial<KeepAwakeStoreState>) => void) {
  keepAwakePendingMutations = Math.max(0, keepAwakePendingMutations - 1);
  finishKeepAwakeRequest(set);
}

function applyKeepAwakeReadState(
  requestId: number,
  set: (partial: Partial<KeepAwakeStoreState>) => void,
  readMutationId: number,
  state: KeepAwakeState,
) {
  if (keepAwakePendingMutations > 0 || readMutationId !== keepAwakeMutationId) {
    return false;
  }

  if (requestId < keepAwakeLastAppliedRequestId) {
    return false;
  }

  keepAwakeLastAppliedRequestId = requestId;
  set({
    state,
    loadedOnce: true,
  });
  return true;
}

function applyKeepAwakeMutationState(
  requestId: number,
  set: (partial: Partial<KeepAwakeStoreState>) => void,
  mutationId: number,
  state: KeepAwakeState,
) {
  if (mutationId !== keepAwakeMutationId) {
    return false;
  }

  keepAwakeLastAppliedRequestId = Math.max(keepAwakeLastAppliedRequestId, requestId);
  set({
    state,
    loadedOnce: true,
  });
  return true;
}

function requestKeepAwakeState(
  set: (partial: Partial<KeepAwakeStoreState>) => void,
  get: () => KeepAwakeStoreState,
) {
  if (pendingKeepAwakeState) {
    return pendingKeepAwakeState;
  }

  const requestId = beginKeepAwakeRequest(set);
  const readMutationId = keepAwakeMutationId;
  const request = (async () => {
    try {
      const state = await getPowerManagementGateway().getKeepAwakeState();
      return applyKeepAwakeReadState(requestId, set, readMutationId, state)
        ? state
        : get().state;
    } catch (error) {
      console.warn("[keepAwakeStore] Failed to load keep awake state", error);
      set({ loadedOnce: true });
      return get().state;
    } finally {
      finishKeepAwakeRequest(set);
    }
  })();

  pendingKeepAwakeState = request;
  request.finally(() => {
    if (pendingKeepAwakeState === request) {
      pendingKeepAwakeState = null;
    }
  });
  return request;
}

export const useKeepAwakeStore = create<KeepAwakeStoreState>((set, get) => ({
  state: null,
  loading: false,
  loadedOnce: false,
  powerSettingsLoading: false,
  powerSettingsLoaded: false,
  powerSettings: null,
  powerSettingsOpen: false,

  load: async () => requestKeepAwakeState(set, get),

  refresh: async () => requestKeepAwakeState(set, get),

  toggle: async () => {
    const current = get().state ?? await get().load();
    if (!current) {
      return null;
    }

    if (!canToggleKeepAwake(current)) {
      toast.warning(t(KEEP_AWAKE_TOAST_KEYS.unsupported));
      return current;
    }

    const targetEnabled = !current.enabled;
    const { requestId, mutationId } = beginKeepAwakeMutation(set);
    try {
      const nextState = await getPowerManagementGateway().setKeepAwakeEnabled(targetEnabled);
      applyKeepAwakeMutationState(requestId, set, mutationId, nextState);
      showKeepAwakeToast(nextState, targetEnabled);
      return nextState;
    } catch (error) {
      console.warn("[keepAwakeStore] Failed to toggle keep awake", error);
      toast.error(t(targetEnabled ? KEEP_AWAKE_TOAST_KEYS.enableFailed : KEEP_AWAKE_TOAST_KEYS.disableFailed));
      return get().state;
    } finally {
      finishKeepAwakeMutation(set);
    }
  },

  loadPowerSettings: async () => {
    powerSettingsLoadRequestId += 1;
    const requestId = powerSettingsLoadRequestId;
    set({
      powerSettingsLoading: true,
      powerSettingsLoaded: false,
      powerSettings: null,
    });
    try {
      const settings = await getPowerManagementGateway().getPowerSettings();
      if (requestId !== powerSettingsLoadRequestId) {
        return settings;
      }
      set({
        powerSettings: settings,
        powerSettingsLoading: false,
        powerSettingsLoaded: true,
      });
      return settings;
    } catch (error) {
      console.warn("[keepAwakeStore] Failed to load power settings", error);
      if (requestId !== powerSettingsLoadRequestId) {
        return null;
      }
      set({
        powerSettings: null,
        powerSettingsLoading: false,
        powerSettingsLoaded: false,
      });
      return null;
    }
  },

  savePowerSettings: async (input) => {
    const { requestId, mutationId } = beginKeepAwakeMutation(set);
    try {
      const nextState = await getPowerManagementGateway().setPowerSettings(input);
      applyKeepAwakeMutationState(requestId, set, mutationId, nextState);
      set({
        powerSettings: { ...input },
        powerSettingsLoaded: true,
      });
      toast.success(t(KEEP_AWAKE_TOAST_KEYS.settingsSaved));
      return nextState;
    } catch (error) {
      console.warn("[keepAwakeStore] Failed to save power settings", error);
      toast.error(t(KEEP_AWAKE_TOAST_KEYS.settingsSaveFailed));
      return null;
    } finally {
      finishKeepAwakeMutation(set);
    }
  },

  openPowerSettings: () => set({ powerSettingsOpen: true }),
  closePowerSettings: () => set({ powerSettingsOpen: false }),

  helperStatus: null,
  helperLoading: false,

  loadHelperStatus: async () => {
    set({ helperLoading: true });
    try {
      const status = await getPowerManagementGateway().getHelperStatus();
      set({ helperStatus: status, helperLoading: false });
      return status;
    } catch (error) {
      console.warn("[keepAwakeStore] Failed to load helper status", error);
      set({ helperLoading: false });
      return null;
    }
  },

  registerHelper: async () => {
    set({ helperLoading: true });
    try {
      const status = await getPowerManagementGateway().registerKeepAwakeHelper();
      set({ helperStatus: status, helperLoading: false });
      if (status.status === "registered") {
        toast.success(t(KEEP_AWAKE_TOAST_KEYS.helperInstallSuccess));
      } else if (status.status === "requiresApproval") {
        toast.warning(t(KEEP_AWAKE_TOAST_KEYS.helperApprovalRequired));
      }
      return status;
    } catch (error) {
      console.warn("[keepAwakeStore] Failed to register helper", error);
      toast.error(t(KEEP_AWAKE_TOAST_KEYS.helperInstallFailed));
      set({ helperLoading: false });
      return null;
    }
  },
}));

import { create } from "zustand";
import type { UpdateState } from "../domain/updateState";
import { getUpdateGateway } from "./updateGateway";

export const useUpdateStore = create<UpdateState>((set, get) => ({
  status: "idle",
  version: null,
  error: null,
  snoozed: false,

  checkForUpdate: async () => {
    if (get().status === "checking") return;
    set({ status: "checking", error: null });
    try {
      const update = await getUpdateGateway().checkForAvailableUpdate();
      if (update) {
        set({ status: "available", version: update.version });
      } else {
        set({ status: "idle" });
      }
    } catch {
      set({ status: "idle" });
    }
  },

  downloadAndInstall: async () => {
    set({ status: "downloading", error: null });
    try {
      const update = await getUpdateGateway().checkForAvailableUpdate();
      if (!update) {
        set({ status: "idle" });
        return;
      }
      await update.downloadAndInstall();
      set({ status: "ready" });
      await getUpdateGateway().relaunchAfterUpdate();
    } catch (err) {
      set({
        status: "error",
        error: err instanceof Error ? err.message : "Update failed",
      });
    }
  },

  resetToIdle: () => {
    set({ status: "idle", error: null });
  },

  snooze: () => {
    set({ snoozed: true });
  },
}));

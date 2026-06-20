import { create } from "zustand";
import {
  getInstalledHarnesses,
  type HarnessStore,
} from "../domain/harnessState";
import { getHarnessGateway } from "./harnessGateway";

let pendingHarnessScan: Promise<void> | null = null;

function requestHarnessScan(
  set: (partial: Partial<HarnessStore>) => void,
  get: () => HarnessStore,
) {
  if (pendingHarnessScan) {
    return pendingHarnessScan;
  }

  if (get().phase === "scanning") {
    return Promise.resolve();
  }

  set({ phase: "scanning", error: null });
  const request = (async () => {
    try {
      const report = await getHarnessGateway().checkHarnesses();
      set({
        harnesses: report.harnesses,
        npmAvailable: report.npmAvailable,
        phase: "idle",
        error: null,
        loadedOnce: true,
      });
    } catch (err) {
      set({
        phase: "error",
        error: err instanceof Error ? err.message : String(err),
        loadedOnce: true,
      });
    } finally {
      pendingHarnessScan = null;
    }
  })();

  pendingHarnessScan = request;
  return request;
}

export const useHarnessStore = create<HarnessStore>((set, get) => ({
  phase: "idle",
  harnesses: [],
  npmAvailable: false,
  error: null,
  loadedOnce: false,

  scan: async () => requestHarnessScan(set, get),

  ensureScanned: async () => {
    if (get().loadedOnce) {
      return;
    }
    await requestHarnessScan(set, get);
  },

  launch: async (harnessId) => {
    try {
      return await getHarnessGateway().launchHarness(harnessId);
    } catch {
      return null;
    }
  },

  getInstalledHarnesses: () => getInstalledHarnesses(get().harnesses),
}));

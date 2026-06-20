import type { HarnessInfo } from "../../../types";

export type HarnessPhase = "idle" | "scanning" | "error";

export interface HarnessStore {
  phase: HarnessPhase;
  harnesses: HarnessInfo[];
  npmAvailable: boolean;
  error: string | null;
  loadedOnce: boolean;
  scan: () => Promise<void>;
  ensureScanned: () => Promise<void>;
  launch: (harnessId: string) => Promise<string | null>;
  getInstalledHarnesses: () => HarnessInfo[];
}

export function getInstalledHarnesses(harnesses: HarnessInfo[]): HarnessInfo[] {
  return harnesses.filter((harness) => harness.found);
}

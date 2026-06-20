import { ipc } from "../../../lib/ipc";
import type { HarnessReport } from "../../../types";
import type { HarnessGateway } from "../application/harnessGateway";

export async function checkHarnesses(): Promise<HarnessReport> {
  return ipc.checkHarnesses();
}

export async function launchHarness(harnessId: string): Promise<string> {
  return ipc.launchHarness(harnessId);
}

export const harnessRepository: HarnessGateway = {
  checkHarnesses,
  launchHarness,
};

import type { HarnessReport } from "../../../types";

export interface HarnessGateway {
  checkHarnesses(): Promise<HarnessReport>;
  launchHarness(harnessId: string): Promise<string>;
}

let configuredHarnessGateway: HarnessGateway | null = null;

export function configureHarnessGateway(gateway: HarnessGateway): void {
  configuredHarnessGateway = gateway;
}

export function getHarnessGateway(): HarnessGateway {
  if (!configuredHarnessGateway) {
    throw new Error("HarnessGateway has not been configured.");
  }
  return configuredHarnessGateway;
}

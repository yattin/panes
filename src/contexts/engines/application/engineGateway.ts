import type { EngineHealth, EngineInfo } from "../../../types";

export interface EngineGateway {
  listEngines(): Promise<EngineInfo[]>;
  getEngineHealth(engineId: string): Promise<EngineHealth>;
}

let configuredEngineGateway: EngineGateway | null = null;

export function configureEngineGateway(gateway: EngineGateway): void {
  configuredEngineGateway = gateway;
}

export function getEngineGateway(): EngineGateway {
  if (!configuredEngineGateway) {
    throw new Error("EngineGateway has not been configured.");
  }
  return configuredEngineGateway;
}

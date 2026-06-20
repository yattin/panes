import type {
  HelperStatus,
  KeepAwakeState,
  PowerSettings,
  PowerSettingsInput,
} from "../../../types";

export interface PowerManagementGateway {
  getHelperStatus(): Promise<HelperStatus>;
  getKeepAwakeState(): Promise<KeepAwakeState>;
  getPowerSettings(): Promise<PowerSettings>;
  registerKeepAwakeHelper(): Promise<HelperStatus>;
  setKeepAwakeEnabled(enabled: boolean): Promise<KeepAwakeState>;
  setPowerSettings(input: PowerSettingsInput): Promise<KeepAwakeState>;
}

let configuredPowerManagementGateway: PowerManagementGateway | null = null;

export function configurePowerManagementGateway(gateway: PowerManagementGateway): void {
  configuredPowerManagementGateway = gateway;
}

export function getPowerManagementGateway(): PowerManagementGateway {
  if (!configuredPowerManagementGateway) {
    throw new Error("PowerManagementGateway has not been configured.");
  }
  return configuredPowerManagementGateway;
}

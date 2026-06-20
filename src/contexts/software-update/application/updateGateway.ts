export interface AvailableUpdate {
  version: string;
  downloadAndInstall: () => Promise<void>;
}

export interface UpdateGateway {
  checkForAvailableUpdate(): Promise<AvailableUpdate | null>;
  relaunchAfterUpdate(): Promise<void>;
}

let configuredUpdateGateway: UpdateGateway | null = null;

export function configureUpdateGateway(gateway: UpdateGateway): void {
  configuredUpdateGateway = gateway;
}

export function getUpdateGateway(): UpdateGateway {
  if (!configuredUpdateGateway) {
    throw new Error("UpdateGateway has not been configured.");
  }
  return configuredUpdateGateway;
}

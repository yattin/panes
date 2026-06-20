import { getShellUiGateway } from "./shellUiGateway";

export function getAppVersion(): Promise<string> {
  return getShellUiGateway().getAppVersion();
}

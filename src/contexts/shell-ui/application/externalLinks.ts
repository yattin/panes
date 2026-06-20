import { getShellUiGateway } from "./shellUiGateway";

export function openExternalUrl(url: string): Promise<void> {
  return getShellUiGateway().openExternalUrl(url);
}

import {
  getShellUiGateway,
  type WindowFileDropPayload,
} from "./shellUiGateway";

export type { WindowFileDropPayload };

export function listenWindowFileDrops(
  onDropEvent: (payload: WindowFileDropPayload) => void,
): Promise<() => void> {
  return getShellUiGateway().listenWindowFileDrops(onDropEvent);
}

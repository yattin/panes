import { ipc } from "../../../lib/ipc";

export const getTerminalAcceleratedRenderingPreference =
  ipc.getTerminalAcceleratedRendering;
export const setTerminalAcceleratedRenderingPreference =
  ipc.setTerminalAcceleratedRendering;
export {
  emitTerminalAcceleratedRenderingChanged,
  getTerminalAcceleratedRenderingPreferenceVersion,
  listenTerminalAcceleratedRenderingChanged,
} from "../application/terminalRenderingSettings";
export type { TerminalAcceleratedRenderingEventDetail } from "../application/terminalRenderingSettings";

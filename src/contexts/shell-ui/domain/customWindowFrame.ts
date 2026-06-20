export interface CustomWindowFrameState {
  isFullscreen: boolean;
  isMaximized: boolean;
}

export const DEFAULT_CUSTOM_WINDOW_FRAME_STATE: CustomWindowFrameState = {
  isFullscreen: false,
  isMaximized: false,
};

export function canCustomWindowResize(frameState: CustomWindowFrameState): boolean {
  return !(frameState.isFullscreen || frameState.isMaximized);
}

export function shouldShowCustomWindowChrome(frameState: CustomWindowFrameState): boolean {
  return !frameState.isFullscreen;
}

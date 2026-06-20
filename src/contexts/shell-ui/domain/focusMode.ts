import type { ShellVisibilityState } from "./uiState";

export function enterFocusMode(state: ShellVisibilityState): Partial<ShellVisibilityState> {
  if (state.focusMode) {
    return state;
  }

  return {
    focusMode: true,
    focusModeSnapshot: {
      showSidebar: state.showSidebar,
      showGitPanel: state.showGitPanel,
    },
    showSidebar: false,
  };
}

export function leaveFocusMode(state: ShellVisibilityState): Partial<ShellVisibilityState> {
  if (!state.focusMode) {
    return state;
  }

  const snapshot = state.focusModeSnapshot;
  return {
    focusMode: false,
    focusModeSnapshot: null,
    showSidebar: snapshot?.showSidebar ?? state.showSidebar,
    showGitPanel: snapshot?.showGitPanel ?? state.showGitPanel,
  };
}

export function toggleFocusModeState(
  state: ShellVisibilityState,
): Partial<ShellVisibilityState> {
  return state.focusMode ? leaveFocusMode(state) : enterFocusMode(state);
}

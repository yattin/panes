import { create } from "zustand";
import {
  clearWorkspaceComposerRuntime,
  type ChatComposerState,
  setWorkspaceComposerRuntime,
} from "../domain/chatComposerState";

export const useChatComposerStore = create<ChatComposerState>((set) => ({
  runtimeByWorkspace: {},
  setWorkspaceRuntime: (workspaceId, runtime) =>
    set((state) => ({
      runtimeByWorkspace: setWorkspaceComposerRuntime(
        state.runtimeByWorkspace,
        workspaceId,
        runtime,
      ),
    })),
  clearWorkspaceRuntime: (workspaceId) =>
    set((state) => ({
      runtimeByWorkspace: clearWorkspaceComposerRuntime(
        state.runtimeByWorkspace,
        workspaceId,
      ),
    })),
}));

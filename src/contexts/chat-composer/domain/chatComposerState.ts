export type ComposerRuntimeServiceTier = "fast" | "flex";

export interface ComposerRuntimeSnapshot {
  engineId: string;
  modelId: string;
  reasoningEffort: string | null;
  serviceTier: ComposerRuntimeServiceTier | null;
}

export interface ChatComposerState {
  runtimeByWorkspace: Record<string, ComposerRuntimeSnapshot>;
  setWorkspaceRuntime: (
    workspaceId: string,
    runtime: ComposerRuntimeSnapshot,
  ) => void;
  clearWorkspaceRuntime: (workspaceId: string) => void;
}

export function setWorkspaceComposerRuntime(
  runtimeByWorkspace: Record<string, ComposerRuntimeSnapshot>,
  workspaceId: string,
  runtime: ComposerRuntimeSnapshot,
): Record<string, ComposerRuntimeSnapshot> {
  return {
    ...runtimeByWorkspace,
    [workspaceId]: runtime,
  };
}

export function clearWorkspaceComposerRuntime(
  runtimeByWorkspace: Record<string, ComposerRuntimeSnapshot>,
  workspaceId: string,
): Record<string, ComposerRuntimeSnapshot> {
  const { [workspaceId]: _removed, ...rest } = runtimeByWorkspace;
  return rest;
}

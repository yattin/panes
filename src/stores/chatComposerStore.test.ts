import { beforeEach, describe, expect, it } from "vitest";
import type { ComposerRuntimeSnapshot } from "../contexts/threads/domain/newThreadRuntime";
import { useChatComposerStore } from "./chatComposerStore";

function runtime(modelId: string): ComposerRuntimeSnapshot {
  return {
    engineId: "codex",
    modelId,
    reasoningEffort: "high",
    serviceTier: "fast",
  };
}

describe("chatComposerStore", () => {
  beforeEach(() => {
    useChatComposerStore.setState({ runtimeByWorkspace: {} });
  });

  it("keeps composer runtimes isolated by workspace", () => {
    useChatComposerStore.getState().setWorkspaceRuntime("workspace-a", runtime("gpt-5.4"));
    useChatComposerStore.getState().setWorkspaceRuntime("workspace-b", runtime("gpt-5.4-mini"));

    useChatComposerStore.getState().clearWorkspaceRuntime("workspace-a");

    expect(useChatComposerStore.getState().runtimeByWorkspace).toEqual({
      "workspace-b": runtime("gpt-5.4-mini"),
    });
  });
});

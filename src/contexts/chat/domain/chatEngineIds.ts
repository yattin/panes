import type { ChatEngineId } from "../../../types";

/**
 * Normalize legacy engine IDs to their current equivalents.
 * Maps deprecated "claude-code-native" to "claurst-native".
 */
export function normalizeEngineId(engineId: string): string {
  if (engineId === "claude-code-native") {
    return "claurst-native";
  }
  return engineId;
}

export function isClaudeFamilyEngine(
  engineId?: string | null,
): engineId is Extract<ChatEngineId, "claude" | "claude-code-native" | "claurst-native"> {
  return (
    engineId === "claude" ||
    engineId === "claude-code-native" ||
    engineId === "claurst-native"
  );
}

import type { ChatEngineId } from "../types";

export function isClaudeFamilyEngine(
  engineId?: string | null,
): engineId is Extract<ChatEngineId, "claude" | "claude-code-native"> {
  return engineId === "claude" || engineId === "claude-code-native";
}

import type {
  OnboardingChatEngineId,
  OnboardingWorkflowPreference,
} from "../../../types";

export const CHAT_ENGINE_ORDER: OnboardingChatEngineId[] = [
  "claude-code-native",
  "codex",
  "claude",
  "opencode",
];

export const DEFAULT_CHAT_ENGINES: OnboardingChatEngineId[] = ["claude-code-native"];

export function normalizeOnboardingWorkflow(
  value: string | null,
): OnboardingWorkflowPreference | null {
  return value === "cli" || value === "chat" ? value : null;
}

export function normalizeOnboardingChatEngines(
  values: Iterable<unknown>,
): OnboardingChatEngineId[] {
  const selected = new Set<OnboardingChatEngineId>();

  for (const value of values) {
    if (
      value === "codex" ||
      value === "claude" ||
      value === "claude-code-native" ||
      value === "opencode"
    ) {
      selected.add(value);
    }
  }

  return CHAT_ENGINE_ORDER.filter((engine) => selected.has(engine));
}

import type {
  OnboardingChatEngineId,
  OnboardingWorkflowPreference,
} from "../../../types";
import {
  LEGACY_SETUP_COMPLETED_KEY,
  ONBOARDING_CHAT_ENGINES_KEY,
  ONBOARDING_COMPLETED_KEY,
  ONBOARDING_WORKFLOW_KEY,
  type StoredOnboardingState,
} from "../application/onboardingGateway";
import {
  DEFAULT_CHAT_ENGINES,
  normalizeOnboardingChatEngines,
  normalizeOnboardingWorkflow,
} from "../domain/onboardingPreferences";

function readBooleanKey(key: string): boolean {
  try {
    return localStorage.getItem(key) === "1";
  } catch {
    return false;
  }
}

function writeBooleanKey(key: string, value: boolean): void {
  try {
    if (value) {
      localStorage.setItem(key, "1");
      return;
    }
    localStorage.removeItem(key);
  } catch {
    // Ignore persistence failures in tests or restricted environments.
  }
}

function readWorkflow(): OnboardingWorkflowPreference | null {
  try {
    return normalizeOnboardingWorkflow(localStorage.getItem(ONBOARDING_WORKFLOW_KEY));
  } catch {
    return null;
  }
}

export function writeOnboardingWorkflow(
  workflow: OnboardingWorkflowPreference | null,
): void {
  try {
    if (!workflow) {
      localStorage.removeItem(ONBOARDING_WORKFLOW_KEY);
      return;
    }
    localStorage.setItem(ONBOARDING_WORKFLOW_KEY, workflow);
  } catch {
    // Ignore persistence failures in tests or restricted environments.
  }
}

function readChatEngines(): OnboardingChatEngineId[] {
  try {
    const raw = localStorage.getItem(ONBOARDING_CHAT_ENGINES_KEY);
    if (raw === null) {
      return [...DEFAULT_CHAT_ENGINES];
    }

    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) {
      return [];
    }

    return normalizeOnboardingChatEngines(parsed);
  } catch {
    return [];
  }
}

export function writeOnboardingChatEngines(engines: OnboardingChatEngineId[]): void {
  try {
    localStorage.setItem(
      ONBOARDING_CHAT_ENGINES_KEY,
      JSON.stringify(normalizeOnboardingChatEngines(engines)),
    );
  } catch {
    // Ignore persistence failures in tests or restricted environments.
  }
}

export function writeOnboardingCompleted(value: boolean): void {
  writeBooleanKey(ONBOARDING_COMPLETED_KEY, value);
}

export function readStoredOnboardingState(): StoredOnboardingState {
  return {
    completed: readBooleanKey(ONBOARDING_COMPLETED_KEY),
    legacyCompleted: readBooleanKey(LEGACY_SETUP_COMPLETED_KEY),
    preferredWorkflow: readWorkflow(),
    selectedChatEngines: readChatEngines(),
  };
}

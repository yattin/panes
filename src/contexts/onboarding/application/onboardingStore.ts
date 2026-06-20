import { create } from "zustand";
import type {
  OnboardingChatEngineId,
  OnboardingStep,
  OnboardingWorkflowPreference,
} from "../../../types";
import { normalizeOnboardingHarnessInstallId } from "../domain/onboardingFlow";
import { normalizeOnboardingChatEngines } from "../domain/onboardingPreferences";
import {
  LEGACY_SETUP_COMPLETED_KEY,
  ONBOARDING_CHAT_ENGINES_KEY,
  ONBOARDING_COMPLETED_KEY,
  ONBOARDING_WORKFLOW_KEY,
  getOnboardingGateway,
} from "./onboardingGateway";

export {
  LEGACY_SETUP_COMPLETED_KEY,
  ONBOARDING_CHAT_ENGINES_KEY,
  ONBOARDING_COMPLETED_KEY,
  ONBOARDING_WORKFLOW_KEY,
};

export interface OnboardingInstallLogEntry {
  dep: string;
  line: string;
  stream: string;
}

export interface OnboardingInstallTarget {
  kind: "dependency" | "harness";
  id: string;
  label: string;
}

interface OnboardingState {
  open: boolean;
  completed: boolean;
  legacyCompleted: boolean;
  step: OnboardingStep;
  preferredWorkflow: OnboardingWorkflowPreference | null;
  selectedChatEngines: OnboardingChatEngineId[];
  selectedWorkspaceId: string | null;
  installLog: OnboardingInstallLogEntry[];
  installing: OnboardingInstallTarget | null;
  error: string | null;
  openOnboarding: () => void;
  closeOnboarding: () => void;
  isCompleted: () => boolean;
  hasLegacyCompletion: () => boolean;
  setStep: (step: OnboardingStep) => void;
  setPreferredWorkflow: (workflow: OnboardingWorkflowPreference | null) => void;
  setSelectedChatEngines: (engines: OnboardingChatEngineId[]) => void;
  toggleChatEngine: (engine: OnboardingChatEngineId) => void;
  setSelectedWorkspaceId: (workspaceId: string | null) => void;
  clearInstallState: () => void;
  installDependency: (dependency: string, method: string, label?: string) => Promise<boolean>;
  installHarness: (harnessId: string, label?: string) => Promise<boolean>;
  complete: () => void;
}

function describeInstallTarget(
  kind: "dependency" | "harness",
  id: string,
  label?: string,
): OnboardingInstallTarget {
  return {
    kind,
    id,
    label: label ?? id,
  };
}

export function readStoredOnboardingState() {
  return getOnboardingGateway().readStoredOnboardingState();
}

export function hydrateOnboardingPreferences(): void {
  useOnboardingStore.setState(readStoredOnboardingState());
}

export const useOnboardingStore = create<OnboardingState>((set, get) => ({
  open: false,
  completed: false,
  legacyCompleted: false,
  preferredWorkflow: null,
  selectedChatEngines: [],
  step: "greeting",
  selectedWorkspaceId: null,
  installLog: [],
  installing: null,
  error: null,

  openOnboarding: () =>
    set((state) => ({
      open: true,
      step: "greeting",
      selectedWorkspaceId: null,
      installLog: [],
      installing: null,
      error: null,
      preferredWorkflow: state.preferredWorkflow,
      selectedChatEngines: state.selectedChatEngines,
    })),
  closeOnboarding: () => set({ open: false, installLog: [], error: null, installing: null }),
  isCompleted: () => get().completed,
  hasLegacyCompletion: () => get().legacyCompleted,
  setStep: (step) => set({ step, error: null }),
  setPreferredWorkflow: (workflow) => {
    getOnboardingGateway().writeOnboardingWorkflow(workflow);
    set({ preferredWorkflow: workflow });
  },
  setSelectedChatEngines: (engines) => {
    const normalized = normalizeOnboardingChatEngines(engines);
    getOnboardingGateway().writeOnboardingChatEngines(normalized);
    set({ selectedChatEngines: normalized });
  },
  toggleChatEngine: (engine) => {
    const current = get().selectedChatEngines;
    const next = current.includes(engine)
      ? current.filter((entry) => entry !== engine)
      : [...current, engine];
    get().setSelectedChatEngines(next);
  },
  setSelectedWorkspaceId: (workspaceId) => set({ selectedWorkspaceId: workspaceId }),
  clearInstallState: () => set({ installLog: [], installing: null, error: null }),
  installDependency: async (dependency, method, label) => {
    if (get().installing) {
      return false;
    }

    set({
      installing: describeInstallTarget("dependency", dependency, label),
      error: null,
    });

    let unlisten: (() => void) | null = null;

    try {
      const gateway = getOnboardingGateway();
      unlisten = await gateway.listenInstallProgress((event) => {
        set((state) => ({
          installLog: [
            ...state.installLog,
            { dep: event.dependency, line: event.line, stream: event.stream },
          ],
        }));
      });
      const result = await gateway.installDependency(dependency, method);
      return result.success;
    } catch (error) {
      set({ error: error instanceof Error ? error.message : String(error) });
      return false;
    } finally {
      set({ installing: null });
      unlisten?.();
    }
  },
  installHarness: async (harnessId, label) => {
    if (get().installing) {
      return false;
    }

    const installId = normalizeOnboardingHarnessInstallId(harnessId);
    set({
      installing: describeInstallTarget("harness", installId, label),
      error: null,
    });

    let unlisten: (() => void) | null = null;

    try {
      const gateway = getOnboardingGateway();
      unlisten = await gateway.listenInstallProgress((event) => {
        set((state) => ({
          installLog: [
            ...state.installLog,
            { dep: event.dependency, line: event.line, stream: event.stream },
          ],
        }));
      });
      const result = await gateway.installHarness(installId);
      return result.success;
    } catch (error) {
      set({ error: error instanceof Error ? error.message : String(error) });
      return false;
    } finally {
      set({ installing: null });
      unlisten?.();
    }
  },
  complete: () => {
    getOnboardingGateway().writeOnboardingCompleted(true);
    set({ completed: true, open: false, error: null, installing: null });
  },
}));

export type { OnboardingState };

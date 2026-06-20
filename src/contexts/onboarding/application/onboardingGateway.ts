import type {
  DependencyReport,
  InstallProgressEvent,
  InstallResult,
  OnboardingChatEngineId,
  OnboardingWorkflowPreference,
} from "../../../types";

export const LEGACY_SETUP_COMPLETED_KEY = "panes.setup.completed.v2";
export const ONBOARDING_COMPLETED_KEY = "panes.onboarding.completed.v1";
export const ONBOARDING_WORKFLOW_KEY = "panes.onboarding.workflow.v1";
export const ONBOARDING_CHAT_ENGINES_KEY = "panes.onboarding.chatEngines.v1";

export interface StoredOnboardingState {
  completed: boolean;
  legacyCompleted: boolean;
  preferredWorkflow: OnboardingWorkflowPreference | null;
  selectedChatEngines: OnboardingChatEngineId[];
}

export interface OnboardingGateway {
  checkDependencies(): Promise<DependencyReport>;
  installDependency(dependency: string, method: string): Promise<InstallResult>;
  installHarness(harnessId: string): Promise<InstallResult>;
  listenInstallProgress(onEvent: (event: InstallProgressEvent) => void): Promise<() => void>;
  readStoredOnboardingState(): StoredOnboardingState;
  writeOnboardingChatEngines(engines: OnboardingChatEngineId[]): void;
  writeOnboardingCompleted(value: boolean): void;
  writeOnboardingWorkflow(workflow: OnboardingWorkflowPreference | null): void;
}

let configuredOnboardingGateway: OnboardingGateway | null = null;

export function configureOnboardingGateway(gateway: OnboardingGateway): void {
  configuredOnboardingGateway = gateway;
}

export function getOnboardingGateway(): OnboardingGateway {
  if (!configuredOnboardingGateway) {
    throw new Error("OnboardingGateway has not been configured.");
  }
  return configuredOnboardingGateway;
}

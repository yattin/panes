import * as ipcModule from "../../../lib/ipc";
import type { OnboardingGateway } from "../application/onboardingGateway";
import {
  readStoredOnboardingState,
  writeOnboardingChatEngines,
  writeOnboardingCompleted,
  writeOnboardingWorkflow,
} from "./onboardingPreferencesStorage";

const { ipc } = ipcModule;

type ListenInstallProgress = typeof ipcModule.listenInstallProgress;

function listenInstallProgress(
  onEvent: Parameters<ListenInstallProgress>[0],
): ReturnType<ListenInstallProgress> {
  return (ipcModule as { listenInstallProgress: ListenInstallProgress })
    .listenInstallProgress(onEvent);
}

export const onboardingRepository = {
  checkDependencies: ipc.checkDependencies,
  installDependency: ipc.installDependency,
  installHarness: ipc.installHarness,
  listenInstallProgress,
};

export const onboardingGateway: OnboardingGateway = {
  checkDependencies: onboardingRepository.checkDependencies,
  installDependency: onboardingRepository.installDependency,
  installHarness: onboardingRepository.installHarness,
  listenInstallProgress: onboardingRepository.listenInstallProgress,
  readStoredOnboardingState,
  writeOnboardingChatEngines,
  writeOnboardingCompleted,
  writeOnboardingWorkflow,
};

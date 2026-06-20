import type {
  DependencyReport,
  EngineHealth,
  OnboardingChatEngineId,
} from "../../../types";
import { getEngineGateway } from "../../engines/application/engineGateway";
import { getOnboardingGateway } from "./onboardingGateway";

export function checkOnboardingDependencies(): Promise<DependencyReport> {
  return getOnboardingGateway().checkDependencies();
}

export function getOnboardingEngineHealth(
  engineId: OnboardingChatEngineId,
): Promise<EngineHealth> {
  return getEngineGateway().getEngineHealth(engineId);
}

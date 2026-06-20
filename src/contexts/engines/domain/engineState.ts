import type { EngineHealth, EngineInfo, EngineRuntimeUpdatedEvent } from "../../../types";

export interface EngineState {
  engines: EngineInfo[];
  health: Record<string, EngineHealth>;
  healthLoading: Record<string, boolean>;
  loading: boolean;
  loadedOnce: boolean;
  error?: string;
  load: () => Promise<void>;
  ensureHealth: (
    engineId: string,
    options?: { force?: boolean },
  ) => Promise<EngineHealth | null>;
  mergeHealth: (reports: EngineHealth[]) => void;
  applyRuntimeUpdate: (event: EngineRuntimeUpdatedEvent) => void;
}

import { create } from "zustand";
import type { EngineHealth } from "../../../types";
import {
  applyEngineRuntimeUpdate,
  buildEngineDiscoveryFailureHealth,
} from "../domain/engineHealth";
import type { EngineState } from "../domain/engineState";
import { getEngineGateway } from "./engineGateway";

let pendingHealthRequests: Partial<Record<string, Promise<EngineHealth | null>>> = {};

export const useEngineStore = create<EngineState>((set, get) => ({
  engines: [],
  health: {},
  healthLoading: {},
  loading: false,
  loadedOnce: false,
  load: async () => {
    set({ loading: true, error: undefined });
    try {
      const engines = await getEngineGateway().listEngines();
      set({
        engines,
        loading: false,
        loadedOnce: true,
        error: undefined,
      });
    } catch (error) {
      const message = String(error);
      set({
        loading: false,
        loadedOnce: true,
        error: message,
        health: {
          codex: buildEngineDiscoveryFailureHealth(message),
        },
      });
    }
  },
  ensureHealth: async (engineId, options) => {
    const existing = get().health[engineId];
    if (existing && !options?.force) {
      return existing;
    }

    if (pendingHealthRequests[engineId]) {
      return pendingHealthRequests[engineId];
    }

    set((state) => {
      if (
        state.healthLoading[engineId] ||
        (!options?.force && state.health[engineId])
      ) {
        return state;
      }

      return {
        healthLoading: {
          ...state.healthLoading,
          [engineId]: true,
        },
      };
    });

    const request = (async () => {
      try {
        const health = await getEngineGateway().getEngineHealth(engineId);
        set((state) => {
          const { [engineId]: _ignored, ...rest } = state.healthLoading;
          return {
            health: {
              ...state.health,
              [health.id]: health,
            },
            healthLoading: rest,
          };
        });
        return health;
      } catch (error) {
        const message = String(error);
        set((state) => {
          const { [engineId]: _ignored, ...rest } = state.healthLoading;
          return {
            healthLoading: rest,
            error: `${engineId}: ${message}`,
          };
        });
        return null;
      } finally {
        delete pendingHealthRequests[engineId];
      }
    })();

    pendingHealthRequests[engineId] = request;
    return request;
  },
  mergeHealth: (reports) =>
    set((state) => {
      if (reports.length === 0) {
        return state;
      }

      const nextHealth = { ...state.health };
      const nextHealthLoading = { ...state.healthLoading };
      for (const report of reports) {
        nextHealth[report.id] = report;
        delete nextHealthLoading[report.id];
      }

      return {
        health: nextHealth,
        healthLoading: nextHealthLoading,
      };
    }),
  applyRuntimeUpdate: (event) =>
    set((state) => {
      const nextHealth = applyEngineRuntimeUpdate(state.health[event.engineId], event);
      const { [event.engineId]: _ignored, ...rest } = state.healthLoading;

      return {
        health: {
          ...state.health,
          [event.engineId]: nextHealth,
        },
        healthLoading: rest,
      };
    }),
}));

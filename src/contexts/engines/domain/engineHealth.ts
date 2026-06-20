import type { EngineHealth, EngineRuntimeUpdatedEvent } from "../../../types";

export function buildEngineDiscoveryFailureHealth(message: string): EngineHealth {
  return {
    id: "codex",
    available: false,
    details: `Engine discovery failed: ${message}`,
    warnings: [],
    checks: ["codex --version", "command -v codex"],
    fixes: [],
  };
}

export function applyEngineRuntimeUpdate(
  current: EngineHealth | undefined,
  event: EngineRuntimeUpdatedEvent,
): EngineHealth {
  if (current) {
    return {
      ...current,
      available: true,
      details: current.available ? current.details : undefined,
      protocolDiagnostics: event.protocolDiagnostics ?? current.protocolDiagnostics,
    };
  }

  return {
    id: event.engineId,
    available: true,
    warnings: [],
    checks: [],
    fixes: [],
    protocolDiagnostics: event.protocolDiagnostics,
  };
}

import type {
  WorkspaceStartupPreset,
  WorkspaceStartupPresetFormat,
} from "../../../types";
import { getTerminalSessionGateway } from "../../terminal-sessions/application/terminalSessionGateway";
import { getWorkspaceGateway } from "./workspaceGateway";

export const workspaceStartupPresets = {
  clearWorkspaceStartupPreset(workspaceId: string): Promise<void> {
    return getWorkspaceGateway().clearWorkspaceStartupPreset(workspaceId);
  },
  getWorkspaceStartupPreset(workspaceId: string): Promise<WorkspaceStartupPreset | null> {
    return getWorkspaceGateway().getWorkspaceStartupPreset(workspaceId);
  },
  normalizeWorkspaceStartupPreset(
    workspaceId: string,
    preset: WorkspaceStartupPreset,
  ): Promise<WorkspaceStartupPreset> {
    return getWorkspaceGateway().normalizeWorkspaceStartupPreset(workspaceId, preset);
  },
  normalizeWorkspaceStartupPresetRaw(
    workspaceId: string,
    format: WorkspaceStartupPresetFormat,
    raw: string,
  ): Promise<WorkspaceStartupPreset> {
    return getWorkspaceGateway().normalizeWorkspaceStartupPresetRaw(workspaceId, format, raw);
  },
  serializeWorkspaceStartupPreset(
    workspaceId: string,
    preset: WorkspaceStartupPreset,
    format: WorkspaceStartupPresetFormat,
  ): Promise<string> {
    return getWorkspaceGateway().serializeWorkspaceStartupPreset(workspaceId, preset, format);
  },
  setWorkspaceStartupPreset(
    workspaceId: string,
    preset: WorkspaceStartupPreset,
  ): Promise<WorkspaceStartupPreset> {
    return getWorkspaceGateway().setWorkspaceStartupPreset(workspaceId, preset);
  },
  setWorkspaceStartupPresetRaw(
    workspaceId: string,
    format: WorkspaceStartupPresetFormat,
    raw: string,
  ): Promise<WorkspaceStartupPreset> {
    return getWorkspaceGateway().setWorkspaceStartupPresetRaw(workspaceId, format, raw);
  },
  terminalListSessions(workspaceId: string) {
    return getTerminalSessionGateway().terminalListSessions(workspaceId);
  },
};

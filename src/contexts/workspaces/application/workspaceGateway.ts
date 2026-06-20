import type {
  CueLightProjectBinding,
  Repo,
  TrustLevel,
  Workspace,
  WorkspaceStartupPreset,
  WorkspaceStartupPresetFormat,
} from "../../../types";

export type LastRepoByWorkspace = Record<string, string>;

export interface WorkspaceGitSelectionStatus {
  configured: boolean;
}

export interface WorkspaceGateway {
  archiveWorkspace(workspaceId: string): Promise<void>;
  bindCueLightProject(
    workspaceId: string,
    binding: { projectId: string; projectName: string },
  ): Promise<void>;
  clearWorkspaceStartupPreset(workspaceId: string): Promise<void>;
  getCueLightBinding(workspaceId: string): Promise<CueLightProjectBinding | null>;
  getRepos(workspaceId: string): Promise<Repo[]>;
  getWorkspaceStartupPreset(workspaceId: string): Promise<WorkspaceStartupPreset | null>;
  hasWorkspaceGitSelection(workspaceId: string): Promise<WorkspaceGitSelectionStatus>;
  listArchivedWorkspaces(): Promise<Workspace[]>;
  listWorkspaces(): Promise<Workspace[]>;
  normalizeWorkspaceStartupPreset(
    workspaceId: string,
    preset: WorkspaceStartupPreset,
  ): Promise<WorkspaceStartupPreset>;
  normalizeWorkspaceStartupPresetRaw(
    workspaceId: string,
    format: WorkspaceStartupPresetFormat,
    raw: string,
  ): Promise<WorkspaceStartupPreset>;
  openWorkspace(path: string, scanDepth?: number): Promise<Workspace>;
  readLastRepoByWorkspace(): LastRepoByWorkspace;
  readLastWorkspaceId(): string | null;
  rememberLastRepo(workspaceId: string, repoId: string): void;
  revealWorkspacePath(path: string): Promise<void>;
  restoreWorkspace(workspaceId: string): Promise<Workspace>;
  serializeWorkspaceStartupPreset(
    workspaceId: string,
    preset: WorkspaceStartupPreset,
    format: WorkspaceStartupPresetFormat,
  ): Promise<string>;
  setRepoGitActive(repoId: string, isActive: boolean): Promise<void>;
  setRepoTrustLevel(repoId: string, trustLevel: TrustLevel): Promise<void>;
  setWorkspaceStartupPreset(
    workspaceId: string,
    preset: WorkspaceStartupPreset,
  ): Promise<WorkspaceStartupPreset>;
  setWorkspaceStartupPresetRaw(
    workspaceId: string,
    format: WorkspaceStartupPresetFormat,
    raw: string,
  ): Promise<WorkspaceStartupPreset>;
  setWorkspaceGitActiveRepos(workspaceId: string, repoIds: string[]): Promise<void>;
  unbindCueLightProject(workspaceId: string): Promise<void>;
  writeLastWorkspaceId(workspaceId: string): void;
}

let configuredWorkspaceGateway: WorkspaceGateway | null = null;

export function configureWorkspaceGateway(gateway: WorkspaceGateway): void {
  configuredWorkspaceGateway = gateway;
}

export function getWorkspaceGateway(): WorkspaceGateway {
  if (!configuredWorkspaceGateway) {
    throw new Error("WorkspaceGateway has not been configured.");
  }
  return configuredWorkspaceGateway;
}

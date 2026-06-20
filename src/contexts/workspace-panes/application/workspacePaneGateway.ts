import type { WorkspacePaneLayout } from "../domain/workspacePaneLayout";

export interface WorkspacePaneGateway {
  createId(prefix: string): string;
  persistLayout(workspaceId: string, layout: WorkspacePaneLayout): void;
  readLayout(workspaceId: string): WorkspacePaneLayout | null;
}

let configuredWorkspacePaneGateway: WorkspacePaneGateway | null = null;

export function configureWorkspacePaneGateway(gateway: WorkspacePaneGateway): void {
  configuredWorkspacePaneGateway = gateway;
}

export function getWorkspacePaneGateway(): WorkspacePaneGateway {
  if (!configuredWorkspacePaneGateway) {
    throw new Error("WorkspacePaneGateway has not been configured.");
  }
  return configuredWorkspacePaneGateway;
}

import type { ResolvedEditorFileReference } from "../../../types";

export interface FileNavigationGateway {
  resolveEditorFileReference(
    workspaceId: string,
    rawReference: string,
    preferredRepoPath?: string | null,
    currentCwd?: string | null,
  ): Promise<ResolvedEditorFileReference | null>;
}

let configuredFileNavigationGateway: FileNavigationGateway | null = null;

export function configureFileNavigationGateway(gateway: FileNavigationGateway): void {
  configuredFileNavigationGateway = gateway;
}

export function getFileNavigationGateway(): FileNavigationGateway {
  if (!configuredFileNavigationGateway) {
    throw new Error("FileNavigationGateway has not been configured.");
  }
  return configuredFileNavigationGateway;
}

import type { CueLightProjectBinding } from "../../../types";

export interface CueLightProjectOption {
  id: string;
  name: string;
  projectType?: string;
}

export interface CueLightGateway {
  get<T>(
    binding: CueLightProjectBinding,
    path: string,
    query?: Record<string, string>,
  ): Promise<T>;
  listProjects(token?: string | null): Promise<CueLightProjectOption[]>;
  maximizeWindow(): Promise<void>;
  openServer(): Promise<void>;
  readToken(): string | null;
  saveToken(token: string): void;
  syncAuthToken(token: string): Promise<void>;
  validateToken(token: string): Promise<void>;
}

let configuredCueLightGateway: CueLightGateway | null = null;

export function configureCueLightGateway(gateway: CueLightGateway): void {
  configuredCueLightGateway = gateway;
}

export function getCueLightGateway(): CueLightGateway {
  if (!configuredCueLightGateway) {
    throw new Error("CueLightGateway has not been configured.");
  }
  return configuredCueLightGateway;
}

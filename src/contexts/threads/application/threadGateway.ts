import type { Thread } from "../../../types";
import type { NewThreadServiceTier } from "../domain/newThreadRuntime";

export interface ThreadGateway {
  archiveThread(threadId: string): Promise<void>;
  attachCodexRemoteThread(
    workspaceId: string,
    engineThreadId: string,
    modelId: string,
  ): Promise<Thread>;
  attachOpenCodeRemoteSession(
    workspaceId: string,
    engineThreadId: string,
    cwd: string,
    modelId: string,
  ): Promise<Thread>;
  clearLastActiveThreadId(): void;
  compactCodexThread(threadId: string): Promise<Thread>;
  createThread(
    workspaceId: string,
    repoId: string | null,
    engineId: string,
    modelId: string,
    title: string,
    reasoningEffort?: string | null,
    serviceTier?: NewThreadServiceTier | null,
  ): Promise<Thread>;
  forkCodexThread(threadId: string): Promise<Thread>;
  listArchivedThreads(workspaceId: string): Promise<Thread[]>;
  listThreads(workspaceId: string): Promise<Thread[]>;
  readLastActiveThreadId(): string | null;
  renameThread(threadId: string, title: string): Promise<Thread>;
  restoreThread(threadId: string): Promise<Thread>;
  rollbackCodexThread(threadId: string, numTurns: number): Promise<Thread>;
  writeLastActiveThreadId(threadId: string): void;
}

let configuredThreadGateway: ThreadGateway | null = null;

export function configureThreadGateway(gateway: ThreadGateway): void {
  configuredThreadGateway = gateway;
}

export function getThreadGateway(): ThreadGateway {
  if (!configuredThreadGateway) {
    throw new Error("ThreadGateway has not been configured.");
  }
  return configuredThreadGateway;
}

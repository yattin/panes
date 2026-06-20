import type {
  FileTreeEntry,
  FileTreePage,
  GitCompareSource,
  GitFileCompare,
  ReadFileResult,
} from "../../../types";

export interface FileEditorGateway {
  createDir(rootPath: string, dirPath: string, workspaceId?: string | null): Promise<void>;
  createEditorRevealNonce(): string;
  createEditorTabId(): string;
  createFile(rootPath: string, filePath: string, workspaceId?: string | null): Promise<void>;
  deletePath(rootPath: string, filePath: string, workspaceId?: string | null): Promise<void>;
  destroyEditorRuntimeCache(cacheKey: string): void;
  getGitFileCompare(
    repoPath: string,
    filePath: string,
    source: GitCompareSource,
  ): Promise<GitFileCompare>;
  listDir(rootPath: string, dirPath: string): Promise<FileTreeEntry[]>;
  openPathWithDefaultApp(path: string): Promise<void>;
  readFile(rootPath: string, filePath: string): Promise<ReadFileResult>;
  renamePath(
    rootPath: string,
    oldPath: string,
    newName: string,
    workspaceId?: string | null,
  ): Promise<void>;
  revealPath(path: string): Promise<void>;
  searchWorkspaceFiles(
    workspaceId: string,
    query: string,
    offset?: number,
    limit?: number,
    refresh?: boolean,
  ): Promise<FileTreePage>;
  writeFile(
    rootPath: string,
    filePath: string,
    content: string,
    workspaceId: string | null,
  ): Promise<void>;
}

let configuredFileEditorGateway: FileEditorGateway | null = null;

export function configureFileEditorGateway(gateway: FileEditorGateway): void {
  configuredFileEditorGateway = gateway;
}

export function getFileEditorGateway(): FileEditorGateway {
  if (!configuredFileEditorGateway) {
    throw new Error("FileEditorGateway has not been configured.");
  }
  return configuredFileEditorGateway;
}

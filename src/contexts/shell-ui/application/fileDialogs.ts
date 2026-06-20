import { getShellUiGateway, type FileDialogFilter } from "./shellUiGateway";

export type { FileDialogFilter };

export async function selectDirectoryPath(): Promise<string | null> {
  return getShellUiGateway().selectDirectoryPath();
}

export async function selectFilePaths(options: {
  multiple?: boolean;
  title?: string;
  filters?: FileDialogFilter[];
}): Promise<string[]> {
  return getShellUiGateway().selectFilePaths(options);
}

export async function selectTextFile(options: {
  title?: string;
  filters?: FileDialogFilter[];
}): Promise<{ path: string; text: string } | null> {
  return getShellUiGateway().selectTextFile(options);
}

export async function saveTextFile(options: {
  title?: string;
  defaultPath?: string;
  filters?: FileDialogFilter[];
  text: string;
}): Promise<boolean> {
  return getShellUiGateway().saveTextFile(options);
}

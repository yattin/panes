import type { AppLocale } from "../domain/appLocale";

export interface FileDialogFilter {
  name: string;
  extensions: string[];
}

export type WindowFileDropPayload =
  | {
      type: "drop";
      paths: string[];
      position: { x: number; y: number };
    }
  | {
      type: "over";
      paths: string[];
      position: { x: number; y: number };
    }
  | {
      type: "enter";
      paths: string[];
      position: { x: number; y: number };
    }
  | {
      type: "leave";
    };

export type WindowResizeDirection =
  | "East"
  | "North"
  | "NorthEast"
  | "NorthWest"
  | "South"
  | "SouthEast"
  | "SouthWest"
  | "West";

export interface ShellUiGateway {
  closeNativeWindow(): Promise<void>;
  destroyNativeWindow(): Promise<void>;
  getAppVersion(): Promise<string>;
  getPersistedAppLocale(): Promise<AppLocale | null>;
  hideNativeWindow(): Promise<void>;
  isNativeWindowFullscreen(): Promise<boolean>;
  isTauriRuntime(): boolean;
  listenWindowFileDrops(
    onDropEvent: (payload: WindowFileDropPayload) => void,
  ): Promise<() => void>;
  minimizeNativeWindow(): Promise<void>;
  now(): number;
  openExternalUrl(url: string): Promise<void>;
  readExplorerOpenPreference(): boolean | null;
  readGitPanelPinnedPreference(): boolean | null;
  readSidebarPinnedPreference(): boolean | null;
  saveTextFile(options: {
    title?: string;
    defaultPath?: string;
    filters?: FileDialogFilter[];
    text: string;
  }): Promise<boolean>;
  selectDirectoryPath(): Promise<string | null>;
  selectFilePaths(options: {
    multiple?: boolean;
    title?: string;
    filters?: FileDialogFilter[];
  }): Promise<string[]>;
  selectTextFile(options: {
    title?: string;
    filters?: FileDialogFilter[];
  }): Promise<{ path: string; text: string } | null>;
  setNativeWindowFullscreen(fullscreen: boolean): Promise<void>;
  setPersistedAppLocale(locale: AppLocale): Promise<AppLocale>;
  startNativeWindowDrag(): Promise<void>;
  startNativeWindowResizeDrag(direction: WindowResizeDirection): Promise<void>;
  toggleNativeWindowMaximize(): Promise<void>;
  writeExplorerOpenPreference(open: boolean): void;
  writeGitPanelPinnedPreference(pinned: boolean): void;
  writeSidebarPinnedPreference(pinned: boolean): void;
}

let configuredShellUiGateway: ShellUiGateway | null = null;

export function configureShellUiGateway(gateway: ShellUiGateway): void {
  configuredShellUiGateway = gateway;
}

export function getShellUiGateway(): ShellUiGateway {
  if (!configuredShellUiGateway) {
    throw new Error("ShellUiGateway has not been configured.");
  }
  return configuredShellUiGateway;
}

import { getVersion } from "@tauri-apps/api/app";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open, save } from "@tauri-apps/plugin-dialog";
import { readTextFile, writeTextFile } from "@tauri-apps/plugin-fs";
import { open as openExternal } from "@tauri-apps/plugin-shell";
import type {
  FileDialogFilter,
  WindowFileDropPayload,
  WindowResizeDirection,
} from "../application/shellUiGateway";

export function getAppVersion(): Promise<string> {
  return getVersion();
}

export function isTauriRuntime(): boolean {
  return isTauri();
}

export function openExternalUrl(url: string): Promise<void> {
  return openExternal(url);
}

export function closeNativeWindow(): Promise<void> {
  return getCurrentWindow().close();
}

export function destroyNativeWindow(): Promise<void> {
  return getCurrentWindow().destroy();
}

export function hideNativeWindow(): Promise<void> {
  return getCurrentWindow().hide();
}

export function minimizeNativeWindow(): Promise<void> {
  return getCurrentWindow().minimize();
}

export function toggleNativeWindowMaximize(): Promise<void> {
  return getCurrentWindow().toggleMaximize();
}

export function isNativeWindowFullscreen(): Promise<boolean> {
  return getCurrentWindow().isFullscreen();
}

export function setNativeWindowFullscreen(fullscreen: boolean): Promise<void> {
  return getCurrentWindow().setFullscreen(fullscreen);
}

export function startNativeWindowDrag(): Promise<void> {
  return getCurrentWindow().startDragging();
}

export function startNativeWindowResizeDrag(direction: WindowResizeDirection): Promise<void> {
  return getCurrentWindow().startResizeDragging(direction);
}

export function listenWindowFileDrops(
  onDropEvent: (payload: WindowFileDropPayload) => void,
): Promise<() => void> {
  return getCurrentWindow().onDragDropEvent((event) => {
    onDropEvent(event.payload as WindowFileDropPayload);
  });
}

export async function selectDirectoryPath(): Promise<string | null> {
  const selected = await open({ directory: true, multiple: false });
  if (!selected || Array.isArray(selected)) {
    return null;
  }

  return selected;
}

export async function selectFilePaths(options: {
  multiple?: boolean;
  title?: string;
  filters?: FileDialogFilter[];
}): Promise<string[]> {
  const selected = await open(options);
  if (!selected) {
    return [];
  }

  return Array.isArray(selected) ? selected : [selected];
}

export async function selectTextFile(options: {
  title?: string;
  filters?: FileDialogFilter[];
}): Promise<{ path: string; text: string } | null> {
  const [path] = await selectFilePaths({ ...options, multiple: false });
  if (!path) {
    return null;
  }

  return {
    path,
    text: await readTextFile(path),
  };
}

export async function saveTextFile(options: {
  title?: string;
  defaultPath?: string;
  filters?: FileDialogFilter[];
  text: string;
}): Promise<boolean> {
  const target = await save({
    title: options.title,
    defaultPath: options.defaultPath,
    filters: options.filters,
  });
  if (!target) {
    return false;
  }

  await writeTextFile(target, options.text);
  return true;
}

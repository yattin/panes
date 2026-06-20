import { ipc } from "../../../lib/ipc";
import type { FileEditorGateway } from "../application/fileEditorGateway";
import {
  createEditorRevealNonce,
  createEditorTabId,
} from "./editorIdGenerator";
import { destroyEditorRuntimeCache } from "./editorRuntimeCache";

export const fileRepository = {
  createDir: ipc.createDir,
  createFile: ipc.createFile,
  deletePath: ipc.deletePath,
  getGitFileCompare: ipc.getGitFileCompare,
  listDir: ipc.listDir,
  openPathWithDefaultApp: ipc.openPathWithDefaultApp,
  readFile: ipc.readFile,
  renamePath: ipc.renamePath,
  revealPath: ipc.revealPath,
  searchWorkspaceFiles: ipc.searchWorkspaceFiles,
  writeFile: ipc.writeFile,
};

export const fileEditorGateway: FileEditorGateway = {
  createDir: fileRepository.createDir,
  createEditorRevealNonce,
  createEditorTabId,
  createFile: fileRepository.createFile,
  deletePath: fileRepository.deletePath,
  destroyEditorRuntimeCache,
  getGitFileCompare: fileRepository.getGitFileCompare,
  listDir: fileRepository.listDir,
  openPathWithDefaultApp: fileRepository.openPathWithDefaultApp,
  readFile: fileRepository.readFile,
  renamePath: fileRepository.renamePath,
  revealPath: fileRepository.revealPath,
  searchWorkspaceFiles: fileRepository.searchWorkspaceFiles,
  writeFile: fileRepository.writeFile,
};

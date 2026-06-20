import { create } from "zustand";
import {
  resolveAbsoluteFilePath,
  resolveOwningRepoForAbsolutePath,
} from "../../file-navigation/domain/pathRoots";
import { t } from "../../../i18n";
import { useGitStore } from "../../git/application/gitStore";
import { toast } from "../../shell-ui/application/toastStore";
import { useWorkspaceStore } from "../../workspaces/application/workspaceStore";
import {
  applyGitCompare,
  createPlainTab,
  createRevealRequest,
  defaultOpenRenderMode,
  retargetEditorTabAfterRename,
  toMarkdownPreviewTab,
  toOpenedFileTab,
  toPlainEditorTab,
  type ResolvedFileContext,
} from "../domain/editorTabs";
import { getFileEditorGateway } from "./fileEditorGateway";
import type {
  EditorRevealLocation,
  EditorRenderMode,
  EditorTab,
  GitCompareSource,
} from "../../../types";

function resolveFileContext(rootPath: string, filePath: string): ResolvedFileContext {
  const absolutePath = resolveAbsoluteFilePath(rootPath, filePath);
  const workspaceState = useWorkspaceStore.getState();
  const ownership = resolveOwningRepoForAbsolutePath(
    absolutePath,
    workspaceState.repos,
    workspaceState.activeRepoId,
  );

  return {
    absolutePath,
    gitRepoPath: ownership?.repo.path ?? null,
    gitFilePath: ownership?.filePath ?? null,
  };
}

export interface FileStoreState {
  tabs: EditorTab[];
  activeTabId: string | null;
  pendingCloseTabId: string | null;
  openFile: (rootPath: string, filePath: string) => Promise<void>;
  openFileAtLocation: (
    rootPath: string,
    filePath: string,
    reveal?: EditorRevealLocation | null,
  ) => Promise<void>;
  openGitDiffFile: (
    repoPath: string,
    filePath: string,
    options: { source: GitCompareSource },
  ) => Promise<void>;
  refreshGitContext: (tabId: string, source?: GitCompareSource) => Promise<void>;
  closeTab: (tabId: string) => void;
  requestCloseTab: (tabId: string) => void;
  confirmCloseTab: () => void;
  cancelCloseTab: () => void;
  setActiveTab: (tabId: string) => void;
  setTabRenderMode: (tabId: string, renderMode: EditorRenderMode) => void;
  setTabContent: (tabId: string, content: string) => void;
  clearPendingReveal: (tabId: string, nonce: string) => void;
  retargetTabsAfterRename: (
    rootPath: string,
    oldPath: string,
    newPath: string,
  ) => void;
  saveTab: (tabId: string) => Promise<void>;
}

export const useFileStore = create<FileStoreState>((set, get) => ({
  tabs: [],
  activeTabId: null,
  pendingCloseTabId: null,

  openFile: async (rootPath, filePath) => {
    await get().openFileAtLocation(rootPath, filePath);
  },

  openFileAtLocation: async (rootPath, filePath, reveal) => {
    const workspaceId = useWorkspaceStore.getState().activeWorkspaceId;
    const resolved = resolveFileContext(rootPath, filePath);
    const existing = get().tabs.find((tab) => tab.absolutePath === resolved.absolutePath);
    const pendingReveal = createRevealRequest(
      reveal,
      getFileEditorGateway().createEditorRevealNonce,
    );
    const renderMode = defaultOpenRenderMode(filePath, pendingReveal);
    if (existing) {
      getFileEditorGateway().destroyEditorRuntimeCache(`${existing.id}:git-base`);
      getFileEditorGateway().destroyEditorRuntimeCache(`${existing.id}:git-modified`);
      const nextRenderMode = existing.isBinary ? "plain-editor" : renderMode;
      set((state) => ({
        activeTabId: existing.id,
        tabs: state.tabs.map((tab) =>
          tab.id === existing.id
            ? {
                ...toOpenedFileTab(tab, pendingReveal, nextRenderMode),
                workspaceId,
                rootPath,
                filePath,
                gitRepoPath: resolved.gitRepoPath ?? tab.gitRepoPath,
                gitFilePath: resolved.gitFilePath ?? tab.gitFilePath,
              }
            : tab,
        ),
      }));
      return;
    }

    const id = getFileEditorGateway().createEditorTabId();
    const tab = createPlainTab(id, workspaceId, rootPath, filePath, resolved);

    set((state) => ({
      tabs: [...state.tabs, tab],
      activeTabId: id,
    }));

    try {
      const result = await getFileEditorGateway().readFile(rootPath, filePath);
      set((state) => ({
        tabs: state.tabs.map((t) =>
          t.id === id
            ? {
                ...toOpenedFileTab(
                  t,
                  pendingReveal,
                  result.isBinary ? "plain-editor" : renderMode,
                ),
                content: result.content,
                savedContent: result.content,
                isBinary: result.isBinary,
                isLoading: false,
              }
            : t,
        ),
      }));
    } catch (err) {
      set((state) => ({
        tabs: state.tabs.map((t) =>
          t.id === id
            ? { ...t, isLoading: false, loadError: String(err) }
            : t,
        ),
      }));
    }
  },

  openGitDiffFile: async (repoPath, filePath, options) => {
    const workspaceId = useWorkspaceStore.getState().activeWorkspaceId;
    const resolved = {
      absolutePath: resolveAbsoluteFilePath(repoPath, filePath),
      gitRepoPath: repoPath,
      gitFilePath: filePath,
    };
    const existing = get().tabs.find((tab) => tab.absolutePath === resolved.absolutePath);
    const tabId = existing?.id ?? getFileEditorGateway().createEditorTabId();

    if (existing) {
      getFileEditorGateway().destroyEditorRuntimeCache(existing.id);
      set((state) => ({
        activeTabId: existing.id,
        tabs: state.tabs.map((tab) =>
          tab.id === existing.id
            ? {
                ...tab,
                workspaceId,
                gitRepoPath: repoPath,
                gitFilePath: filePath,
                isLoading: true,
                renderMode: "git-diff-editor",
                pendingReveal: null,
                loadError: undefined,
              }
            : tab,
        ),
      }));
    } else {
      const tab = {
        ...createPlainTab(tabId, workspaceId, repoPath, filePath, resolved),
        renderMode: "git-diff-editor" as const,
      };
      set((state) => ({
        tabs: [...state.tabs, tab],
        activeTabId: tabId,
      }));
    }

    await get().refreshGitContext(tabId, options.source);
  },

  refreshGitContext: async (tabId, source) => {
    const tab = get().tabs.find((item) => item.id === tabId);
    if (!tab) return;

    const compareSource = source ?? tab.gitContext?.source;
    if (!compareSource || !tab.gitRepoPath || !tab.gitFilePath) return;

    try {
      const compare = await getFileEditorGateway().getGitFileCompare(
        tab.gitRepoPath,
        tab.gitFilePath,
        compareSource,
      );
      set((state) => ({
        tabs: state.tabs.map((item) =>
          item.id === tabId ? applyGitCompare(item, compare) : item,
        ),
      }));
    } catch (err) {
      set((state) => ({
        tabs: state.tabs.map((item) =>
          item.id === tabId
            ? {
                ...item,
                isLoading: false,
                renderMode: "git-diff-editor",
                loadError: String(err),
              }
            : item,
        ),
      }));
    }
  },

  closeTab: (tabId) => {
    getFileEditorGateway().destroyEditorRuntimeCache(tabId);
    getFileEditorGateway().destroyEditorRuntimeCache(`${tabId}:git-base`);
    getFileEditorGateway().destroyEditorRuntimeCache(`${tabId}:git-modified`);
    set((state) => {
      const index = state.tabs.findIndex((t) => t.id === tabId);
      if (index === -1) return state;

      const newTabs = state.tabs.filter((t) => t.id !== tabId);
      let newActiveId = state.activeTabId;

      if (state.activeTabId === tabId) {
        if (newTabs.length === 0) {
          newActiveId = null;
        } else {
          const nextIndex = Math.min(index, newTabs.length - 1);
          newActiveId = newTabs[nextIndex].id;
        }
      }

      return { tabs: newTabs, activeTabId: newActiveId, pendingCloseTabId: null };
    });
  },

  requestCloseTab: (tabId) => {
    const tab = get().tabs.find((t) => t.id === tabId);
    if (!tab) return;
    if (tab.isDirty) {
      set({ pendingCloseTabId: tabId });
    } else {
      get().closeTab(tabId);
    }
  },

  confirmCloseTab: () => {
    const { pendingCloseTabId } = get();
    if (pendingCloseTabId) {
      get().closeTab(pendingCloseTabId);
    }
  },

  cancelCloseTab: () => {
    set({ pendingCloseTabId: null });
  },

  setActiveTab: (tabId) => {
    set({ activeTabId: tabId });
  },

  setTabRenderMode: (tabId, renderMode) => {
    set((state) => ({
      tabs: state.tabs.map((tab) => {
        if (tab.id !== tabId) {
          return tab;
        }

        if (renderMode === "plain-editor") {
          return toPlainEditorTab(tab, null);
        }

        if (renderMode === "markdown-preview") {
          return toMarkdownPreviewTab(tab);
        }

        return tab;
      }),
    }));
  },

  setTabContent: (tabId, content) => {
    set((state) => ({
      tabs: state.tabs.map((t) =>
        t.id === tabId
          ? { ...t, content, isDirty: content !== t.savedContent }
          : t,
      ),
    }));
  },

  clearPendingReveal: (tabId, nonce) => {
    set((state) => ({
      tabs: state.tabs.map((tab) =>
        tab.id === tabId && tab.pendingReveal?.nonce === nonce
          ? { ...tab, pendingReveal: null }
          : tab,
      ),
    }));
  },

  retargetTabsAfterRename: (rootPath, oldPath, newPath) => {
    const workspaceState = useWorkspaceStore.getState();

    set((state) => ({
      tabs: state.tabs.map((tab) =>
        retargetEditorTabAfterRename(tab, {
          rootPath,
          oldPath,
          newPath,
          repos: workspaceState.repos,
          activeRepoId: workspaceState.activeRepoId,
        }),
      ),
    }));
  },

  saveTab: async (tabId) => {
    const tab = get().tabs.find((t) => t.id === tabId);
    if (!tab || !tab.isDirty) return;

    // Check if the file was modified externally since we loaded/last-saved it
    try {
      const disk = await getFileEditorGateway().readFile(tab.rootPath, tab.filePath);
      if (!disk.isBinary && disk.content !== tab.savedContent) {
        toast.warning(t("app:editor.toasts.modifiedExternally", { name: tab.fileName }));
        set((state) => ({
          tabs: state.tabs.map((t) =>
            t.id === tabId
              ? { ...t, savedContent: disk.content, isDirty: true }
              : t,
          ),
        }));
        return;
      }
    } catch {
      // File may have been deleted — proceed with save
    }

    const contentToSave = tab.content;
    try {
      await getFileEditorGateway().writeFile(
        tab.rootPath,
        tab.filePath,
        contentToSave,
        tab.workspaceId,
      );
      set((state) => ({
        tabs: state.tabs.map((t) =>
          t.id === tabId
            ? { ...t, savedContent: contentToSave, isDirty: t.content !== contentToSave }
            : t,
        ),
      }));

      if (tab.gitContext && tab.gitRepoPath) {
        const gitStore = useGitStore.getState();
        try {
          gitStore.invalidateRepoCache(tab.gitRepoPath);
          await gitStore.refresh(tab.gitRepoPath, { force: true });
          await get().refreshGitContext(tabId, tab.gitContext.source);
        } catch {
          // Saving already succeeded; leave the editor usable even if the git refresh fails.
        }
      }

      toast.success(t("app:editor.toasts.saved", { name: tab.fileName }));
    } catch (err) {
      toast.error(t("app:editor.toasts.saveFailed", { error: String(err) }));
    }
  },
}));

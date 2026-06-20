export type WorkspacePaneLegacyMode = "chat" | "terminal" | "split" | "editor";

export type WorkspacePaneSurfaceKind =
  | "chat"
  | "terminal"
  | "editor"
  | "overview"
  | "storyboard"
  | "assets";

export type WorkspacePaneSplitDirection = "horizontal" | "vertical";

export interface WorkspacePaneTab {
  id: string;
  kind: WorkspacePaneSurfaceKind;
}

export interface WorkspacePaneLeaf {
  type: "leaf";
  id: string;
  tabs: WorkspacePaneTab[];
  activeTabId: string | null;
}

export interface WorkspacePaneSplit {
  type: "split";
  id: string;
  direction: WorkspacePaneSplitDirection;
  ratio: number;
  children: [WorkspacePaneNode, WorkspacePaneNode];
}

export type WorkspacePaneNode = WorkspacePaneLeaf | WorkspacePaneSplit;

export interface WorkspacePaneLayout {
  root: WorkspacePaneNode;
  focusedLeafId: string;
  legacyMode: WorkspacePaneLegacyMode;
}

const DEFAULT_SPLIT_RATIO = 0.66;

export const SURFACE_ORDER: WorkspacePaneSurfaceKind[] = ["chat"];

export type CreateWorkspacePaneId = (prefix: string) => string;

function makeTab(
  kind: WorkspacePaneSurfaceKind,
  createId: CreateWorkspacePaneId,
): WorkspacePaneTab {
  return { id: createId(kind), kind };
}

function makeLeaf(
  createId: CreateWorkspacePaneId,
  kind?: WorkspacePaneSurfaceKind | null,
): WorkspacePaneLeaf {
  const tab = kind ? makeTab(kind, createId) : null;
  return {
    type: "leaf",
    id: createId("pane"),
    tabs: tab ? [tab] : [],
    activeTabId: tab?.id ?? null,
  };
}

function makeSplit(
  direction: WorkspacePaneSplitDirection,
  children: [WorkspacePaneNode, WorkspacePaneNode],
  createId: CreateWorkspacePaneId,
  ratio = DEFAULT_SPLIT_RATIO,
): WorkspacePaneSplit {
  return {
    type: "split",
    id: createId("split"),
    direction,
    ratio,
    children,
  };
}

export function collectWorkspacePaneLeaves(node: WorkspacePaneNode): WorkspacePaneLeaf[] {
  if (node.type === "leaf") {
    return [node];
  }
  return [
    ...collectWorkspacePaneLeaves(node.children[0]),
    ...collectWorkspacePaneLeaves(node.children[1]),
  ];
}

export function getWorkspacePaneActiveTab(leaf: WorkspacePaneLeaf): WorkspacePaneTab | null {
  return leaf.tabs.find((tab) => tab.id === leaf.activeTabId) ?? leaf.tabs[0] ?? null;
}

function findLeaf(node: WorkspacePaneNode, leafId: string): WorkspacePaneLeaf | null {
  if (node.type === "leaf") {
    return node.id === leafId ? node : null;
  }
  return findLeaf(node.children[0], leafId) ?? findLeaf(node.children[1], leafId);
}

function replaceLeaf(
  node: WorkspacePaneNode,
  leafId: string,
  replacement: WorkspacePaneNode,
): WorkspacePaneNode {
  if (node.type === "leaf") {
    return node.id === leafId ? replacement : node;
  }
  return {
    ...node,
    children: [
      replaceLeaf(node.children[0], leafId, replacement),
      replaceLeaf(node.children[1], leafId, replacement),
    ],
  };
}

function removeLeaf(node: WorkspacePaneNode, leafId: string): WorkspacePaneNode | null {
  if (node.type === "leaf") {
    return node.id === leafId ? null : node;
  }

  const left = removeLeaf(node.children[0], leafId);
  const right = removeLeaf(node.children[1], leafId);
  if (!left && !right) {
    return null;
  }
  if (!left) {
    return right;
  }
  if (!right) {
    return left;
  }
  return { ...node, children: [left, right] };
}

function pruneEmptyLeaves(node: WorkspacePaneNode): WorkspacePaneNode | null {
  if (node.type === "leaf") {
    return node.tabs.length > 0 ? node : null;
  }

  const left = pruneEmptyLeaves(node.children[0]);
  const right = pruneEmptyLeaves(node.children[1]);
  if (!left && !right) {
    return null;
  }
  if (!left) {
    return right;
  }
  if (!right) {
    return left;
  }
  return { ...node, children: [left, right] };
}

function updateRatioInTree(
  node: WorkspacePaneNode,
  containerId: string,
  ratio: number,
): WorkspacePaneNode {
  if (node.type === "leaf") {
    return node;
  }
  if (node.id === containerId) {
    return { ...node, ratio };
  }
  return {
    ...node,
    children: [
      updateRatioInTree(node.children[0], containerId, ratio),
      updateRatioInTree(node.children[1], containerId, ratio),
    ],
  };
}

function removeSurfaceKind(
  node: WorkspacePaneNode,
  kind: WorkspacePaneSurfaceKind,
  preserveLeafId?: string,
): WorkspacePaneNode {
  if (node.type === "leaf") {
    if (node.id === preserveLeafId) {
      return node;
    }
    const tabs = node.tabs.filter((tab) => tab.kind !== kind);
    const activeTabId = tabs.some((tab) => tab.id === node.activeTabId)
      ? node.activeTabId
      : tabs[0]?.id ?? null;
    return { ...node, tabs, activeTabId };
  }
  return {
    ...node,
    children: [
      removeSurfaceKind(node.children[0], kind, preserveLeafId),
      removeSurfaceKind(node.children[1], kind, preserveLeafId),
    ],
  };
}

function addSurfaceToLeaf(
  node: WorkspacePaneNode,
  leafId: string,
  kind: WorkspacePaneSurfaceKind,
  createId: CreateWorkspacePaneId,
): { root: WorkspacePaneNode; tabId: string | null } {
  let tabId: string | null = null;
  const targetHasSurface = findLeaf(node, leafId)?.tabs.some((tab) => tab.kind === kind) ?? false;
  const withoutExisting =
    pruneEmptyLeaves(removeSurfaceKind(node, kind, targetHasSurface ? leafId : undefined)) ??
    makeLeaf(createId, kind);

  function add(current: WorkspacePaneNode): WorkspacePaneNode {
    if (current.type === "leaf") {
      if (current.id !== leafId) {
        return current;
      }
      const existing = current.tabs.find((tab) => tab.kind === kind);
      const tab = existing ?? makeTab(kind, createId);
      tabId = tab.id;
      return {
        ...current,
        tabs: existing ? current.tabs : [...current.tabs, tab],
        activeTabId: tab.id,
      };
    }
    return {
      ...current,
      children: [add(current.children[0]), add(current.children[1])],
    };
  }

  return { root: add(withoutExisting), tabId };
}

function leafWithSurface(
  leaf: WorkspacePaneLeaf,
  kind: WorkspacePaneSurfaceKind,
  createId: CreateWorkspacePaneId,
): WorkspacePaneLeaf {
  const tab = makeTab(kind, createId);
  return {
    ...leaf,
    tabs: [tab],
    activeTabId: tab.id,
  };
}

function activateSurfaceInLeafNode(
  node: WorkspacePaneNode,
  leafId: string,
  kind: WorkspacePaneSurfaceKind,
  createId: CreateWorkspacePaneId,
): WorkspacePaneNode {
  const targetLeaf = findLeaf(node, leafId);
  if (!targetLeaf) {
    return node;
  }
  const currentKind = getWorkspacePaneActiveTab(targetLeaf)?.kind ?? null;
  if (currentKind === kind) {
    return node;
  }
  const sourceLeaf = collectWorkspacePaneLeaves(node).find(
    (leaf) => leaf.id !== leafId && leaf.tabs.some((tab) => tab.kind === kind),
  );

  function update(current: WorkspacePaneNode): WorkspacePaneNode {
    if (current.type === "leaf") {
      if (current.id === leafId) {
        return leafWithSurface(current, kind, createId);
      }
      if (sourceLeaf && current.id === sourceLeaf.id && currentKind) {
        return leafWithSurface(current, currentKind, createId);
      }
      return current;
    }
    return { ...current, children: [update(current.children[0]), update(current.children[1])] };
  }

  return update(node);
}

function sanitizeRatio(ratio: number): number {
  if (!Number.isFinite(ratio)) {
    return DEFAULT_SPLIT_RATIO;
  }
  return Math.max(0.12, Math.min(0.88, ratio));
}

function sanitizeTab(value: unknown): WorkspacePaneTab | null {
  if (!value || typeof value !== "object") {
    return null;
  }
  const tab = value as Partial<WorkspacePaneTab>;
  if (!tab.id || typeof tab.id !== "string") {
    return null;
  }
  if (tab.kind !== "chat" && tab.kind !== "terminal" && tab.kind !== "editor") {
    return null;
  }
  return { id: tab.id, kind: tab.kind };
}

function sanitizeNode(value: unknown): WorkspacePaneNode | null {
  if (!value || typeof value !== "object") {
    return null;
  }
  const node = value as Partial<WorkspacePaneNode>;
  if (node.type === "leaf") {
    const rawLeaf = value as Partial<WorkspacePaneLeaf>;
    if (!rawLeaf.id || typeof rawLeaf.id !== "string") {
      return null;
    }
    const tabs = Array.isArray(rawLeaf.tabs)
      ? rawLeaf.tabs.map(sanitizeTab).filter((tab): tab is WorkspacePaneTab => tab !== null)
      : [];
    if (tabs.length === 0) {
      return null;
    }
    const activeTabId =
      typeof rawLeaf.activeTabId === "string" && tabs.some((tab) => tab.id === rawLeaf.activeTabId)
        ? rawLeaf.activeTabId
        : tabs[0]?.id ?? null;
    return { type: "leaf", id: rawLeaf.id, tabs, activeTabId };
  }
  if (node.type === "split") {
    const rawSplit = value as Partial<WorkspacePaneSplit>;
    if (
      !rawSplit.id ||
      typeof rawSplit.id !== "string" ||
      (rawSplit.direction !== "horizontal" && rawSplit.direction !== "vertical") ||
      !Array.isArray(rawSplit.children) ||
      rawSplit.children.length !== 2
    ) {
      return null;
    }
    const left = sanitizeNode(rawSplit.children[0]);
    const right = sanitizeNode(rawSplit.children[1]);
    if (!left && !right) {
      return null;
    }
    if (!left) {
      return right;
    }
    if (!right) {
      return left;
    }
    return {
      type: "split",
      id: rawSplit.id,
      direction: rawSplit.direction,
      ratio: sanitizeRatio(typeof rawSplit.ratio === "number" ? rawSplit.ratio : DEFAULT_SPLIT_RATIO),
      children: [left, right],
    };
  }
  return null;
}

export function sanitizePersistedWorkspacePaneLayout(
  value: unknown,
): WorkspacePaneLayout | null {
  if (!value || typeof value !== "object") {
    return null;
  }
  const parsed = value as Partial<WorkspacePaneLayout>;
  const root = sanitizeNode(parsed.root);
  if (!root) {
    return null;
  }
  const leaves = collectWorkspacePaneLeaves(root);
  const focusedLeafId =
    typeof parsed.focusedLeafId === "string" && leaves.some((leaf) => leaf.id === parsed.focusedLeafId)
      ? parsed.focusedLeafId
      : leaves[0]?.id ?? "";
  const legacyMode =
    parsed.legacyMode === "terminal" ||
    parsed.legacyMode === "split" ||
    parsed.legacyMode === "editor" ||
    parsed.legacyMode === "chat"
      ? parsed.legacyMode
      : deriveLegacyMode(root);
  return { root, focusedLeafId, legacyMode };
}

export function defaultLayoutForLegacyMode(
  mode: WorkspacePaneLegacyMode,
  createId: CreateWorkspacePaneId,
): WorkspacePaneLayout {
  if (mode === "split") {
    const chatLeaf = makeLeaf(createId, "chat");
    const terminalLeaf = makeLeaf(createId, "terminal");
    return {
      root: makeSplit("horizontal", [chatLeaf, terminalLeaf], createId),
      focusedLeafId: chatLeaf.id,
      legacyMode: "split",
    };
  }

  const kind: WorkspacePaneSurfaceKind =
    mode === "terminal" ? "terminal" : mode === "editor" ? "editor" : "chat";
  const leaf = makeLeaf(createId, kind);
  return {
    root: leaf,
    focusedLeafId: leaf.id,
    legacyMode: mode,
  };
}

function deriveLegacyMode(root: WorkspacePaneNode): WorkspacePaneLegacyMode {
  const activeKinds = collectWorkspacePaneLeaves(root)
    .map(getWorkspacePaneActiveTab)
    .filter((tab): tab is WorkspacePaneTab => tab !== null)
    .map((tab) => tab.kind);
  const uniqueKinds = Array.from(new Set(activeKinds));

  if (uniqueKinds.length === 0) {
    return "chat";
  }
  if (uniqueKinds.length === 1) {
    const [kind] = uniqueKinds;
    return kind === "terminal" ? "terminal" : kind === "editor" ? "editor" : "chat";
  }
  if (uniqueKinds.includes("terminal")) {
    return "split";
  }
  if (uniqueKinds.includes("editor")) {
    return "editor";
  }
  return "chat";
}

function chooseSplitSurface(
  layout: WorkspacePaneLayout,
  leafId: string,
): WorkspacePaneSurfaceKind {
  const focusedLeaf = findLeaf(layout.root, leafId);
  const activeKind = focusedLeaf ? getWorkspacePaneActiveTab(focusedLeaf)?.kind ?? null : null;
  const presentKinds = new Set(
    collectWorkspacePaneLeaves(layout.root).flatMap((leaf) => leaf.tabs.map((tab) => tab.kind)),
  );

  const preferred =
    activeKind === "chat"
      ? ["terminal", "editor"]
      : activeKind === "terminal"
        ? ["editor", "chat"]
        : ["chat", "terminal"];

  return (
    preferred.find((kind): kind is WorkspacePaneSurfaceKind =>
      !presentKinds.has(kind as WorkspacePaneSurfaceKind),
    ) ?? (preferred[0] as WorkspacePaneSurfaceKind)
  );
}

export function focusWorkspacePaneLeaf(
  layout: WorkspacePaneLayout,
  leafId: string,
): WorkspacePaneLayout {
  if (!findLeaf(layout.root, leafId)) {
    return layout;
  }
  return { ...layout, focusedLeafId: leafId };
}

export function setWorkspacePaneActiveTab(
  layout: WorkspacePaneLayout,
  leafId: string,
  tabId: string,
): WorkspacePaneLayout {
  let found = false;
  function update(node: WorkspacePaneNode): WorkspacePaneNode {
    if (node.type === "leaf") {
      if (node.id !== leafId || !node.tabs.some((tab) => tab.id === tabId)) {
        return node;
      }
      found = true;
      return { ...node, activeTabId: tabId };
    }
    return { ...node, children: [update(node.children[0]), update(node.children[1])] };
  }
  const root = update(layout.root);
  return found ? { root, focusedLeafId: leafId, legacyMode: deriveLegacyMode(root) } : layout;
}

export function activateWorkspacePaneSurfaceInLeaf(
  layout: WorkspacePaneLayout,
  leafId: string,
  kind: WorkspacePaneSurfaceKind,
  createId: CreateWorkspacePaneId,
): WorkspacePaneLayout {
  if (!findLeaf(layout.root, leafId)) {
    return layout;
  }
  const root = activateSurfaceInLeafNode(layout.root, leafId, kind, createId);
  return {
    root,
    focusedLeafId: leafId,
    legacyMode: deriveLegacyMode(root),
  };
}

export function showWorkspacePaneSurface(
  layout: WorkspacePaneLayout,
  kind: WorkspacePaneSurfaceKind,
  createId: CreateWorkspacePaneId,
  leafId?: string | null,
): WorkspacePaneLayout {
  const targetLeafId = leafId && findLeaf(layout.root, leafId) ? leafId : layout.focusedLeafId;
  const targetLeaf = findLeaf(layout.root, targetLeafId);
  if (!targetLeaf) {
    return layout;
  }
  const { root } = addSurfaceToLeaf(layout.root, targetLeafId, kind, createId);
  return {
    root,
    focusedLeafId: targetLeafId,
    legacyMode: deriveLegacyMode(root),
  };
}

export function splitWorkspacePaneLeaf(
  layout: WorkspacePaneLayout,
  leafId: string,
  direction: WorkspacePaneSplitDirection,
  createId: CreateWorkspacePaneId,
  kind?: WorkspacePaneSurfaceKind | null,
  position: "before" | "after" = "after",
): WorkspacePaneLayout {
  const targetLeaf = findLeaf(layout.root, leafId);
  if (!targetLeaf) {
    return layout;
  }
  const surfaceKind = kind ?? chooseSplitSurface(layout, leafId);
  if (getWorkspacePaneActiveTab(targetLeaf)?.kind === surfaceKind) {
    return layout;
  }
  const rootWithoutExisting =
    pruneEmptyLeaves(removeSurfaceKind(layout.root, surfaceKind)) ?? layout.root;
  const targetAfterRemoval = findLeaf(rootWithoutExisting, leafId);
  if (!targetAfterRemoval) {
    return layout;
  }
  const newLeaf = makeLeaf(createId, surfaceKind);
  const children: [WorkspacePaneNode, WorkspacePaneNode] =
    position === "before" ? [newLeaf, targetAfterRemoval] : [targetAfterRemoval, newLeaf];
  const split = makeSplit(direction, children, createId, 0.5);
  const root = replaceLeaf(rootWithoutExisting, leafId, split);
  return {
    root,
    focusedLeafId: newLeaf.id,
    legacyMode: deriveLegacyMode(root),
  };
}

export function closeWorkspacePaneLeaf(
  layout: WorkspacePaneLayout,
  leafId: string,
  createId: CreateWorkspacePaneId,
): WorkspacePaneLayout {
  const root = removeLeaf(layout.root, leafId);
  if (!root) {
    return defaultLayoutForLegacyMode("chat", createId);
  }
  const leaves = collectWorkspacePaneLeaves(root);
  const focusedLeafId = leaves.some((leaf) => leaf.id === layout.focusedLeafId)
    ? layout.focusedLeafId
    : leaves[0]?.id ?? "";
  return {
    root,
    focusedLeafId,
    legacyMode: deriveLegacyMode(root),
  };
}

export function closeWorkspacePaneTab(
  layout: WorkspacePaneLayout,
  leafId: string,
  tabId: string,
): WorkspacePaneLayout {
  let changed = false;
  function update(node: WorkspacePaneNode): WorkspacePaneNode {
    if (node.type === "leaf") {
      if (node.id !== leafId || !node.tabs.some((tab) => tab.id === tabId)) {
        return node;
      }
      changed = true;
      const tabs = node.tabs.filter((tab) => tab.id !== tabId);
      return {
        ...node,
        tabs,
        activeTabId: tabs.some((tab) => tab.id === node.activeTabId)
          ? node.activeTabId
          : tabs[0]?.id ?? null,
      };
    }
    return { ...node, children: [update(node.children[0]), update(node.children[1])] };
  }
  const root = update(layout.root);
  return changed ? { root, focusedLeafId: leafId, legacyMode: deriveLegacyMode(root) } : layout;
}

export function updateWorkspacePaneRatio(
  layout: WorkspacePaneLayout,
  containerId: string,
  ratio: number,
): WorkspacePaneLayout {
  return {
    ...layout,
    root: updateRatioInTree(layout.root, containerId, sanitizeRatio(ratio)),
  };
}

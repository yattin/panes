import type { SplitNode } from "../../../types";

const MIN_SPLIT_RATIO = 0.1;
const MAX_SPLIT_RATIO = 0.9;

export function collectSessionIds(node: SplitNode): string[] {
  if (node.type === "leaf") {
    return [node.sessionId];
  }
  return [...collectSessionIds(node.children[0]), ...collectSessionIds(node.children[1])];
}

export type CreateTerminalSplitId = () => string;

export function buildGridSplitTree(
  sessionIds: string[],
  createSplitId: CreateTerminalSplitId,
): SplitNode {
  if (sessionIds.length === 0) {
    throw new Error("buildGridSplitTree requires at least 1 session");
  }
  if (sessionIds.length === 1) {
    return { type: "leaf", sessionId: sessionIds[0] };
  }
  if (sessionIds.length <= 3) {
    return buildVerticalColumn(sessionIds, createSplitId);
  }

  const topCount = Math.ceil(sessionIds.length / 2);
  const top = sessionIds.slice(0, topCount);
  const bottom = sessionIds.slice(topCount);
  return {
    type: "split",
    id: createSplitId(),
    direction: "horizontal",
    ratio: top.length / sessionIds.length,
    children: [
      buildVerticalColumn(top, createSplitId),
      buildVerticalColumn(bottom, createSplitId),
    ],
  };
}

function buildVerticalColumn(
  sessionIds: string[],
  createSplitId: CreateTerminalSplitId,
): SplitNode {
  if (sessionIds.length === 1) {
    return { type: "leaf", sessionId: sessionIds[0] };
  }
  return {
    type: "split",
    id: createSplitId(),
    direction: "vertical",
    ratio: 1 / sessionIds.length,
    children: [
      { type: "leaf", sessionId: sessionIds[0] },
      buildVerticalColumn(sessionIds.slice(1), createSplitId),
    ],
  };
}

export function replaceLeafInTree(
  node: SplitNode,
  targetId: string,
  replacement: SplitNode,
): SplitNode {
  if (node.type === "leaf") {
    return node.sessionId === targetId ? replacement : node;
  }
  return {
    ...node,
    children: [
      replaceLeafInTree(node.children[0], targetId, replacement),
      replaceLeafInTree(node.children[1], targetId, replacement),
    ],
  };
}

export function removeLeafFromTree(node: SplitNode, targetId: string): SplitNode | null {
  if (node.type === "leaf") {
    return node.sessionId === targetId ? null : node;
  }
  const [left, right] = node.children;
  if (left.type === "leaf" && left.sessionId === targetId) {
    return right;
  }
  if (right.type === "leaf" && right.sessionId === targetId) {
    return left;
  }
  const newLeft = removeLeafFromTree(left, targetId);
  const newRight = removeLeafFromTree(right, targetId);
  if (newLeft === null) {
    return newRight;
  }
  if (newRight === null) {
    return newLeft;
  }
  return { ...node, children: [newLeft, newRight] };
}

function sanitizeSplitRatio(ratio: number, fallback: number): number {
  const candidate = Number.isFinite(ratio) ? ratio : fallback;
  return Math.max(MIN_SPLIT_RATIO, Math.min(MAX_SPLIT_RATIO, candidate));
}

export function updateRatioInTree(
  node: SplitNode,
  containerId: string,
  ratio: number,
): SplitNode {
  if (node.type === "leaf") {
    return node;
  }
  if (node.id === containerId) {
    return { ...node, ratio: sanitizeSplitRatio(ratio, node.ratio) };
  }
  return {
    ...node,
    children: [
      updateRatioInTree(node.children[0], containerId, ratio),
      updateRatioInTree(node.children[1], containerId, ratio),
    ],
  };
}

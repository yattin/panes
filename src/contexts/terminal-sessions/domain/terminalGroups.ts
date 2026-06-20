import type { TerminalGroup } from "../../../types";

export function nextTerminalNumber(groups: TerminalGroup[]): number {
  const used = new Set<number>();
  for (const group of groups) {
    const match = /^Terminal (\d+)$/.exec(group.name);
    if (match) {
      used.add(Number(match[1]));
    }
  }
  let next = 1;
  while (used.has(next)) {
    next += 1;
  }
  return next;
}

export function reorderTerminalGroups(
  groups: TerminalGroup[],
  fromIndex: number,
  toIndex: number,
): TerminalGroup[] {
  if (
    fromIndex === toIndex ||
    fromIndex < 0 ||
    toIndex < 0 ||
    fromIndex >= groups.length ||
    toIndex >= groups.length
  ) {
    return groups;
  }

  const nextGroups = [...groups];
  const [moved] = nextGroups.splice(fromIndex, 1);
  nextGroups.splice(toIndex, 0, moved);
  return nextGroups;
}

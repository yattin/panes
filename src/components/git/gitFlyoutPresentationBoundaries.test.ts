import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const GIT_FLYOUT_PRESENTATION_FILES = [
  "src/components/git/GitBranchesView.tsx",
  "src/components/git/GitPanel.tsx",
  "src/components/git/GitWorktreesView.tsx",
  "src/components/git/MultiRepoChangesView.tsx",
  "src/components/shared/Dropdown.tsx",
];

describe("git flyout presentation boundaries", () => {
  it("does not import flyout helpers from infrastructure", () => {
    for (const file of GIT_FLYOUT_PRESENTATION_FILES) {
      const source = readFileSync(resolve(process.cwd(), file), "utf8");
      expect(source, file).not.toContain("contexts/git/infrastructure/gitFlyoutRegion");
    }
  });
});

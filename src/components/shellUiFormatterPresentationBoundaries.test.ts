import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const COMPONENT_SOURCE_FILES = [
  "src/components/sidebar/Sidebar.tsx",
  "src/components/workspace/WorkspaceSettingsModal.tsx",
  "src/components/workspace/WorkspaceSettingsPage.tsx",
  "src/components/git/GitBranchesView.tsx",
  "src/components/git/GitCommitsView.tsx",
  "src/components/git/GitStashView.tsx",
  "src/components/shared/CommandPalette.tsx",
];

describe("shell UI formatter presentation boundaries", () => {
  it("does not import formatter helpers from infrastructure", () => {
    for (const file of COMPONENT_SOURCE_FILES) {
      const source = readFileSync(resolve(process.cwd(), file), "utf8");
      expect(source, file).not.toContain("contexts/shell-ui/infrastructure/formatters");
    }
  });
});

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const WORKSPACE_SETTINGS_PRESENTATION_FILES = [
  "src/components/workspace/WorkspaceSettingsModal.tsx",
  "src/components/workspace/WorkspaceSettingsPage.tsx",
];

describe("workspace settings presentation boundaries", () => {
  it("does not import workspace infrastructure directly", () => {
    for (const file of WORKSPACE_SETTINGS_PRESENTATION_FILES) {
      const source = readFileSync(resolve(process.cwd(), file), "utf8");
      expect(source, file).not.toContain("contexts/workspaces/infrastructure");
    }
  });
});

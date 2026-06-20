import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const WORKSPACE_STARTUP_PRESENTATION_FILES = [
  "src/components/workspace/WorkspaceStartupSection.tsx",
  "src/components/workspace/WorkspaceStartupPresetModal.tsx",
];

describe("workspace startup presentation boundaries", () => {
  it("does not import workspace or terminal infrastructure directly", () => {
    for (const file of WORKSPACE_STARTUP_PRESENTATION_FILES) {
      const source = readFileSync(resolve(process.cwd(), file), "utf8");
      expect(source, file).not.toContain("contexts/workspaces/infrastructure");
      expect(source, file).not.toContain("contexts/terminal-sessions/infrastructure");
    }
  });

  it("does not import Tauri file system or dialog APIs directly", () => {
    for (const file of WORKSPACE_STARTUP_PRESENTATION_FILES) {
      const source = readFileSync(resolve(process.cwd(), file), "utf8");
      expect(source, file).not.toContain("@tauri-apps/plugin-dialog");
      expect(source, file).not.toContain("@tauri-apps/plugin-fs");
    }
  });
});

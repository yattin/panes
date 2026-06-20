import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const WINDOW_ACTIONS_PRESENTATION_FILES = [
  "src/components/layout/ThreeColumnLayout.tsx",
  "src/components/editor/FileExplorer.tsx",
  "src/components/editor/FileEditorPanel.tsx",
  "src/components/shared/CustomWindowFrame.tsx",
  "src/components/workspace/WorkspacePaneShell.tsx",
  "src/components/terminal/TerminalPanel.tsx",
  "src/components/chat/ChatPanel.tsx",
];

describe("shell ui window actions presentation boundaries", () => {
  it("does not import window actions from infrastructure", () => {
    for (const file of WINDOW_ACTIONS_PRESENTATION_FILES) {
      const source = readFileSync(resolve(process.cwd(), file), "utf8");
      expect(source, file).not.toContain("contexts/shell-ui/infrastructure/windowActions");
    }
  });
});

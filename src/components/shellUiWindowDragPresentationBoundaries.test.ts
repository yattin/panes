import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const WINDOW_DRAG_PRESENTATION_FILES = [
  "src/components/git/GitPanel.tsx",
  "src/components/workspace/WorkspaceSettingsPage.tsx",
  "src/components/workspace/WorkspacePaneShell.tsx",
  "src/components/chat/ChatPanel.tsx",
  "src/components/terminal/TerminalPanel.tsx",
  "src/components/sidebar/Sidebar.tsx",
  "src/components/onboarding/HarnessPanel.tsx",
  "src/components/layout/ThreeColumnLayout.tsx",
  "src/components/shared/CustomWindowFrame.tsx",
  "src/components/shared/CustomWindowResizeHandles.tsx",
];
const WINDOW_RESIZE_PRESENTATION_FILES = [
  "src/components/shared/CustomWindowResizeHandles.tsx",
];

describe("shell ui window drag presentation boundaries", () => {
  it("does not import window drag helpers from infrastructure", () => {
    for (const file of WINDOW_DRAG_PRESENTATION_FILES) {
      const source = readFileSync(resolve(process.cwd(), file), "utf8");
      expect(source, file).not.toContain("contexts/shell-ui/infrastructure/windowDrag");
    }
  });

  it("does not call Tauri window APIs directly from presentation components", () => {
    for (const file of WINDOW_RESIZE_PRESENTATION_FILES) {
      const source = readFileSync(resolve(process.cwd(), file), "utf8");
      expect(source, file).not.toContain("@tauri-apps/api/window");
    }
  });
});

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const TERMINAL_RENDERING_SETTINGS_PRESENTATION_FILES = [
  "src/components/terminal/TerminalPanel.tsx",
  "src/components/sidebar/Sidebar.tsx",
];

describe("terminal rendering settings presentation boundaries", () => {
  it("does not import terminal rendering settings from infrastructure", () => {
    for (const file of TERMINAL_RENDERING_SETTINGS_PRESENTATION_FILES) {
      const source = readFileSync(resolve(process.cwd(), file), "utf8");
      expect(source, file).not.toContain(
        "contexts/terminal-sessions/infrastructure/terminalRenderingSettings",
      );
    }
  });
});

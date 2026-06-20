import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const TERMINAL_SESSION_PRESENTATION_FILES = [
  "src/components/terminal/TerminalPanel.tsx",
  "src/components/onboarding/HarnessPanel.tsx",
];

describe("terminal session presentation boundaries", () => {
  it("does not import terminal session infrastructure directly", () => {
    for (const file of TERMINAL_SESSION_PRESENTATION_FILES) {
      const source = readFileSync(resolve(process.cwd(), file), "utf8");
      expect(source, file).not.toContain("contexts/terminal-sessions/infrastructure");
    }
  });
});

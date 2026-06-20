import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const CLIPBOARD_PRESENTATION_FILES = [
  "src/components/terminal/TerminalPanel.tsx",
  "src/components/onboarding/HarnessPanel.tsx",
  "src/components/onboarding/OnboardingWizard.tsx",
];

describe("shell ui clipboard presentation boundaries", () => {
  it("does not import clipboard helpers from infrastructure", () => {
    for (const file of CLIPBOARD_PRESENTATION_FILES) {
      const source = readFileSync(resolve(process.cwd(), file), "utf8");
      expect(source, file).not.toContain("contexts/shell-ui/infrastructure/clipboard");
    }
  });
});

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const NATIVE_EDIT_ACTION_PRESENTATION_FILES = [
  "src/App.tsx",
  "src/components/shared/CustomWindowFrame.tsx",
];

describe("shell ui native edit actions presentation boundaries", () => {
  it("does not import native edit actions from infrastructure", () => {
    for (const file of NATIVE_EDIT_ACTION_PRESENTATION_FILES) {
      const source = readFileSync(resolve(process.cwd(), file), "utf8");
      expect(source, file).not.toContain("contexts/shell-ui/infrastructure/nativeEditActions");
    }
  });
});

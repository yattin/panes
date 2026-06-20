import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const GIT_PRESENTATION_FILES = [
  "src/components/git/GitPanel.tsx",
  "src/components/git/MultiRepoChangesView.tsx",
];

describe("git presentation boundaries", () => {
  it("does not import git infrastructure directly", () => {
    for (const file of GIT_PRESENTATION_FILES) {
      const source = readFileSync(resolve(process.cwd(), file), "utf8");
      expect(source, file).not.toContain("contexts/git/infrastructure");
    }
  });
});

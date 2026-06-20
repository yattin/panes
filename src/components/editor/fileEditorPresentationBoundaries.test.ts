import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const FILE_EDITOR_PRESENTATION_FILES = [
  "src/components/editor/FileEditorPanel.tsx",
  "src/components/editor/FileExplorer.tsx",
  "src/components/git/GitFilesView.tsx",
];

describe("file editor presentation boundaries", () => {
  it("does not import file editor infrastructure directly", () => {
    for (const file of FILE_EDITOR_PRESENTATION_FILES) {
      const source = readFileSync(resolve(process.cwd(), file), "utf8");
      expect(source, file).not.toContain("contexts/file-editor/infrastructure");
    }
  });
});

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("CommandPalette presentation boundaries", () => {
  it("does not import git, file editor, or terminal infrastructure directly", () => {
    const source = readFileSync(resolve(__dirname, "CommandPalette.tsx"), "utf8");

    expect(source).not.toContain("contexts/git/infrastructure");
    expect(source).not.toContain("contexts/file-editor/infrastructure");
    expect(source).not.toContain("contexts/terminal-sessions/infrastructure");
    expect(source).not.toContain("contexts/chat/infrastructure");
  });
});

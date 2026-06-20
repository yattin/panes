import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("FileExplorer presentation boundaries", () => {
  it("does not import git infrastructure directly", () => {
    const file = resolve(__dirname, "FileExplorer.tsx");
    const source = readFileSync(file, "utf8");

    expect(source).not.toContain("contexts/git/infrastructure");
  });

  it("does not import chat infrastructure directly", () => {
    const file = resolve(__dirname, "FileExplorer.tsx");
    const source = readFileSync(file, "utf8");

    expect(source).not.toContain("contexts/chat/infrastructure");
  });
});

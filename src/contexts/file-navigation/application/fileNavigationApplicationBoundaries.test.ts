import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("file-navigation application boundaries", () => {
  it("does not import Tauri shell APIs directly for link navigation", () => {
    const source = readFileSync(
      resolve(process.cwd(), "src/contexts/file-navigation/application/fileLinkNavigation.ts"),
      "utf8",
    );

    expect(source).not.toContain("@tauri-apps/plugin-shell");
  });

  it("does not import IPC directly for editor file reference resolution", () => {
    const source = readFileSync(
      resolve(process.cwd(), "src/contexts/file-navigation/application/openEditorFileReference.ts"),
      "utf8",
    );

    expect(source).not.toContain("../../../lib/ipc");
  });
});

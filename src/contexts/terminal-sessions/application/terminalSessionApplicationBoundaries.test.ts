import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("terminal-sessions application boundaries", () => {
  it("does not import IPC directly for rendering settings", () => {
    const source = readFileSync(
      resolve(process.cwd(), "src/contexts/terminal-sessions/application/terminalRenderingSettings.ts"),
      "utf8",
    );

    expect(source).not.toContain("../../../lib/ipc");
  });
});

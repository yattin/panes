import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("shell ui app locale presentation boundaries", () => {
  it("does not import app locale persistence from infrastructure", () => {
    const source = readFileSync(
      resolve(process.cwd(), "src/components/sidebar/Sidebar.tsx"),
      "utf8",
    );

    expect(source).not.toContain("contexts/shell-ui/infrastructure/appLocaleRepository");
  });

  it("does not import Tauri dialog APIs directly", () => {
    const source = readFileSync(
      resolve(process.cwd(), "src/components/sidebar/Sidebar.tsx"),
      "utf8",
    );

    expect(source).not.toContain("@tauri-apps/plugin-dialog");
  });
});

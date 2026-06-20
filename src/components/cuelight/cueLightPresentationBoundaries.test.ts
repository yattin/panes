import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const CUELIGHT_PRESENTATION_FILES = [
  "src/components/cuelight/CueLightTokenGate.tsx",
  "src/components/cuelight/CueLightProjectPicker.tsx",
  "src/components/cuelight/CreateWorkspaceDialog.tsx",
];

describe("CueLight presentation boundaries", () => {
  it("does not import CueLight infrastructure directly", () => {
    for (const file of CUELIGHT_PRESENTATION_FILES) {
      const source = readFileSync(resolve(process.cwd(), file), "utf8");
      expect(source, file).not.toContain("contexts/cue-light/infrastructure");
    }
  });

  it("does not import Tauri dialog APIs directly", () => {
    const source = readFileSync(
      resolve(process.cwd(), "src/components/cuelight/CreateWorkspaceDialog.tsx"),
      "utf8",
    );

    expect(source).not.toContain("@tauri-apps/plugin-dialog");
  });
});

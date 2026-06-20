import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const SHELL_UI_APPLICATION_FILES = [
  "src/contexts/shell-ui/application/appInfo.ts",
  "src/contexts/shell-ui/application/appLocaleRepository.ts",
  "src/contexts/shell-ui/application/externalLinks.ts",
  "src/contexts/shell-ui/application/fileDialogs.ts",
  "src/contexts/shell-ui/application/windowActions.ts",
  "src/contexts/shell-ui/application/windowDrag.ts",
  "src/contexts/shell-ui/application/windowFileDrops.ts",
];

describe("shell-ui application boundaries", () => {
  it("does not import Tauri APIs directly for native shell capabilities", () => {
    for (const file of SHELL_UI_APPLICATION_FILES) {
      const source = readFileSync(resolve(process.cwd(), file), "utf8");

      expect(source, file).not.toContain("@tauri-apps/");
      expect(source, file).not.toContain("../../../lib/ipc");
    }
  });
});

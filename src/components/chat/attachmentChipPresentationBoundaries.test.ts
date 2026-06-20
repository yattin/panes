import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("chat presentation boundaries", () => {
  it("does not import chat infrastructure directly from small chat components", () => {
    const files = [
      resolve(__dirname, "AttachmentChip.tsx"),
      resolve(__dirname, "ChatCommandPanel.tsx"),
      resolve(__dirname, "CodexThreadPicker.tsx"),
      resolve(__dirname, "MarkdownContent.tsx"),
    ];

    for (const file of files) {
      const source = readFileSync(file, "utf8");
      expect(source, file).not.toContain("contexts/chat/infrastructure");
    }
  });
});

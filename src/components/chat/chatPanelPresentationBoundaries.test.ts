import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

describe("ChatPanel presentation boundaries", () => {
  it("uses the chat application gateway instead of chat infrastructure adapters", () => {
    const source = readFileSync(resolve(__dirname, "ChatPanel.tsx"), "utf8");

    expect(source).not.toContain("contexts/chat/infrastructure");
  });

  it("does not subscribe to Tauri window events directly", () => {
    const source = readFileSync(resolve(__dirname, "ChatPanel.tsx"), "utf8");

    expect(source).not.toContain("@tauri-apps/api/window");
  });

  it("does not import Tauri dialog APIs directly", () => {
    const source = readFileSync(resolve(__dirname, "ChatPanel.tsx"), "utf8");

    expect(source).not.toContain("@tauri-apps/plugin-dialog");
  });
});

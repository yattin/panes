import { describe, expect, it } from "vitest";
import { isClaudeFamilyEngine } from "./chatEngineIds";

describe("isClaudeFamilyEngine", () => {
  it("matches Claude engine ids only", () => {
    expect(isClaudeFamilyEngine("claude")).toBe(true);
    expect(isClaudeFamilyEngine("claude-code-native")).toBe(true);
    expect(isClaudeFamilyEngine("claurst-native")).toBe(true);
    expect(isClaudeFamilyEngine("codex")).toBe(false);
    expect(isClaudeFamilyEngine(null)).toBe(false);
  });
});

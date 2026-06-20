import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

describe("App composition boundaries", () => {
  it("uses the configured chat gateway instead of direct chat repository calls", () => {
    const source = readFileSync(resolve(__dirname, "App.tsx"), "utf8");

    expect(source).not.toContain("chatRepository");
  });
});

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("inline file reference presentation boundaries", () => {
  it("does not import editor file reference navigation from infrastructure", () => {
    const source = readFileSync(
      resolve(process.cwd(), "src/components/shared/InlineFileReferenceText.tsx"),
      "utf8",
    );

    expect(source).not.toContain("contexts/file-navigation/infrastructure");
  });
});

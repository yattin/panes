import { describe, expect, it } from "vitest";
import { applyAppTheme, normalizeAppTheme } from "./appTheme";

describe("appTheme", () => {
  it("normalizes supported theme values", () => {
    expect(normalizeAppTheme("dark")).toBe("dark");
    expect(normalizeAppTheme(" Light ")).toBe("light");
    expect(normalizeAppTheme("system")).toBeNull();
    expect(normalizeAppTheme(null)).toBeNull();
  });

  it("applies the theme to the document element", () => {
    let appliedTheme: string | null = null;
    const doc = {
      documentElement: {
        setAttribute(name: string, value: string) {
          if (name === "data-theme") {
            appliedTheme = value;
          }
        },
      },
    } as unknown as Document;

    applyAppTheme("light", doc);

    expect(appliedTheme).toBe("light");
  });
});

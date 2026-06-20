import { describe, expect, it } from "vitest";
import { getActionBlockDisplayText } from "./MessageBlocks";

describe("getActionBlockDisplayText", () => {
  it("uses CueLight display label and subtitle when present", () => {
    expect(
      getActionBlockDisplayText({
        summary: "cuelight_create_character",
        displayLabel: " 创建角色 ",
        displaySubtitle: " 田雨 ",
      }),
    ).toEqual({
      label: "创建角色",
      subtitle: "田雨",
    });
  });

  it("falls back to summary for older action blocks", () => {
    expect(
      getActionBlockDisplayText({
        summary: "cuelight_create_character 田雨",
      }),
    ).toEqual({
      label: "cuelight_create_character 田雨",
      subtitle: null,
    });
  });

  it("ignores blank display fields", () => {
    expect(
      getActionBlockDisplayText({
        summary: "search path",
        displayLabel: "   ",
        displaySubtitle: "\n\t",
      }),
    ).toEqual({
      label: "search path",
      subtitle: null,
    });
  });
});

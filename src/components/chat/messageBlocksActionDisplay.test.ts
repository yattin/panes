import { describe, expect, it } from "vitest";
import { getActionBlockDisplayText } from "./MessageBlocks";

describe("getActionBlockDisplayText", () => {
  it("uses CueLight display label and subtitle when present", () => {
    expect(
      getActionBlockDisplayText({
        summary: "save_drama_character",
        displayLabel: " 创建角色 ",
        displaySubtitle: " 田雨 ",
      }),
    ).toEqual({
      label: "创建角色",
      subtitle: "田雨",
    });
  });

  it("maps CueLight action blocks to Chinese labels", () => {
    expect(
      getActionBlockDisplayText({
        summary: "query_visual_bible",
      }),
    ).toEqual({
      label: "读取视觉设计",
      subtitle: null,
    });

    expect(
      getActionBlockDisplayText({
        summary: "update_visual_bible",
      }),
    ).toEqual({
      label: "更新视觉设计",
      subtitle: null,
    });
  });

  it("falls back to summary for non-CueLight action blocks and ignores blank display fields", () => {
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

import { describe, expect, it } from "vitest";
import {
  compileCueLightCommandDisplayPrompt,
  compileCueLightCommandPrompt,
  getCueLightToolLabel,
  nativeCueLightCommandId,
  nativeCueLightSlashCommands,
} from "./nativeCueLightSlashCommands";

describe("nativeCueLightSlashCommands", () => {
  it("exposes Chinese CueLight workflow labels and hides low-level utility commands", () => {
    const labels = nativeCueLightSlashCommands.map((command) => command.label);

    expect(labels).toContain("项目状态");
    expect(labels).toContain("故事设计");
    expect(labels).toContain("视觉设计");
    expect(labels).toContain("分镜规划");
    expect(labels).not.toContain("模型列表");
    expect(labels).not.toContain("下载原文");
  });

  it("binds visible commands to CueLight tools and templates mention the tools", () => {
    const storyDesign = nativeCueLightSlashCommands.find(
      (command) => command.label === "故事设计",
    );
    const visualDesign = nativeCueLightSlashCommands.find(
      (command) => command.label === "视觉设计",
    );
    const storyboards = nativeCueLightSlashCommands.find(
      (command) => command.label === "分镜规划",
    );

    expect(storyDesign?.tools).toEqual([
      "cuelight_get_story_bible",
      "cuelight_update_story_bible",
    ]);
    expect(storyDesign?.template).toContain("cuelight_get_story_bible");
    expect(visualDesign?.tools).toEqual([
      "cuelight_get_visual_bible",
      "cuelight_update_visual_bible",
    ]);
    expect(visualDesign?.template).toContain("cuelight_update_visual_bible");
    expect(storyboards?.tools).toContain("cuelight_batch_update_storyboards");
    expect(storyboards?.template).toContain("cuelight_list_storyboards");
  });

  it("templates require direct tool use instead of stopping after a plan", () => {
    for (const command of nativeCueLightSlashCommands) {
      expect(command.template).toContain("本轮必须优先调用");
      expect(command.template).toContain("请直接调用上述 CueLight 工具");
      expect(command.template).not.toContain("请先说明你将调用哪些 CueLight 工具，再执行");
    }
  });

  it("uses stable native command ids for ChatPanel selection", () => {
    expect(nativeCueLightCommandId("storyboards")).toBe("native-cuelight:storyboards");
  });

  it("compiles a selected command with user-provided text only when text exists", () => {
    const command = nativeCueLightSlashCommands.find(
      (candidate) => candidate.label === "项目状态",
    );

    expect(command).toBeTruthy();
    expect(compileCueLightCommandPrompt(command!, "")).toBe(command!.template);
    expect(compileCueLightCommandPrompt(command!, "重点看分镜进度")).toContain(
      "用户补充：\n重点看分镜进度",
    );
    expect(compileCueLightCommandPrompt(command!, "重点看分镜进度")).toContain(
      "cuelight_project_status",
    );
  });

  it("compiles a user-visible display prompt without CueLight tool names", () => {
    const command = nativeCueLightSlashCommands.find(
      (candidate) => candidate.label === "视觉设计",
    );

    expect(command).toBeTruthy();
    const displayPrompt = compileCueLightCommandDisplayPrompt(command!, "明确为3d动画");

    expect(displayPrompt).toBe("完善 CueLight 视觉设计\n\n用户补充：\n明确为3d动画");
    expect(displayPrompt).not.toContain("cuelight_");
  });

  it("maps CueLight tool names to Chinese action labels", () => {
    expect(getCueLightToolLabel("cuelight_get_visual_bible")).toBe("读取视觉设计");
    expect(getCueLightToolLabel("cuelight_update_visual_bible")).toBe("更新视觉设计");
    expect(getCueLightToolLabel("cuelight_download_original_script")).toBe("下载剧本原文");
    expect(getCueLightToolLabel("search")).toBeNull();
  });
});

import {
  Clapperboard,
  Film,
  Image,
  Layers,
  ListChecks,
  Palette,
  ScrollText,
  Sparkles,
  Theater,
  Video,
  Wand2,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";

export interface NativeCueLightSlashCommand {
  id: string;
  label: string;
  description: string;
  icon: LucideIcon;
  tools: string[];
  template: string;
}

const cueLightToolLabels: Record<string, string> = {
  cuelight_project_status: "查看项目状态",
  cuelight_get_story_bible: "读取故事设计",
  cuelight_update_story_bible: "更新故事设计",
  cuelight_get_visual_bible: "读取视觉设计",
  cuelight_update_visual_bible: "更新视觉设计",
  cuelight_list_characters: "列出角色资产",
  cuelight_get_character: "读取角色资产",
  cuelight_create_character: "创建角色资产",
  cuelight_update_character: "更新角色资产",
  cuelight_delete_character: "删除角色资产",
  cuelight_list_scenes: "列出场景资产",
  cuelight_get_scene: "读取场景资产",
  cuelight_create_scene: "创建场景资产",
  cuelight_update_scene: "更新场景资产",
  cuelight_delete_scene: "删除场景资产",
  cuelight_list_props: "列出道具资产",
  cuelight_get_prop: "读取道具资产",
  cuelight_create_prop: "创建道具资产",
  cuelight_update_prop: "更新道具资产",
  cuelight_delete_prop: "删除道具资产",
  cuelight_list_episodes: "列出分集剧本",
  cuelight_get_episode: "读取分集剧本",
  cuelight_create_episode: "创建分集剧本",
  cuelight_update_episode: "更新分集剧本",
  cuelight_delete_episode: "删除分集剧本",
  cuelight_list_storyboards: "列出分镜规划",
  cuelight_get_storyboard: "读取分镜规划",
  cuelight_create_storyboard: "创建分镜规划",
  cuelight_update_storyboard: "更新分镜规划",
  cuelight_delete_storyboard: "删除分镜规划",
  cuelight_batch_update_storyboards: "批量更新分镜规划",
  cuelight_upload_file: "上传参考文件",
  cuelight_generate_image: "生成图片",
  cuelight_generate_video: "生成视频",
  cuelight_task_status: "查询任务状态",
  cuelight_list_models: "查看生成模型",
  cuelight_download_original_script: "下载剧本原文",
};

export function getCueLightToolLabel(toolName: string): string | null {
  return cueLightToolLabels[toolName] ?? null;
}

function toolLine(tools: string[]): string {
  return tools.map((tool) => `\`${tool}\``).join(" / ");
}

function template(title: string, tools: string[], request: string): string {
  return [
    `${title}`,
    "",
    `本轮必须优先调用 ${toolLine(tools)} 获取或更新 CueLight 项目数据。`,
    request,
    "",
    "不要只说明计划后停止；请直接调用上述 CueLight 工具，拿到结果后再用中文汇总。",
  ].join("\n");
}

export const nativeCueLightSlashCommands: NativeCueLightSlashCommand[] = [
  {
    id: "project-status",
    label: "项目状态",
    description: "查看当前 CueLight 项目的整体进度与可用数据",
    icon: ListChecks,
    tools: ["cuelight_project_status"],
    template: template(
      "查看 CueLight 项目状态",
      ["cuelight_project_status"],
      "请基于项目状态指出当前最适合推进的下一步。",
    ),
  },
  {
    id: "story-design",
    label: "故事设计",
    description: "读取或更新世界观、故事设定与核心叙事方向",
    icon: ScrollText,
    tools: ["cuelight_get_story_bible", "cuelight_update_story_bible"],
    template: template(
      "完善 CueLight 故事设计",
      ["cuelight_get_story_bible", "cuelight_update_story_bible"],
      "请先读取当前故事设计，再根据我的补充给出可写入的更新建议；需要写入时再调用更新工具。",
    ),
  },
  {
    id: "visual-design",
    label: "视觉设计",
    description: "读取或更新项目视觉风格、画幅与生成风格设定",
    icon: Palette,
    tools: ["cuelight_get_visual_bible", "cuelight_update_visual_bible"],
    template: template(
      "完善 CueLight 视觉设计",
      ["cuelight_get_visual_bible", "cuelight_update_visual_bible"],
      "请先读取当前视觉设计，再整理适合影视生成的视觉方向；需要写入时再调用更新工具。",
    ),
  },
  {
    id: "characters",
    label: "角色资产",
    description: "梳理、创建或更新角色资产与角色提示词",
    icon: Theater,
    tools: ["cuelight_list_characters", "cuelight_create_character", "cuelight_update_character"],
    template: template(
      "整理 CueLight 角色资产",
      ["cuelight_list_characters", "cuelight_create_character", "cuelight_update_character"],
      "请先列出现有角色，再根据剧情需要补全角色描述、basePrompt 与参考图需求。",
    ),
  },
  {
    id: "scenes",
    label: "场景资产",
    description: "梳理、创建或更新场景资产与场景提示词",
    icon: Clapperboard,
    tools: ["cuelight_list_scenes", "cuelight_create_scene", "cuelight_update_scene"],
    template: template(
      "整理 CueLight 场景资产",
      ["cuelight_list_scenes", "cuelight_create_scene", "cuelight_update_scene"],
      "请先列出现有场景，再根据故事推进补全场景描述、basePrompt 与参考图需求。",
    ),
  },
  {
    id: "props",
    label: "道具资产",
    description: "梳理、创建或更新关键道具资产",
    icon: Sparkles,
    tools: ["cuelight_list_props", "cuelight_create_prop", "cuelight_update_prop"],
    template: template(
      "整理 CueLight 道具资产",
      ["cuelight_list_props", "cuelight_create_prop", "cuelight_update_prop"],
      "请先列出现有道具，再补全关键道具的功能、外观描述与镜头使用建议。",
    ),
  },
  {
    id: "episodes",
    label: "分集剧本",
    description: "读取、创建或更新分集结构与剧本正文",
    icon: Layers,
    tools: ["cuelight_list_episodes", "cuelight_get_episode", "cuelight_create_episode", "cuelight_update_episode"],
    template: template(
      "推进 CueLight 分集剧本",
      ["cuelight_list_episodes", "cuelight_get_episode", "cuelight_create_episode", "cuelight_update_episode"],
      "请先查看分集列表，再根据我提供的目标拆解或更新分集剧情。",
    ),
  },
  {
    id: "storyboards",
    label: "分镜规划",
    description: "读取、创建或批量更新分镜与视频提示词",
    icon: Film,
    tools: [
      "cuelight_list_storyboards",
      "cuelight_create_storyboard",
      "cuelight_update_storyboard",
      "cuelight_batch_update_storyboards",
    ],
    template: template(
      "规划 CueLight 分镜",
      [
        "cuelight_list_storyboards",
        "cuelight_create_storyboard",
        "cuelight_update_storyboard",
        "cuelight_batch_update_storyboards",
      ],
      "请先读取目标集数的分镜列表，再生成或更新镜头设计、画面描述、角色关联和英文 videoPrompt。",
    ),
  },
  {
    id: "generate-image",
    label: "生成图片",
    description: "调用 CueLight 图片生成能力制作参考图或关键帧",
    icon: Image,
    tools: ["cuelight_generate_image"],
    template: template(
      "生成 CueLight 图片",
      ["cuelight_generate_image"],
      "请根据项目视觉设计、角色/场景/道具资产和我的要求生成图片任务，并返回 taskId 与后续检查方式。",
    ),
  },
  {
    id: "generate-video",
    label: "生成视频",
    description: "调用 CueLight 视频生成能力制作视频片段",
    icon: Video,
    tools: ["cuelight_generate_video"],
    template: template(
      "生成 CueLight 视频",
      ["cuelight_generate_video"],
      "请根据分镜、参考图和我的运动要求生成视频任务，并返回 taskId 与后续检查方式。",
    ),
  },
  {
    id: "task-status",
    label: "任务状态",
    description: "查询 CueLight 图片或视频异步生成任务状态",
    icon: Wand2,
    tools: ["cuelight_task_status"],
    template: template(
      "查询 CueLight 任务状态",
      ["cuelight_task_status"],
      "请根据我提供的 taskId 查询任务状态，并说明结果和下一步。",
    ),
  },
];

export function nativeCueLightCommandId(id: string): string {
  return `native-cuelight:${id}`;
}

export function compileCueLightCommandPrompt(
  command: NativeCueLightSlashCommand,
  inputText: string,
): string {
  const trimmed = inputText.trim();
  if (!trimmed) {
    return command.template;
  }
  return `${command.template}\n\n用户补充：\n${trimmed}`;
}

export function compileCueLightCommandDisplayPrompt(
  command: NativeCueLightSlashCommand,
  inputText: string,
): string {
  const title = command.template.split("\n", 1)[0]?.trim() || command.label;
  const trimmed = inputText.trim();
  if (!trimmed) {
    return title;
  }
  return `${title}\n\n用户补充：\n${trimmed}`;
}

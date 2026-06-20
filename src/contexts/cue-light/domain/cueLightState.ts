import type { CueLightProjectBinding } from "../../../types";

export interface CueLightProject {
  id: string;
  name: string;
  projectType?: string;
  videoAspectRatio?: string;
  episodes?: { id: string; title?: string }[];
  storyboards?: { id: string }[];
  [key: string]: unknown;
}

export interface CueLightBible {
  worldView?: string;
  stylePrompt?: string;
  [key: string]: unknown;
}

export interface CueLightEpisode {
  id: string;
  title?: string;
  number?: number;
  summary?: string;
  [key: string]: unknown;
}

export interface CueLightCharacter {
  id: string;
  name: string;
  description?: string;
  referenceImageUrl?: string;
  [key: string]: unknown;
}

export interface CueLightScene {
  id: string;
  name: string;
  description?: string;
  referenceImageUrl?: string;
  [key: string]: unknown;
}

export interface CueLightProp {
  id: string;
  name: string;
  description?: string;
  referenceImageUrl?: string;
  [key: string]: unknown;
}

export interface CueLightStoryboard {
  id: string;
  sceneNumber?: number;
  videoPrompt?: string;
  firstFrameUrl?: string;
  firstFrameThumbnailUrl?: string;
  videoClipUrl?: string;
  videoCoverUrl?: string;
  nineGridImageUrl?: string;
  referenceCharacterIds?: string[];
  sceneId?: string;
  status?: string;
  [key: string]: unknown;
}

export interface CueLightVideoAsset {
  id: string;
  url?: string;
  thumbnailUrl?: string;
  createdAt?: string;
  [key: string]: unknown;
}

export type AssetsTab = "characters" | "scenes" | "props" | "history";

export interface CueLightState {
  projectDetail: CueLightProject | null;
  bible: CueLightBible | null;
  episodes: CueLightEpisode[];
  characters: CueLightCharacter[];
  scenes: CueLightScene[];
  props: CueLightProp[];
  storyboards: Record<string, CueLightStoryboard[]>;
  videoAssets: CueLightVideoAsset[];
  selectedEpisodeId: string | null;
  assetsTab: AssetsTab;
  loading: Record<string, boolean>;
  error: string | null;
  loadOverview: (binding: CueLightProjectBinding) => Promise<void>;
  loadEpisodes: (binding: CueLightProjectBinding) => Promise<void>;
  loadStoryboards: (
    binding: CueLightProjectBinding,
    episodeId: string,
  ) => Promise<void>;
  loadCharacters: (binding: CueLightProjectBinding) => Promise<void>;
  loadScenes: (binding: CueLightProjectBinding) => Promise<void>;
  loadProps: (binding: CueLightProjectBinding) => Promise<void>;
  loadVideoAssets: (binding: CueLightProjectBinding) => Promise<void>;
  setSelectedEpisodeId: (id: string | null) => void;
  setAssetsTab: (tab: AssetsTab) => void;
  reset: () => void;
}

export const initialCueLightState = {
  projectDetail: null,
  bible: null,
  episodes: [],
  characters: [],
  scenes: [],
  props: [],
  storyboards: {},
  videoAssets: [],
  selectedEpisodeId: null,
  assetsTab: "characters" as AssetsTab,
  loading: {},
  error: null,
};

export function selectFirstEpisodeIfNeeded(
  selectedEpisodeId: string | null,
  episodes: CueLightEpisode[],
): string | null {
  return selectedEpisodeId ?? episodes[0]?.id ?? null;
}

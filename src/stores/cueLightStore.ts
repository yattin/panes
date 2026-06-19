import { create } from "zustand";
import { ipc } from "../lib/ipc";
import { CUELIGHT_SERVER_URL, getCueLightToken } from "../lib/cueLightConfig";
import type { CueLightProjectBinding } from "../types";

// ---------------------------------------------------------------------------
// Types for CueLight data
// ---------------------------------------------------------------------------

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
  referenceImageUrl?: string;
  referenceCharacterIds?: string[];
  sceneId?: string;
  videoUrl?: string;
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

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

interface CueLightState {
  // Data
  projectDetail: CueLightProject | null;
  bible: CueLightBible | null;
  episodes: CueLightEpisode[];
  characters: CueLightCharacter[];
  scenes: CueLightScene[];
  props: CueLightProp[];
  storyboards: Record<string, CueLightStoryboard[]>; // episodeId → storyboards
  videoAssets: CueLightVideoAsset[];

  // UI state
  selectedEpisodeId: string | null;
  assetsTab: AssetsTab;
  loading: Record<string, boolean>;
  error: string | null;

  // Actions
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

async function cueLightGet<T>(
  _binding: CueLightProjectBinding,
  path: string,
  query?: Record<string, string>,
): Promise<T> {
  const token = getCueLightToken();
  const result = await ipc.cueLightProxy({
    method: "GET",
    serverUrl: CUELIGHT_SERVER_URL,
    path,
    authToken: token,
    query,
  });
  return result as T;
}

const initialState = {
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

export const useCueLightStore = create<CueLightState>((set, get) => ({
  ...initialState,

  loadOverview: async (binding) => {
    set({ loading: { ...get().loading, overview: true }, error: null });
    try {
      const [project, bible, videoAssets] = await Promise.all([
        cueLightGet<CueLightProject>(binding, `/api/projects/${binding.projectId}`),
        cueLightGet<CueLightBible>(binding, `/api/projects/${binding.projectId}/bible`),
        cueLightGet<CueLightVideoAsset[]>(binding, `/api/projects/${binding.projectId}/video-assets`),
      ]);
      set({
        projectDetail: project,
        bible,
        videoAssets: videoAssets ?? [],
        loading: { ...get().loading, overview: false },
      });
    } catch (e) {
      set({ error: String(e), loading: { ...get().loading, overview: false } });
    }
  },

  loadEpisodes: async (binding) => {
    set({ loading: { ...get().loading, episodes: true }, error: null });
    try {
      const episodes = await cueLightGet<CueLightEpisode[]>(
        binding,
        `/api/projects/${binding.projectId}/episodes`,
      );
      set({
        episodes: episodes ?? [],
        loading: { ...get().loading, episodes: false },
      });
      // Auto-select first episode if none selected
      if (!get().selectedEpisodeId && episodes?.length > 0) {
        set({ selectedEpisodeId: episodes[0].id });
      }
    } catch (e) {
      set({ error: String(e), loading: { ...get().loading, episodes: false } });
    }
  },

  loadStoryboards: async (binding, episodeId) => {
    set({ loading: { ...get().loading, storyboards: true }, error: null });
    try {
      const storyboards = await cueLightGet<CueLightStoryboard[]>(
        binding,
        `/api/episodes/${episodeId}/storyboards`,
      );
      set({
        storyboards: { ...get().storyboards, [episodeId]: storyboards ?? [] },
        loading: { ...get().loading, storyboards: false },
      });
    } catch (e) {
      set({ error: String(e), loading: { ...get().loading, storyboards: false } });
    }
  },

  loadCharacters: async (binding) => {
    set({ loading: { ...get().loading, characters: true }, error: null });
    try {
      const characters = await cueLightGet<CueLightCharacter[]>(
        binding,
        `/api/projects/${binding.projectId}/characters`,
      );
      set({
        characters: characters ?? [],
        loading: { ...get().loading, characters: false },
      });
    } catch (e) {
      set({ error: String(e), loading: { ...get().loading, characters: false } });
    }
  },

  loadScenes: async (binding) => {
    set({ loading: { ...get().loading, scenes: true }, error: null });
    try {
      const scenes = await cueLightGet<CueLightScene[]>(
        binding,
        `/api/projects/${binding.projectId}/scenes`,
      );
      set({
        scenes: scenes ?? [],
        loading: { ...get().loading, scenes: false },
      });
    } catch (e) {
      set({ error: String(e), loading: { ...get().loading, scenes: false } });
    }
  },

  loadProps: async (binding) => {
    set({ loading: { ...get().loading, props: true }, error: null });
    try {
      const props = await cueLightGet<CueLightProp[]>(
        binding,
        `/api/projects/${binding.projectId}/props`,
      );
      set({
        props: props ?? [],
        loading: { ...get().loading, props: false },
      });
    } catch (e) {
      set({ error: String(e), loading: { ...get().loading, props: false } });
    }
  },

  loadVideoAssets: async (binding) => {
    set({ loading: { ...get().loading, videoAssets: true }, error: null });
    try {
      const assets = await cueLightGet<CueLightVideoAsset[]>(
        binding,
        `/api/projects/${binding.projectId}/video-assets`,
      );
      set({
        videoAssets: assets ?? [],
        loading: { ...get().loading, videoAssets: false },
      });
    } catch (e) {
      set({ error: String(e), loading: { ...get().loading, videoAssets: false } });
    }
  },

  setSelectedEpisodeId: (id) => set({ selectedEpisodeId: id }),
  setAssetsTab: (tab) => set({ assetsTab: tab }),
  reset: () => set(initialState),
}));

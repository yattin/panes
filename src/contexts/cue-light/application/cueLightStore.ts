import { create } from "zustand";
import {
  initialCueLightState,
  selectFirstEpisodeIfNeeded,
  type CueLightBible,
  type CueLightCharacter,
  type CueLightEpisode,
  type CueLightProject,
  type CueLightProp,
  type CueLightScene,
  type CueLightState,
  type CueLightStoryboard,
  type CueLightVideoAsset,
} from "../domain/cueLightState";
import { getCueLightGateway } from "./cueLightGateway";

export const useCueLightStore = create<CueLightState>((set, get) => ({
  ...initialCueLightState,

  loadOverview: async (binding) => {
    set({ loading: { ...get().loading, overview: true }, error: null });
    try {
      const [project, bible, videoAssets] = await Promise.all([
        getCueLightGateway().get<CueLightProject>(binding, `/api/projects/${binding.projectId}`),
        getCueLightGateway().get<CueLightBible>(binding, `/api/projects/${binding.projectId}/bible`),
        getCueLightGateway().get<CueLightVideoAsset[]>(
          binding,
          `/api/projects/${binding.projectId}/video-assets`,
        ),
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
      const episodes = await getCueLightGateway().get<CueLightEpisode[]>(
        binding,
        `/api/projects/${binding.projectId}/episodes`,
      );
      const nextEpisodes = episodes ?? [];
      set({
        episodes: nextEpisodes,
        selectedEpisodeId: selectFirstEpisodeIfNeeded(get().selectedEpisodeId, nextEpisodes),
        loading: { ...get().loading, episodes: false },
      });
    } catch (e) {
      set({ error: String(e), loading: { ...get().loading, episodes: false } });
    }
  },

  loadStoryboards: async (binding, episodeId) => {
    set({ loading: { ...get().loading, storyboards: true }, error: null });
    try {
      const storyboards = await getCueLightGateway().get<CueLightStoryboard[]>(
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
      const characters = await getCueLightGateway().get<CueLightCharacter[]>(
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
      const scenes = await getCueLightGateway().get<CueLightScene[]>(
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
      const props = await getCueLightGateway().get<CueLightProp[]>(
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
      const assets = await getCueLightGateway().get<CueLightVideoAsset[]>(
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
  reset: () => set(initialCueLightState),
}));

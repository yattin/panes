import { beforeEach, describe, expect, it, vi } from "vitest";
import type { CueLightProjectBinding } from "../types";

const mockIpc = vi.hoisted(() => ({
  get: vi.fn(),
}));

import { configureCueLightGateway } from "../contexts/cue-light/application/cueLightGateway";
import { useCueLightStore } from "./cueLightStore";

const binding: CueLightProjectBinding = {
  projectId: "project-1",
  projectName: "Project One",
  boundAt: "2026-01-01T00:00:00Z",
};

describe("cueLightStore", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    configureCueLightGateway({
      get: mockIpc.get,
      listProjects: vi.fn(),
      maximizeWindow: vi.fn(),
      openServer: vi.fn(),
      readToken: vi.fn(() => null),
      saveToken: vi.fn(),
      syncAuthToken: vi.fn(),
      validateToken: vi.fn(),
    });
    useCueLightStore.getState().reset();
  });

  it("loads episodes and selects the first episode when none is selected", async () => {
    mockIpc.get.mockResolvedValue([
      { id: "episode-1", title: "Pilot" },
      { id: "episode-2", title: "Second" },
    ]);

    await useCueLightStore.getState().loadEpisodes(binding);

    expect(mockIpc.get).toHaveBeenCalledWith(
      binding,
      "/api/projects/project-1/episodes",
    );
    expect(useCueLightStore.getState()).toMatchObject({
      episodes: [
        { id: "episode-1", title: "Pilot" },
        { id: "episode-2", title: "Second" },
      ],
      selectedEpisodeId: "episode-1",
      error: null,
    });
    expect(useCueLightStore.getState().loading.episodes).toBe(false);
  });
});

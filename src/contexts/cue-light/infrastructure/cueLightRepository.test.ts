import { beforeEach, describe, expect, it, vi } from "vitest";

const mockIpc = vi.hoisted(() => ({
  cueLightProxy: vi.fn(),
  setCueLightAuthToken: vi.fn(),
}));

vi.mock("../../../lib/ipc", () => ({
  ipc: mockIpc,
}));

vi.mock("./cueLightConfig", () => ({
  CUELIGHT_SERVER_URL: "https://cuelight.test",
  getCueLightToken: () => "stored-token",
  setCueLightToken: vi.fn(),
}));

import {
  listCueLightProjects,
  syncCueLightAuthToken,
  validateCueLightToken,
} from "./cueLightRepository";

describe("cueLightRepository", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("validates a token through the CueLight projects endpoint", async () => {
    mockIpc.cueLightProxy.mockResolvedValue([]);

    await validateCueLightToken("token-1");

    expect(mockIpc.cueLightProxy).toHaveBeenCalledWith({
      method: "GET",
      serverUrl: "https://cuelight.test",
      path: "/api/projects",
      authToken: "token-1",
    });
  });

  it("syncs the CueLight token to the native backend", async () => {
    mockIpc.setCueLightAuthToken.mockResolvedValue(undefined);

    await syncCueLightAuthToken("token-1");

    expect(mockIpc.setCueLightAuthToken).toHaveBeenCalledWith("token-1");
  });

  it("lists normalized CueLight projects using the stored token", async () => {
    mockIpc.cueLightProxy.mockResolvedValue({
      items: [
        { id: "project-1", title: "Project One", projectType: "film" },
        { id: "project-2", name: "Project Two" },
      ],
    });

    await expect(listCueLightProjects()).resolves.toEqual([
      { id: "project-1", name: "Project One", projectType: "film" },
      { id: "project-2", name: "Project Two", projectType: undefined },
    ]);
    expect(mockIpc.cueLightProxy).toHaveBeenCalledWith({
      method: "GET",
      serverUrl: "https://cuelight.test",
      path: "/api/projects",
      authToken: "stored-token",
    });
  });
});

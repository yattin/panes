import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-shell";
import { CUELIGHT_SERVER_URL, getCueLightToken } from "./cueLightConfig";
import { setCueLightToken } from "./cueLightConfig";
import { ipc } from "../../../lib/ipc";
import type { CueLightProjectBinding } from "../../../types";
import type { CueLightGateway, CueLightProjectOption } from "../application/cueLightGateway";

export async function cueLightGet<T>(
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

function normalizeCueLightProjectOptions(payload: unknown): CueLightProjectOption[] {
  const list = Array.isArray(payload)
    ? payload
    : Array.isArray((payload as { items?: unknown })?.items)
      ? (payload as { items: unknown[] }).items
      : Array.isArray((payload as { data?: unknown })?.data)
        ? (payload as { data: unknown[] }).data
        : [];

  return list.map((project) => {
    const value = project as {
      id?: unknown;
      title?: unknown;
      name?: unknown;
      projectType?: unknown;
    };
    return {
      id: typeof value.id === "string" ? value.id : "",
      name:
        typeof value.title === "string"
          ? value.title
          : typeof value.name === "string"
            ? value.name
            : "Untitled",
      projectType: typeof value.projectType === "string" ? value.projectType : undefined,
    };
  });
}

export async function validateCueLightToken(token: string): Promise<void> {
  await ipc.cueLightProxy({
    method: "GET",
    serverUrl: CUELIGHT_SERVER_URL,
    path: "/api/projects",
    authToken: token,
  });
}

export async function syncCueLightAuthToken(token: string): Promise<void> {
  await ipc.setCueLightAuthToken(token);
}

export async function listCueLightProjects(token = getCueLightToken()): Promise<CueLightProjectOption[]> {
  if (!token) {
    throw new Error("请先配置 API Token");
  }

  const result = await ipc.cueLightProxy({
    method: "GET",
    serverUrl: CUELIGHT_SERVER_URL,
    path: "/api/projects",
    authToken: token,
  });
  return normalizeCueLightProjectOptions(result);
}

export const cueLightRepository: CueLightGateway = {
  get: cueLightGet,
  listProjects: listCueLightProjects,
  maximizeWindow: () => getCurrentWindow().maximize(),
  openServer: () => open(CUELIGHT_SERVER_URL),
  readToken: getCueLightToken,
  saveToken: setCueLightToken,
  syncAuthToken: syncCueLightAuthToken,
  validateToken: validateCueLightToken,
};

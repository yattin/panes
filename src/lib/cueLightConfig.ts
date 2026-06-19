/**
 * CueLight 全局配置
 * - 固定服务器 URL
 * - 全局 API Token 存储（localStorage）
 */

export const CUELIGHT_SERVER_URL = "https://cuelight.app";

const CUELIGHT_TOKEN_KEY = "panes.cuelight.authToken";

export function getCueLightToken(): string | null {
  return localStorage.getItem(CUELIGHT_TOKEN_KEY);
}

export function setCueLightToken(token: string): void {
  localStorage.setItem(CUELIGHT_TOKEN_KEY, token);
}

export function clearCueLightToken(): void {
  localStorage.removeItem(CUELIGHT_TOKEN_KEY);
}

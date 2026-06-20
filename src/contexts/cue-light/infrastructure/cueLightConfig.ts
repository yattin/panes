/**
 * CueLight global configuration.
 * - Fixed server URL
 * - Global API token storage (localStorage)
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

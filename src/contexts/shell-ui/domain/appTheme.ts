export type AppTheme = "dark" | "light";

export const DEFAULT_APP_THEME: AppTheme = "dark";
export const SUPPORTED_APP_THEMES: AppTheme[] = ["dark", "light"];

export function normalizeAppTheme(theme: string | null | undefined): AppTheme | null {
  if (!theme) return null;
  const normalized = theme.trim().toLowerCase();
  return normalized === "dark" || normalized === "light" ? normalized : null;
}

export function applyAppTheme(
  theme: AppTheme,
  doc: Document | undefined = globalThis.document,
): void {
  doc?.documentElement.setAttribute("data-theme", theme);
}

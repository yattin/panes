import { normalizeAppLocale, type AppLocale } from "../domain/appLocale";

export function getBrowserLocaleFallback(): AppLocale {
  if (typeof navigator !== "undefined" && typeof navigator.language === "string") {
    return normalizeAppLocale(navigator.language);
  }
  return "en";
}

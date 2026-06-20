import { describe, expect, it } from "vitest";

import {
  getLocaleDisplayName,
  isAppLocale,
  normalizeAppLocale,
  SUPPORTED_APP_LOCALES,
} from "./appLocale";

describe("app locale", () => {
  it("normalizes supported browser and persisted locale values", () => {
    expect(normalizeAppLocale("pt")).toBe("pt-BR");
    expect(normalizeAppLocale("pt_PT")).toBe("pt-BR");
    expect(normalizeAppLocale("zh-Hans-SG")).toBe("zh-CN");
    expect(normalizeAppLocale("en-US")).toBe("en");
    expect(normalizeAppLocale("fr-FR")).toBe("en");
    expect(normalizeAppLocale(null)).toBe("en");
  });

  it("exposes the supported locale contract used by selectors", () => {
    expect(SUPPORTED_APP_LOCALES).toEqual(["en", "pt-BR", "zh-CN"]);
    expect(isAppLocale("zh-CN")).toBe(true);
    expect(isAppLocale("zh")).toBe(false);
    expect(getLocaleDisplayName("pt-BR")).toBe("Português (Brasil)");
  });
});

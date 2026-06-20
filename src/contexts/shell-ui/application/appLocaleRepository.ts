import type { AppLocale } from "../domain/appLocale";
import { getShellUiGateway } from "./shellUiGateway";

export const appLocaleRepository = {
  getPersistedLocale(): Promise<AppLocale | null> {
    return getShellUiGateway().getPersistedAppLocale();
  },
  setPersistedLocale(locale: AppLocale): Promise<AppLocale> {
    return getShellUiGateway().setPersistedAppLocale(locale);
  },
};

import { ipc } from "../../../lib/ipc";
import type { AppLocale } from "../domain/appLocale";

export const appLocaleRepository = {
  getPersistedLocale: ipc.getAppLocale,
  setPersistedLocale(locale: AppLocale): Promise<AppLocale> {
    return ipc.setAppLocale(locale);
  },
};

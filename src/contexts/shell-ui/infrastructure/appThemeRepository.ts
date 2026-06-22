import { ipc } from "../../../lib/ipc";
import type { AppTheme } from "../domain/appTheme";

export const appThemeRepository = {
  getPersistedTheme: ipc.getAppTheme,
  setPersistedTheme(theme: AppTheme): Promise<AppTheme> {
    return ipc.setAppTheme(theme);
  },
};

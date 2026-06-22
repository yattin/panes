import { useTranslation } from "react-i18next";
import {
  canCustomWindowResize,
  shouldShowCustomWindowChrome,
  type CustomWindowFrameState,
} from "../../contexts/shell-ui/domain/customWindowFrame";
import {
  closeCurrentWindow,
  minimizeCurrentWindow,
  toggleCurrentWindowMaximize,
} from "../../contexts/shell-ui/application/windowActions";
import { handleDragDoubleClick, handleDragMouseDown } from "../../contexts/shell-ui/application/windowDrag";
import { CustomWindowResizeHandles } from "./CustomWindowResizeHandles";

interface CustomWindowFrameProps {
  frameState: CustomWindowFrameState;
}

export function CustomWindowFrame({ frameState }: CustomWindowFrameProps) {
  const { t } = useTranslation(["app", "native"]);
  const showChrome = shouldShowCustomWindowChrome(frameState);

  return (
    <>
      {showChrome && (
        <div
          className="linux-window-chrome"
          onMouseDown={handleDragMouseDown}
          onDoubleClick={handleDragDoubleClick}
        >
          <div className="linux-window-chrome-drag-region" />
          <div className="linux-window-chrome-controls no-drag">
            <button
              type="button"
              className="linux-window-control"
              aria-label={t("windowControls.minimize")}
              title={t("windowControls.minimize")}
              onClick={() => {
                void minimizeCurrentWindow();
              }}
            >
              <span className="linux-window-control-icon linux-window-control-icon-minimize" />
            </button>
            <button
              type="button"
              className="linux-window-control"
              aria-label={t(frameState.isMaximized ? "windowControls.restore" : "windowControls.maximize")}
              title={t(frameState.isMaximized ? "windowControls.restore" : "windowControls.maximize")}
              onClick={() => {
                void toggleCurrentWindowMaximize();
              }}
            >
              <span
                className={`linux-window-control-icon ${
                  frameState.isMaximized
                    ? "linux-window-control-icon-restore"
                    : "linux-window-control-icon-maximize"
                }`}
              />
            </button>
            <button
              type="button"
              className="linux-window-control linux-window-control-close"
              aria-label={t("windowControls.close")}
              title={t("windowControls.close")}
              onClick={() => {
                void closeCurrentWindow();
              }}
            >
              <span className="linux-window-control-icon linux-window-control-icon-close" />
            </button>
          </div>
        </div>
      )}
      <CustomWindowResizeHandles canResize={canCustomWindowResize(frameState)} />
    </>
  );
}

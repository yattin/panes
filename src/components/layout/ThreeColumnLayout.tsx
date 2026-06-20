import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Panel, PanelGroup, PanelResizeHandle } from "react-resizable-panels";
import { Sidebar } from "../sidebar/Sidebar";
import { ActiveWorkspacePaneShell } from "../workspace/WorkspacePaneShell";
import { HarnessPanel } from "../onboarding/HarnessPanel";
import { WorkspaceSettingsPage } from "../workspace/WorkspaceSettingsPage";
import { CueLightPanel } from "../cuelight/CueLightPanel";
import { usesCustomWindowFrame } from "../../lib/windowActions";
import { useUiStore } from "../../stores/uiStore";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { handleDragDoubleClick, handleDragMouseDown } from "../../lib/windowDrag";

const SIDEBAR_WIDTH_KEY = "panes:sidebar-width";
const GIT_PANEL_SIZE_KEY = "panes:git-panel-size";
const MIN_SIDEBAR = 160;
const MAX_SIDEBAR = 380;
const DEFAULT_SIDEBAR = 220;
const MIN_GIT_PANEL_SIZE = 18;
const MAX_GIT_PANEL_SIZE = 40;
const DEFAULT_GIT_PANEL_SIZE = 26;
const MIN_GIT_FLYOUT_WIDTH = 260;
const MAX_GIT_FLYOUT_WIDTH = 560;
const RESIZE_HANDLE_CLICK_THRESHOLD = 4;

function loadSidebarWidth(): number {
  try {
    const stored = localStorage.getItem(SIDEBAR_WIDTH_KEY);
    if (stored) {
      const v = parseInt(stored, 10);
      if (v >= MIN_SIDEBAR && v <= MAX_SIDEBAR) return v;
    }
  } catch { /* ignore */ }
  return DEFAULT_SIDEBAR;
}

function loadGitPanelSize(): number {
  try {
    const stored = localStorage.getItem(GIT_PANEL_SIZE_KEY);
    if (stored) {
      const value = Number.parseFloat(stored);
      if (value >= MIN_GIT_PANEL_SIZE && value <= MAX_GIT_PANEL_SIZE) {
        return value;
      }
    }
  } catch {
    // Ignore storage failures in non-browser/test environments.
  }
  return DEFAULT_GIT_PANEL_SIZE;
}

export function ThreeColumnLayout() {
  const showSidebar = useUiStore((state) => state.showSidebar);
  const sidebarPinned = useUiStore((state) => state.sidebarPinned);
  const toggleSidebarPin = useUiStore((state) => state.toggleSidebarPin);
  const focusMode = useUiStore((state) => state.focusMode);
  const activeView = useUiStore((state) => state.activeView);
  const customWindowFrame = usesCustomWindowFrame();
  const activeWorkspaceId = useWorkspaceStore((state) => state.activeWorkspaceId);

  const sidebarDocked = showSidebar && sidebarPinned;
  const fullBleedContent = focusMode || !showSidebar;
  const showCueLightPanel = activeWorkspaceId !== null;

  const [sidebarWidth, setSidebarWidth] = useState(loadSidebarWidth);
  const [cuelightPanelSize, setCuelightPanelSize] = useState(loadGitPanelSize);
  const sidebarHandleRef = useRef<HTMLDivElement>(null);
  const contentCardRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    try { localStorage.setItem(SIDEBAR_WIDTH_KEY, String(sidebarWidth)); } catch { /* ignore */ }
  }, [sidebarWidth]);

  useEffect(() => {
    try { localStorage.setItem(GIT_PANEL_SIZE_KEY, String(cuelightPanelSize)); } catch { /* ignore */ }
  }, [cuelightPanelSize]);

  useEffect(() => {
    const contentCard = contentCardRef.current;
    if (!contentCard) {
      return;
    }

    const updateWidth = () => {
      // Keep contentCardWidth state for potential future use
    };

    updateWidth();
    const observer = new ResizeObserver(updateWidth);
    observer.observe(contentCard);
    return () => observer.disconnect();
  }, []);

  const handleSidebarResizeMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    const startX = e.clientX;
    const startWidth = sidebarWidth;
    let isDragging = false;
    sidebarHandleRef.current?.classList.add("dragging");

    function onMove(ev: MouseEvent) {
      const delta = ev.clientX - startX;
      if (!isDragging && Math.abs(delta) < RESIZE_HANDLE_CLICK_THRESHOLD) {
        return;
      }
      isDragging = true;
      setSidebarWidth(Math.min(MAX_SIDEBAR, Math.max(MIN_SIDEBAR, startWidth + delta)));
    }

    function onUp(ev: MouseEvent) {
      sidebarHandleRef.current?.classList.remove("dragging");
      document.removeEventListener("mousemove", onMove);
      document.removeEventListener("mouseup", onUp);
      if (!isDragging && Math.abs(ev.clientX - startX) < RESIZE_HANDLE_CLICK_THRESHOLD) {
        toggleSidebarPin();
      }
    }

    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
  }, [sidebarWidth, toggleSidebarPin]);

  const mainContent = (
    activeView === "harnesses" ? (
      <HarnessPanel />
    ) : activeView === "workspace-settings" ? (
      <WorkspaceSettingsPage />
    ) : (
      <ActiveWorkspacePaneShell />
    )
  );

  return (
    <div className="layout-root">
      {/* Unpinned sidebar — collapsed rail + hover flyout */}
      {showSidebar && !sidebarPinned && <Sidebar />}

      {/* Pinned sidebar */}
      {sidebarDocked && (
        <div className="layout-sidebar" style={{ width: sidebarWidth }}>
          <Sidebar />
        </div>
      )}

      {/* Sidebar resize handle (pinned only) */}
      {sidebarDocked && (
        <div
          ref={sidebarHandleRef}
          className="sidebar-resize-handle"
          onMouseDown={handleSidebarResizeMouseDown}
        />
      )}

      {/* Floating content card */}
      <div
        ref={contentCardRef}
        className={`content-card ${fullBleedContent ? "content-card-full" : ""}`}
      >
        {showCueLightPanel && activeWorkspaceId ? (
          <PanelGroup
            key="main-layout-docked"
            id="main-layout-panels"
            autoSaveId="panes:main-layout-panels"
            direction="horizontal"
            style={{ height: "100%", flex: 1 }}
          >
            <Panel
              id="main-layout-content"
              order={1}
              defaultSize={100 - cuelightPanelSize}
              minSize={35}
            >
              <div className="content-panel" style={{ height: "100%" }}>
                {mainContent}
              </div>
            </Panel>

            <PanelResizeHandle
              id="main-layout-cuelight-resize-handle"
              className="resize-handle"
            />

            <Panel
              id="main-layout-cuelight-panel"
              order={2}
              defaultSize={cuelightPanelSize}
              minSize={MIN_GIT_PANEL_SIZE}
              maxSize={MAX_GIT_PANEL_SIZE}
              onResize={setCuelightPanelSize}
            >
              <div className="content-panel" style={{ height: "100%" }}>
                <CueLightPanel workspaceId={activeWorkspaceId} />
              </div>
            </Panel>
          </PanelGroup>
        ) : (
          <div className="content-panel" style={{ height: "100%", flex: 1 }}>
            {mainContent}
          </div>
        )}
      </div>
    </div>
  );
}

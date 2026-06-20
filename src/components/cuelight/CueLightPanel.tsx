import { useState } from "react";
import { LayoutDashboard, Clapperboard, ImageIcon } from "lucide-react";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { CueLightOverview } from "./CueLightOverview";
import { CueLightStoryboard } from "./CueLightStoryboard";
import { CueLightAssets } from "./CueLightAssets";

type CueLightTab = "overview" | "storyboard" | "assets";

interface CueLightPanelProps {
  workspaceId: string;
}

const TABS: { id: CueLightTab; label: string; icon: typeof LayoutDashboard }[] = [
  { id: "overview", label: "概览", icon: LayoutDashboard },
  { id: "storyboard", label: "分镜", icon: Clapperboard },
  { id: "assets", label: "资产", icon: ImageIcon },
];

export function CueLightPanel({ workspaceId }: CueLightPanelProps) {
  const workspace = useWorkspaceStore((s) =>
    s.workspaces.find((w) => w.id === workspaceId),
  );
  const binding = workspace?.cueLightBinding;
  const [activeTab, setActiveTab] = useState<CueLightTab>("overview");

  if (!binding) {
    return (
      <div className="cuelight-panel-root">
        <div className="cuelight-panel-empty">
          <Clapperboard size={32} strokeWidth={1.5} />
          <p>未绑定 CueLight 项目</p>
          <p className="cuelight-panel-empty-hint">
            请通过侧栏创建影视工作区并绑定项目
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="cuelight-panel-root">
      {/* Tab header */}
      <div className="cuelight-panel-tabs">
        {TABS.map((tab) => {
          const Icon = tab.icon;
          const active = activeTab === tab.id;
          return (
            <button
              key={tab.id}
              type="button"
              className={`cuelight-panel-tab${active ? " active" : ""}`}
              onClick={() => setActiveTab(tab.id)}
            >
              <Icon size={14} />
              <span>{tab.label}</span>
            </button>
          );
        })}
      </div>

      {/* Tab content */}
      <div className="cuelight-panel-content">
        {activeTab === "overview" && <CueLightOverview />}
        {activeTab === "storyboard" && <CueLightStoryboard />}
        {activeTab === "assets" && <CueLightAssets />}
      </div>
    </div>
  );
}

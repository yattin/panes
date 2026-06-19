import { useEffect } from "react";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useCueLightStore, type AssetsTab } from "../../stores/cueLightStore";

const TABS: { key: AssetsTab; label: string }[] = [
  { key: "characters", label: "角色" },
  { key: "scenes", label: "场景" },
  { key: "props", label: "道具" },
  { key: "history", label: "生成记录" },
];

export function CueLightAssets() {
  const workspace = useWorkspaceStore((s) =>
    s.workspaces.find((w) => w.id === s.activeWorkspaceId),
  );
  const binding = workspace?.cueLightBinding;

  const {
    characters,
    scenes,
    props,
    videoAssets,
    assetsTab,
    loading,
    error,
    loadCharacters,
    loadScenes,
    loadProps,
    loadVideoAssets,
    setAssetsTab,
  } = useCueLightStore();

  useEffect(() => {
    if (!binding) return;
    switch (assetsTab) {
      case "characters":
        loadCharacters(binding);
        break;
      case "scenes":
        loadScenes(binding);
        break;
      case "props":
        loadProps(binding);
        break;
      case "history":
        loadVideoAssets(binding);
        break;
    }
  }, [binding?.projectId, assetsTab]);

  if (!binding) {
    return (
      <div className="cuelight-panel cuelight-assets">
        <p className="cuelight-empty">未绑定 CueLight 项目</p>
      </div>
    );
  }

  const isLoading = loading[assetsTab];

  return (
    <div className="cuelight-panel cuelight-assets">
      {/* Sub-tabs */}
      <div className="cuelight-assets-tabs">
        {TABS.map((tab) => (
          <button
            key={tab.key}
            type="button"
            className={`cuelight-assets-tab${assetsTab === tab.key ? " active" : ""}`}
            onClick={() => setAssetsTab(tab.key)}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {/* Content */}
      {isLoading ? (
        <p className="cuelight-loading">加载中...</p>
      ) : (
        <div className="cuelight-assets-content">
          {assetsTab === "characters" && (
            <div className="cuelight-card-grid">
              {characters.length === 0 && <p className="cuelight-empty">暂无角色</p>}
              {characters.map((c) => (
                <div key={c.id} className="cuelight-asset-card">
                  <div className="cuelight-asset-thumb">
                    {c.referenceImageUrl ? (
                      <img src={c.referenceImageUrl} alt={c.name} />
                    ) : (
                      <div className="cuelight-media-placeholder" />
                    )}
                  </div>
                  <div className="cuelight-asset-info">
                    <span className="cuelight-asset-name">{c.name}</span>
                    {c.description && (
                      <p className="cuelight-asset-desc">
                        {c.description.length > 50
                          ? c.description.slice(0, 50) + "..."
                          : c.description}
                      </p>
                    )}
                  </div>
                </div>
              ))}
            </div>
          )}

          {assetsTab === "scenes" && (
            <div className="cuelight-card-grid">
              {scenes.length === 0 && <p className="cuelight-empty">暂无场景</p>}
              {scenes.map((s) => (
                <div key={s.id} className="cuelight-asset-card">
                  <div className="cuelight-asset-thumb">
                    {s.referenceImageUrl ? (
                      <img src={s.referenceImageUrl} alt={s.name} />
                    ) : (
                      <div className="cuelight-media-placeholder" />
                    )}
                  </div>
                  <div className="cuelight-asset-info">
                    <span className="cuelight-asset-name">{s.name}</span>
                    {s.description && (
                      <p className="cuelight-asset-desc">
                        {s.description.length > 50
                          ? s.description.slice(0, 50) + "..."
                          : s.description}
                      </p>
                    )}
                  </div>
                </div>
              ))}
            </div>
          )}

          {assetsTab === "props" && (
            <div className="cuelight-card-grid">
              {props.length === 0 && <p className="cuelight-empty">暂无道具</p>}
              {props.map((p) => (
                <div key={p.id} className="cuelight-asset-card">
                  <div className="cuelight-asset-thumb">
                    {p.referenceImageUrl ? (
                      <img src={p.referenceImageUrl} alt={p.name} />
                    ) : (
                      <div className="cuelight-media-placeholder" />
                    )}
                  </div>
                  <div className="cuelight-asset-info">
                    <span className="cuelight-asset-name">{p.name}</span>
                  </div>
                </div>
              ))}
            </div>
          )}

          {assetsTab === "history" && (
            <div className="cuelight-card-grid">
              {videoAssets.length === 0 && <p className="cuelight-empty">暂无生成记录</p>}
              {videoAssets.map((asset) => (
                <div key={asset.id} className="cuelight-asset-card">
                  <div className="cuelight-asset-thumb">
                    {asset.thumbnailUrl ? (
                      <img src={asset.thumbnailUrl} alt="" />
                    ) : asset.url ? (
                      <video src={asset.url} muted className="cuelight-video-thumb" />
                    ) : (
                      <div className="cuelight-media-placeholder" />
                    )}
                  </div>
                  <div className="cuelight-asset-info">
                    <span className="cuelight-asset-name">
                      {asset.createdAt
                        ? new Date(asset.createdAt).toLocaleString("zh-CN")
                        : asset.id.slice(0, 8)}
                    </span>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {error && <p className="cuelight-error">{error}</p>}
    </div>
  );
}

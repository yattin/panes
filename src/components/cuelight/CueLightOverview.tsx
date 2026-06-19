import { useEffect } from "react";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useCueLightStore } from "../../stores/cueLightStore";

export function CueLightOverview() {
  const activeWorkspaceId = useWorkspaceStore((s) => s.activeWorkspaceId);
  const workspace = useWorkspaceStore((s) =>
    s.workspaces.find((w) => w.id === s.activeWorkspaceId),
  );
  const binding = workspace?.cueLightBinding;

  const { projectDetail, bible, videoAssets, loading, error, loadOverview } =
    useCueLightStore();

  useEffect(() => {
    if (binding) {
      loadOverview(binding);
    }
  }, [binding?.projectId]);

  if (!binding) {
    return (
      <div className="cuelight-panel cuelight-overview">
        <p className="cuelight-empty">未绑定 CueLight 项目</p>
      </div>
    );
  }

  if (loading.overview) {
    return (
      <div className="cuelight-panel cuelight-overview">
        <p className="cuelight-loading">加载中...</p>
      </div>
    );
  }

  if (error) {
    return (
      <div className="cuelight-panel cuelight-overview">
        <p className="cuelight-error">{error}</p>
      </div>
    );
  }

  const episodeCount =
    projectDetail?.episodes?.length ?? 0;
  const storyboardCount =
    projectDetail?.storyboards?.length ?? 0;

  return (
    <div className="cuelight-panel cuelight-overview">
      {/* Project header */}
      <div className="cuelight-overview-header">
        <h2 className="cuelight-overview-title">
          {(projectDetail?.title as string) ?? binding.projectName}
        </h2>
        <div className="cuelight-overview-meta">
          {projectDetail?.projectType && (
            <span className="cuelight-tag">{projectDetail.projectType}</span>
          )}
          {projectDetail?.videoAspectRatio && (
            <span className="cuelight-tag">{projectDetail.videoAspectRatio}</span>
          )}
        </div>
      </div>

      {/* Progress overview */}
      <div className="cuelight-overview-stats">
        <div className="cuelight-stat">
          <span className="cuelight-stat-value">{episodeCount}</span>
          <span className="cuelight-stat-label">集数</span>
        </div>
        <div className="cuelight-stat">
          <span className="cuelight-stat-value">{storyboardCount}</span>
          <span className="cuelight-stat-label">分镜</span>
        </div>
        <div className="cuelight-stat">
          <span className="cuelight-stat-value">{videoAssets?.length ?? 0}</span>
          <span className="cuelight-stat-label">视频</span>
        </div>
      </div>

      {/* World view */}
      {bible?.worldView && (
        <div className="cuelight-overview-section">
          <h3>世界观</h3>
          <p className="cuelight-overview-text">
            {bible.worldView.length > 200
              ? bible.worldView.slice(0, 200) + "..."
              : bible.worldView}
          </p>
        </div>
      )}

      {/* Style prompt */}
      {bible?.stylePrompt && (
        <div className="cuelight-overview-section">
          <h3>风格设定</h3>
          <p className="cuelight-overview-text cuelight-style-prompt">
            {bible.stylePrompt}
          </p>
        </div>
      )}

      {/* Recent media */}
      {videoAssets && videoAssets.length > 0 && (
        <div className="cuelight-overview-section">
          <h3>最近生成</h3>
          <div className="cuelight-overview-media-grid">
            {videoAssets.slice(0, 4).map((asset) => (
              <div key={asset.id} className="cuelight-media-thumb">
                {asset.thumbnailUrl ? (
                  <img src={asset.thumbnailUrl} alt="" />
                ) : (
                  <div className="cuelight-media-placeholder" />
                )}
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

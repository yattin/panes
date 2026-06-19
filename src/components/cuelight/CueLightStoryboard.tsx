import { useEffect, useCallback } from "react";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useCueLightStore, type CueLightStoryboard as StoryboardItem } from "../../stores/cueLightStore";

export function CueLightStoryboard() {
  const workspace = useWorkspaceStore((s) =>
    s.workspaces.find((w) => w.id === s.activeWorkspaceId),
  );
  const binding = workspace?.cueLightBinding;

  const {
    episodes,
    storyboards,
    characters,
    selectedEpisodeId,
    loading,
    error,
    loadEpisodes,
    loadStoryboards,
    loadCharacters,
    setSelectedEpisodeId,
  } = useCueLightStore();

  useEffect(() => {
    if (binding) {
      loadEpisodes(binding);
      loadCharacters(binding);
    }
  }, [binding?.projectId]);

  useEffect(() => {
    if (binding && selectedEpisodeId) {
      loadStoryboards(binding, selectedEpisodeId);
    }
  }, [binding?.projectId, selectedEpisodeId]);

  const handleEpisodeSelect = useCallback((episodeId: string) => {
    setSelectedEpisodeId(episodeId);
  }, [setSelectedEpisodeId]);

  if (!binding) {
    return (
      <div className="cuelight-panel cuelight-storyboard">
        <p className="cuelight-empty">未绑定 CueLight 项目</p>
      </div>
    );
  }

  if (loading.episodes) {
    return (
      <div className="cuelight-panel cuelight-storyboard">
        <p className="cuelight-loading">加载中...</p>
      </div>
    );
  }

  const currentStoryboards = selectedEpisodeId
    ? storyboards[selectedEpisodeId] ?? []
    : [];

  return (
    <div className="cuelight-panel cuelight-storyboard">
      {/* Episode tabs */}
      <div className="cuelight-episode-tabs">
        {episodes.map((ep) => (
          <button
            key={ep.id}
            type="button"
            className={`cuelight-episode-tab${selectedEpisodeId === ep.id ? " active" : ""}`}
            onClick={() => handleEpisodeSelect(ep.id)}
          >
            {ep.title || `第 ${ep.number ?? "?"} 集`}
          </button>
        ))}
        {episodes.length === 0 && (
          <span className="cuelight-empty-inline">暂无集数</span>
        )}
      </div>

      {/* Storyboard grid */}
      {loading.storyboards ? (
        <p className="cuelight-loading">加载分镜...</p>
      ) : (
        <div className="cuelight-storyboard-grid">
          {currentStoryboards.length === 0 && selectedEpisodeId && (
            <p className="cuelight-empty">该集暂无分镜</p>
          )}
          {currentStoryboards.map((sb) => (
            <StoryboardCard
              key={sb.id}
              storyboard={sb}
              characters={characters}
            />
          ))}
        </div>
      )}

      {error && <p className="cuelight-error">{error}</p>}
    </div>
  );
}

function StoryboardCard({
  storyboard,
  characters,
}: {
  storyboard: StoryboardItem;
  characters: { id: string; name: string; referenceImageUrl?: string }[];
}) {
  const linkedChars = characters.filter((c) =>
    storyboard.referenceCharacterIds?.includes(c.id),
  );

  const videoStatus = storyboard.videoUrl
    ? "done"
    : storyboard.status === "processing"
      ? "processing"
      : "pending";

  const statusIcon =
    videoStatus === "done" ? "✅" : videoStatus === "processing" ? "⏳" : "○";

  return (
    <div className="cuelight-storyboard-card">
      <div className="cuelight-storyboard-thumb">
        {storyboard.referenceImageUrl ? (
          <img src={storyboard.referenceImageUrl} alt="" />
        ) : (
          <div className="cuelight-media-placeholder" />
        )}
        <span className="cuelight-storyboard-status">{statusIcon}</span>
      </div>
      <div className="cuelight-storyboard-info">
        {storyboard.sceneNumber != null && (
          <span className="cuelight-scene-number">#{storyboard.sceneNumber}</span>
        )}
        <p className="cuelight-storyboard-prompt">
          {storyboard.videoPrompt
            ? storyboard.videoPrompt.length > 60
              ? storyboard.videoPrompt.slice(0, 60) + "..."
              : storyboard.videoPrompt
            : "无描述"}
        </p>
        {linkedChars.length > 0 && (
          <div className="cuelight-storyboard-chars">
            {linkedChars.map((c) => (
              <span key={c.id} className="cuelight-char-badge" title={c.name}>
                {c.referenceImageUrl ? (
                  <img src={c.referenceImageUrl} alt={c.name} />
                ) : (
                  c.name.charAt(0)
                )}
              </span>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

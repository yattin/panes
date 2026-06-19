import { useState, useCallback, useEffect } from "react";
import { Loader2 } from "lucide-react";
import { ipc } from "../../lib/ipc";
import { CUELIGHT_SERVER_URL, getCueLightToken } from "../../lib/cueLightConfig";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useCueLightStore } from "../../stores/cueLightStore";

interface ProjectOption {
  id: string;
  name: string;
  projectType?: string;
}

interface CueLightProjectPickerProps {
  workspaceId?: string;
  onProjectSelected?: (project: { id: string; name: string }) => void;
}

export function CueLightProjectPicker({ workspaceId, onProjectSelected }: CueLightProjectPickerProps) {
  const workspace = useWorkspaceStore((s) =>
    workspaceId ? s.workspaces.find((w) => w.id === workspaceId) : undefined,
  );
  const binding = workspace?.cueLightBinding;
  const bindCueLight = useWorkspaceStore((s) => s.bindCueLight);
  const unbindCueLight = useWorkspaceStore((s) => s.unbindCueLight);
  const resetCueLightStore = useCueLightStore((s) => s.reset);

  const [projects, setProjects] = useState<ProjectOption[]>([]);
  const [loadingProjects, setLoadingProjects] = useState(false);
  const [selectedProjectId, setSelectedProjectId] = useState<string | null>(
    binding?.projectId ?? null,
  );
  const [error, setError] = useState<string | null>(null);
  const [binding_, setBindingInProgress] = useState(false);

  // 组件加载时自动获取项目列表
  useEffect(() => {
    if (!binding) {
      fetchProjects();
    }
  }, []);

  const fetchProjects = useCallback(async () => {
    const token = getCueLightToken();
    if (!token) {
      setError("请先配置 API Token");
      return;
    }

    setLoadingProjects(true);
    setError(null);
    try {
      const result = await ipc.cueLightProxy({
        method: "GET",
        serverUrl: CUELIGHT_SERVER_URL,
        path: "/api/projects",
        authToken: token,
      });
      const list = Array.isArray(result) ? result : (result as any)?.data ?? [];
      setProjects(
        list.map((p: any) => ({
          id: p.id,
          name: p.title ?? p.name ?? "Untitled",
          projectType: p.projectType,
        })),
      );
    } catch (e) {
      setError(String(e));
      setProjects([]);
    } finally {
      setLoadingProjects(false);
    }
  }, []);

  const handleBind = useCallback(async () => {
    if (!selectedProjectId || !workspaceId) return;
    const project = projects.find((p) => p.id === selectedProjectId);
    if (!project) return;

    setBindingInProgress(true);
    setError(null);
    try {
      await bindCueLight(workspaceId, {
        projectId: project.id,
        projectName: project.name,
      });
    } catch (e) {
      setError(String(e));
    } finally {
      setBindingInProgress(false);
    }
  }, [selectedProjectId, projects, workspaceId, bindCueLight]);

  const handleSelect = useCallback((projectId: string) => {
    if (!onProjectSelected) return;
    const project = projects.find((p) => p.id === projectId);
    if (!project) return;
    onProjectSelected({ id: project.id, name: project.name });
  }, [projects, onProjectSelected]);

  const handleRadioChange = useCallback((projectId: string) => {
    setSelectedProjectId(projectId);
    // 选择模式下自动触发回调
    if (!workspaceId && onProjectSelected) {
      handleSelect(projectId);
    }
  }, [workspaceId, onProjectSelected, handleSelect]);

  const handleUnbind = useCallback(async () => {
    if (!workspaceId) return;
    try {
      await unbindCueLight(workspaceId);
      resetCueLightStore();
      setSelectedProjectId(null);
      fetchProjects();
    } catch (e) {
      setError(String(e));
    }
  }, [workspaceId, unbindCueLight, resetCueLightStore, fetchProjects]);

  return (
    <div className="cuelight-project-picker">
      <h3 className="cuelight-picker-title">CueLight 项目</h3>

      {binding ? (
        <div className="cuelight-picker-bound">
          <div className="cuelight-picker-bound-info">
            <span className="cuelight-picker-label">已绑定:</span>
            <span className="cuelight-picker-value">{binding.projectName}</span>
          </div>
          <button
            type="button"
            className="cuelight-picker-unbind-btn"
            onClick={handleUnbind}
          >
            更换项目
          </button>
        </div>
      ) : (
        <div className="cuelight-picker-form">
          {loadingProjects ? (
            <div className="cuelight-picker-loading">
              <Loader2 size={16} className="animate-spin" />
              <span>加载项目列表...</span>
            </div>
          ) : projects.length > 0 ? (
            <div className="cuelight-picker-project-list">
              {projects.map((p) => (
                <label key={p.id} className="cuelight-picker-project-item">
                  <input
                    type="radio"
                    name="cuelight-project"
                    value={p.id}
                    checked={selectedProjectId === p.id}
                    onChange={() => handleRadioChange(p.id)}
                  />
                  <span className="cuelight-picker-project-name">{p.name}</span>
                  {p.projectType && (
                    <span className="cuelight-picker-project-type">{p.projectType}</span>
                  )}
                </label>
              ))}
            </div>
          ) : (
            <p className="cuelight-picker-empty">暂无可用项目</p>
          )}

          {selectedProjectId && workspaceId && (
            <button
              type="button"
              className="cuelight-picker-bind-btn"
              onClick={handleBind}
              disabled={binding_}
            >
              {binding_ ? (
                <>
                  <Loader2 size={14} className="animate-spin" />
                  绑定中...
                </>
              ) : (
                "绑定选中项目"
              )}
            </button>
          )}

          <button
            type="button"
            className="cuelight-picker-refresh-btn"
            onClick={fetchProjects}
            disabled={loadingProjects}
          >
            刷新列表
          </button>
        </div>
      )}

      {error && <p className="cuelight-error">{error}</p>}
    </div>
  );
}

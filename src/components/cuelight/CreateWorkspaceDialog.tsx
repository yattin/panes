import { useState, useCallback } from "react";
import { createPortal } from "react-dom";
import { open as openDirectoryDialog } from "@tauri-apps/plugin-dialog";
import { FolderOpen, X, Loader2 } from "lucide-react";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { CueLightProjectPicker } from "./CueLightProjectPicker";

interface CreateWorkspaceDialogProps {
  onClose: () => void;
}

export function CreateWorkspaceDialog({ onClose }: CreateWorkspaceDialogProps) {
  const openWorkspace = useWorkspaceStore((s) => s.openWorkspace);
  const bindCueLight = useWorkspaceStore((s) => s.bindCueLight);

  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [selectedProject, setSelectedProject] = useState<{ id: string; name: string } | null>(null);
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSelectDirectory = useCallback(async () => {
    const selected = await openDirectoryDialog({ directory: true, multiple: false });
    if (!selected || Array.isArray(selected)) return;
    setSelectedPath(selected);
  }, []);

  const handleProjectSelected = useCallback((project: { id: string; name: string }) => {
    setSelectedProject(project);
  }, []);

  const handleCreate = useCallback(async () => {
    if (!selectedPath || !selectedProject) return;

    setCreating(true);
    setError(null);
    try {
      const workspace = await openWorkspace(selectedPath);
      if (!workspace) {
        throw new Error("工作区创建失败");
      }
      await bindCueLight(workspace.id, {
        projectId: selectedProject.id,
        projectName: selectedProject.name,
      });
      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setCreating(false);
    }
  }, [selectedPath, selectedProject, openWorkspace, bindCueLight, onClose]);

  const canCreate = selectedPath && selectedProject && !creating;

  return createPortal(
    <div className="modal-overlay" onClick={onClose}>
      <div className="create-workspace-dialog" onClick={(e) => e.stopPropagation()}>
        <div className="create-workspace-dialog-header">
          <h2>创建影视工作区</h2>
          <button type="button" className="modal-close-btn" onClick={onClose}>
            <X size={18} />
          </button>
        </div>

        <div className="create-workspace-dialog-body">
          {/* 目录选择 */}
          <div className="create-workspace-section">
            <label className="create-workspace-label">
              <FolderOpen size={14} />
              本地工作目录
            </label>
            <div className="create-workspace-path-row">
              <input
                type="text"
                value={selectedPath ?? ""}
                readOnly
                placeholder="选择目录..."
                className="create-workspace-path-input"
              />
              <button
                type="button"
                className="btn btn-secondary"
                onClick={handleSelectDirectory}
              >
                选择目录
              </button>
            </div>
          </div>

          {/* CueLight 项目选择 */}
          <div className="create-workspace-section">
            <CueLightProjectPicker onProjectSelected={handleProjectSelected} />
          </div>

          {error && <p className="cuelight-error">{error}</p>}
        </div>

        <div className="create-workspace-dialog-footer">
          <button type="button" className="btn btn-ghost" onClick={onClose} disabled={creating}>
            取消
          </button>
          <button
            type="button"
            className="btn btn-primary"
            onClick={handleCreate}
            disabled={!canCreate}
          >
            {creating ? (
              <>
                <Loader2 size={14} className="animate-spin" />
                创建中...
              </>
            ) : (
              "创建工作区"
            )}
          </button>
        </div>
      </div>
    </div>,
    document.body,
  );
}

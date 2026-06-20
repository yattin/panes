import { useEffect, useCallback, useState } from "react";
import { createPortal } from "react-dom";
import { X, ZoomIn, ZoomOut } from "lucide-react";

interface MediaPreviewProps {
  url: string;
  type: "image" | "video";
  onClose: () => void;
}

export function MediaPreview({ url, type, onClose }: MediaPreviewProps) {
  const [scale, setScale] = useState(1);

  // Esc 键关闭
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onClose();
      }
    };
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [onClose]);

  // 点击遮罩关闭
  const handleBackdropClick = useCallback((e: React.MouseEvent) => {
    if (e.target === e.currentTarget) {
      onClose();
    }
  }, [onClose]);

  // 缩放控制（仅图片）
  const handleZoomIn = useCallback(() => {
    setScale((s) => Math.min(s + 0.25, 3));
  }, []);

  const handleZoomOut = useCallback(() => {
    setScale((s) => Math.max(s - 0.25, 0.5));
  }, []);

  return createPortal(
    <div className="media-preview-overlay" onClick={handleBackdropClick}>
      {/* 关闭按钮 */}
      <button
        type="button"
        className="media-preview-close"
        onClick={onClose}
        aria-label="关闭"
      >
        <X size={20} />
      </button>

      {/* 缩放控制（仅图片） */}
      {type === "image" && (
        <div className="media-preview-controls">
          <button type="button" onClick={handleZoomOut} disabled={scale <= 0.5}>
            <ZoomOut size={16} />
          </button>
          <span className="media-preview-scale">{Math.round(scale * 100)}%</span>
          <button type="button" onClick={handleZoomIn} disabled={scale >= 3}>
            <ZoomIn size={16} />
          </button>
        </div>
      )}

      {/* 媒体内容 */}
      <div className="media-preview-content" style={{ transform: `scale(${scale})` }}>
        {type === "image" ? (
          <img src={url} alt="预览" className="media-preview-image" />
        ) : (
          <video
            src={url}
            controls
            autoPlay
            className="media-preview-video"
          >
            您的浏览器不支持视频播放
          </video>
        )}
      </div>
    </div>,
    document.body,
  );
}

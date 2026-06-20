import { useState, useEffect, type ReactNode } from "react";
import { KeyRound, Loader2, CheckCircle2, AlertCircle, ExternalLink } from "lucide-react";
import { open } from "@tauri-apps/plugin-shell";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { CUELIGHT_SERVER_URL, getCueLightToken, setCueLightToken } from "../../lib/cueLightConfig";
import { ipc } from "../../lib/ipc";

interface CueLightTokenGateProps {
  children: ReactNode;
}

export function CueLightTokenGate({ children }: CueLightTokenGateProps) {
  const [token, setToken] = useState<string>("");
  const [hasToken, setHasToken] = useState<boolean>(false);
  const [validating, setValidating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const stored = getCueLightToken();
    if (stored) {
      setToken(stored);
      setHasToken(true);
      // 同步到 Rust 后端
      ipc.setCueLightAuthToken(stored).catch(() => {
        // 如果同步失败，忽略错误（后端可能还没完全初始化）
      });
    }
  }, []);

  const handleValidate = async () => {
    if (!token.trim()) return;
    setValidating(true);
    setError(null);

    try {
      // 通过代理调用健康检查验证 Token
      await ipc.cueLightProxy({
        method: "GET",
        serverUrl: CUELIGHT_SERVER_URL,
        path: "/api/projects",
        authToken: token.trim(),
      });
      // 验证成功，保存 Token
      const trimmedToken = token.trim();
      setCueLightToken(trimmedToken);
      await ipc.setCueLightAuthToken(trimmedToken);
      setHasToken(true);
      // 验证成功后最大化窗口
      getCurrentWindow().maximize().catch(() => {});
    } catch (e) {
      setError(e instanceof Error ? e.message : "验证失败，请检查 Token 是否正确");
    } finally {
      setValidating(false);
    }
  };

  // 已有 Token，直接渲染子组件
  if (hasToken) {
    return <>{children}</>;
  }

  // 无 Token，显示配置界面
  return (
    <div className="cuelight-token-gate">
      <div className="cuelight-token-gate-content">
        <div className="cuelight-token-gate-icon">
          <KeyRound size={48} strokeWidth={1.5} />
        </div>
        <h1 className="cuelight-token-gate-title">欢迎使用 CueLight</h1>
        <p className="cuelight-token-gate-subtitle">
          请输入您的 CueLight API Token 以继续
        </p>

        <div className="cuelight-token-gate-form">
          <div className="cuelight-token-gate-input-wrapper">
            <input
              type="password"
              value={token}
              onChange={(e) => setToken(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleValidate()}
              placeholder="cue_xxxxxxxxxxxxxxxxxxxx"
              className="cuelight-token-gate-input"
              autoFocus
            />
          </div>

          <button
            type="button"
            onClick={handleValidate}
            disabled={validating || !token.trim()}
            className="cuelight-token-gate-btn"
          >
            {validating ? (
              <>
                <Loader2 size={16} className="animate-spin" />
                验证中...
              </>
            ) : (
              <>
                <CheckCircle2 size={16} />
                验证并继续
              </>
            )}
          </button>

          {error && (
            <div className="cuelight-token-gate-error">
              <AlertCircle size={14} />
              <span>{error}</span>
            </div>
          )}
        </div>

        <p className="cuelight-token-gate-hint">
          还没有 API Key？
          <button
            type="button"
            className="cuelight-token-gate-link"
            onClick={() => void open(CUELIGHT_SERVER_URL)}
          >
            <ExternalLink size={12} />
            点击这里获取
          </button>
        </p>
      </div>
    </div>
  );
}

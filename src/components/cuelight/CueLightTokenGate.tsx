import { useState, useEffect, type ReactNode } from "react";
import { KeyRound, Loader2, CheckCircle2, AlertCircle, ExternalLink } from "lucide-react";
import { getCueLightGateway } from "../../contexts/cue-light/application/cueLightGateway";

interface CueLightTokenGateProps {
  children: ReactNode;
}

export function CueLightTokenGate({ children }: CueLightTokenGateProps) {
  const [token, setToken] = useState<string>("");
  const [hasToken, setHasToken] = useState<boolean>(false);
  const [validating, setValidating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const gateway = getCueLightGateway();
    const stored = gateway.readToken();
    if (stored) {
      setToken(stored);
      setHasToken(true);
      gateway.syncAuthToken(stored).catch(() => undefined);
    }
  }, []);

  const handleValidate = async () => {
    if (!token.trim()) return;
    setValidating(true);
    setError(null);

    try {
      const gateway = getCueLightGateway();
      const trimmedToken = token.trim();
      await gateway.validateToken(trimmedToken);
      gateway.saveToken(trimmedToken);
      await gateway.syncAuthToken(trimmedToken);
      setHasToken(true);
      gateway.maximizeWindow().catch(() => undefined);
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
            onClick={() => void getCueLightGateway().openServer()}
          >
            <ExternalLink size={12} />
            点击这里获取
          </button>
        </p>
      </div>
    </div>
  );
}

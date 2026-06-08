import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { AccountLoginResult, LoginFlowKind, QqLoginQrResult, QqPollCode, WechatLoginQrResult, WechatPollCode } from "@/components/app/delta-types";
import { LOGIN_FLOW_KINDS, LOGIN_FLOW_KIND_LABELS, LOGIN_FLOW_MODE_MAP } from "@/components/app/delta-types";
import {
  buildAccessTokenInvokeArgs,
  buildLoginQrInvokeArgs,
  buildQqPollInvokeArgs,
  buildWechatPollInvokeArgs,
  extractPollSessionKey,
  extractQqQrImage,
  extractQqQrToken,
} from "@/components/app/delta-login-utils";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Spinner } from "@/components/ui/spinner";

const QQ_POLL_INTERVAL_MS = 2000;
const QQ_POLL_MAX_ATTEMPTS = 60;
const WECHAT_POLL_MAX_ATTEMPTS = 150;

// ── 命令映射表（集中在模块顶层，避免每次渲染重建） ──

const QQ_LOGIN_QR_CMDS: Record<string, string> = {
  qq: "delta_qq_get_login_qr",
  qqsafe: "delta_qqsafe_get_login_qr",
  wegame_qq: "delta_wegame_qq_get_login_qr",
  pioneer: "delta_pioneer_get_login_qr",
};

const WECHAT_LOGIN_QR_CMDS: Record<string, string> = {
  wechat: "delta_wechat_get_login_qr",
  wegame_wechat: "delta_wegame_wechat_get_login_qr",
};

const QQ_POLL_CMDS: Record<string, string> = {
  qq: "delta_qq_poll_login_status",
  qqsafe: "delta_qqsafe_poll_status",
  wegame_qq: "delta_wegame_qq_poll_status",
  pioneer: "delta_pioneer_poll_status",
};

const WECHAT_POLL_CMDS: Record<string, string> = {
  wechat: "delta_wechat_poll_status",
  wegame_wechat: "delta_wegame_wechat_poll_status",
};

const QQ_ACCESS_TOKEN_CMDS: Record<string, string> = {
  qq: "delta_qq_get_access_token",
  qqsafe: "delta_qqsafe_get_access_token",
  wegame_qq: "delta_wegame_qq_get_access_token",
  pioneer: "delta_pioneer_get_access_token",
};

const WECHAT_ACCESS_TOKEN_CMDS: Record<string, string> = {
  wechat: "delta_wechat_get_access_token",
  wegame_wechat: "delta_wegame_wechat_get_access_token",
};

type QqSession = { qrToken: string; qrSig: string; loginSig: string; sessionKey: string };
type AccessTokenPayload = { sessionKey?: string };

// ── 组件 ──

type DeltaLoginDialogProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onLoginSuccess: () => void;
};

type DialogStep = "select_type" | "loading_qr" | "qr_code" | "fetching_token" | "success" | "error";

function isTerminalPollStatus(status: string): boolean {
  return status.includes("过期") || status.includes("拒绝") || status.includes("错误") || status.includes("失败");
}

export function DeltaLoginDialog({ open, onOpenChange, onLoginSuccess }: DeltaLoginDialogProps) {
  const [step, setStep] = useState<DialogStep>("select_type");
  const [flowKind, setFlowKind] = useState<LoginFlowKind | null>(null);
  const [qrImageData, setQrImageData] = useState<string | null>(null);
  const [qrCodeUrl, setQrCodeUrl] = useState<string | null>(null);
  const [pollStatus, setPollStatus] = useState<string>("");
  const [errorMessage, setErrorMessage] = useState<string>("");
  const [qrSession, setQrSession] = useState<QqSession | null>(null);
  const [wechatUuid, setWechatUuid] = useState<string | null>(null);
  const pollTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const pollAttemptsRef = useRef(0);

  // 使用 ref 保存最新值，避免轮询闭包陈旧
  const qrSessionRef = useRef(qrSession);
  const wechatUuidRef = useRef(wechatUuid);
  const flowKindRef = useRef(flowKind);
  useEffect(() => { qrSessionRef.current = qrSession; }, [qrSession]);
  useEffect(() => { wechatUuidRef.current = wechatUuid; }, [wechatUuid]);
  useEffect(() => { flowKindRef.current = flowKind; }, [flowKind]);

  const stopPolling = useCallback(() => {
    if (pollTimerRef.current) {
      clearInterval(pollTimerRef.current);
      pollTimerRef.current = null;
    }
  }, []);

  const resetState = useCallback(() => {
    stopPolling();
    setStep("select_type");
    setFlowKind(null);
    setQrImageData(null);
    setQrCodeUrl(null);
    setPollStatus("");
    setErrorMessage("");
    setQrSession(null);
    setWechatUuid(null);
    pollAttemptsRef.current = 0;
  }, [stopPolling]);

  useEffect(() => {
    if (!open) resetState();
  }, [open, resetState]);

  // ── 获取访问令牌 ──

  const fetchAccessToken = useCallback(async (payload: AccessTokenPayload = {}) => {
    const kind = flowKindRef.current;
    if (!kind) return;
    setStep("fetching_token");

    const mode = LOGIN_FLOW_MODE_MAP[kind];

    try {
      let res: { code: number; msg: string };

      if (mode === "qq" && (payload.sessionKey || qrSessionRef.current)) {
        const cmd = QQ_ACCESS_TOKEN_CMDS[kind];
        if (!cmd) { setStep("error"); setErrorMessage("未知的令牌获取流程"); return; }
        const sessionKey = payload.sessionKey ?? qrSessionRef.current?.sessionKey;
        res = await invoke<{ code: number; msg: string; data: AccountLoginResult }>(
          cmd,
          buildAccessTokenInvokeArgs(kind, sessionKey),
        );
      } else if (mode === "wechat" && payload.sessionKey) {
        const cmd = WECHAT_ACCESS_TOKEN_CMDS[kind];
        if (!cmd) { setStep("error"); setErrorMessage("未知的令牌获取流程"); return; }
        res = await invoke<{ code: number; msg: string; data: AccountLoginResult }>(
          cmd,
          buildAccessTokenInvokeArgs(kind, payload.sessionKey),
        );
      } else {
        setStep("error"); setErrorMessage("缺少令牌获取所需参数"); return;
      }

      if (res.code === 0) {
        setStep("success");
        onLoginSuccess();
        onOpenChange(false);
      } else {
        setStep("error");
        setErrorMessage(res.msg || "获取令牌失败");
      }
    } catch (e) {
      setStep("error");
      setErrorMessage(String(e));
    }
  }, [onLoginSuccess, onOpenChange]);

  // ── 轮询登录状态 ──

  useEffect(() => {
    if (step !== "qr_code" || !flowKind) return;

    const mode = LOGIN_FLOW_MODE_MAP[flowKind];
    pollAttemptsRef.current = 0;
    const maxAttempts = mode === "qq" ? QQ_POLL_MAX_ATTEMPTS : WECHAT_POLL_MAX_ATTEMPTS;

    const poll = async () => {
      if (pollAttemptsRef.current >= maxAttempts) {
        stopPolling();
        setPollStatus("二维码已过期");
        return;
      }
      pollAttemptsRef.current++;

      try {
        if (mode === "qq" && qrSessionRef.current) {
          const session = qrSessionRef.current;
          const cmd = QQ_POLL_CMDS[flowKindRef.current!];
          const res = await invoke<{ code: number; msg: string; data: unknown }>(
            cmd,
            buildQqPollInvokeArgs(flowKindRef.current!, session),
          );
          const code = res.code as QqPollCode;
          if (code === 0) {
            const sessionKey = extractPollSessionKey(res);
            if (!sessionKey) {
              stopPolling();
              setStep("error");
              setErrorMessage("登录成功但未返回会话标识，请重新扫码");
              return;
            }
            stopPolling(); setPollStatus("登录成功");
            await fetchAccessToken({ sessionKey });
          } else if (code === 1) {
            setPollStatus("等待扫描...");
          } else if (code === 2) {
            setPollStatus("已扫描，请在手机上确认");
          } else if (code === -2) {
            stopPolling(); setPollStatus("二维码已过期");
          } else if (code === -3) {
            stopPolling(); setPollStatus("登录被拒绝");
          } else {
            stopPolling(); setPollStatus(`错误: ${res.msg}`);
          }
        } else if (mode === "wechat" && wechatUuidRef.current) {
          const cmd = WECHAT_POLL_CMDS[flowKindRef.current!];
          const res = await invoke<{ code: number; msg: string; data: unknown }>(
            cmd,
            buildWechatPollInvokeArgs(flowKindRef.current!, wechatUuidRef.current),
          );
          const code = res.code as WechatPollCode;
          if (code === 3) {
            const sessionKey = extractPollSessionKey(res);
            if (!sessionKey) {
              stopPolling();
              setStep("error");
              setErrorMessage("登录成功但未返回会话标识，请重新扫码");
              return;
            }
            stopPolling(); setPollStatus("登录成功");
            await fetchAccessToken({ sessionKey });
          } else if (code === 1) {
            setPollStatus("等待扫描...");
          } else if (code === 2) {
            setPollStatus("已扫描，请在手机上确认");
          } else if (code === -2) {
            stopPolling(); setPollStatus("二维码已过期");
          } else if (code === -3) {
            stopPolling(); setPollStatus("登录被拒绝");
          } else {
            stopPolling(); setPollStatus(`错误: ${res.msg}`);
          }
        }
      } catch (e) {
        setPollStatus(`请求失败: ${String(e)}`);
      }
    };

    poll();
    pollTimerRef.current = setInterval(poll, QQ_POLL_INTERVAL_MS);

    return () => { stopPolling(); };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [step, flowKind, stopPolling, fetchAccessToken]);

  // ── 选择登录类型并获取二维码 ──

  const handleSelectKind = useCallback(async (kind: LoginFlowKind) => {
    setFlowKind(kind);
    setStep("loading_qr");
    setErrorMessage("");

    const mode = LOGIN_FLOW_MODE_MAP[kind];

    try {
      if (mode === "qq") {
        const cmd = QQ_LOGIN_QR_CMDS[kind];
        if (!cmd) { setStep("error"); setErrorMessage("未知的 QQ 流程"); return; }

        const res = await invoke<{ code: number; msg: string; data: QqLoginQrResult }>(
          cmd,
          buildLoginQrInvokeArgs(kind),
        );
        if (res.code === 0 && res.data) {
          const image = extractQqQrImage(res.data);
          const qrToken = extractQqQrToken(res.data);
          if (!image || !qrToken) {
            setStep("error");
            setErrorMessage("二维码数据不完整，请重新扫码");
            return;
          }
          setQrImageData(image);
          setQrSession({ qrToken, qrSig: res.data.qrSig, loginSig: res.data.loginSig, sessionKey: res.data.sessionKey });
          setQrCodeUrl(null);
          setStep("qr_code");
        } else {
          setStep("error");
          setErrorMessage(res.msg || "获取二维码失败");
        }
      } else {
        const cmd = WECHAT_LOGIN_QR_CMDS[kind];
        if (!cmd) { setStep("error"); setErrorMessage("未知的微信流程"); return; }

        const res = await invoke<{ code: number; msg: string; data: WechatLoginQrResult }>(
          cmd,
          buildLoginQrInvokeArgs(kind),
        );
        if (res.code === 0 && res.data) {
          setQrCodeUrl(res.data.qrCode);
          setWechatUuid(res.data.uuid);
          setQrImageData(null);
          setQrSession(null);
          setStep("qr_code");
        } else {
          setStep("error");
          setErrorMessage(res.msg || "获取二维码失败");
        }
      }
    } catch (e) {
      setStep("error");
      setErrorMessage(String(e));
    }
  }, []);

  const handleRefreshQr = useCallback(() => {
    if (flowKind) {
      stopPolling();
      setPollStatus("");
      pollAttemptsRef.current = 0;
      handleSelectKind(flowKind);
    }
  }, [flowKind, stopPolling, handleSelectKind]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-xl border-2 border-[var(--ink)] bg-[var(--paper)] p-0">
        <DialogHeader className="border-b-2 border-[var(--ink)] bg-[var(--ink)] px-4 py-3 text-[var(--paper)]">
          <DialogTitle className="uppercase">添加账号</DialogTitle>
          <DialogDescription className="font-mono text-[0.68rem] font-bold tracking-[0.08em] text-[var(--bone)] uppercase">选择账号类型并扫描二维码登录</DialogDescription>
        </DialogHeader>

        <div className="space-y-4 p-4">
          {step === "select_type" && (
            <div className="grid gap-px border-2 border-[var(--ink)] bg-[var(--ink)] sm:grid-cols-2">
              {LOGIN_FLOW_KINDS.map((kind) => (
                <Button
                  key={kind}
                  variant="outline"
                  className="h-auto min-h-24 flex-col items-start gap-2 border-0 bg-[var(--paper)] px-4 py-4 text-left"
                  onClick={() => handleSelectKind(kind)}
                >
                  <span className="font-mono text-[0.62rem] font-black tracking-[0.18em] text-[var(--steel)] uppercase">登录流程</span>
                  <span className="text-sm font-black uppercase">{LOGIN_FLOW_KIND_LABELS[kind]}</span>
                </Button>
              ))}
            </div>
          )}

          {step === "loading_qr" && (
            <div className="flex min-h-56 items-center justify-center gap-2 border-2 border-[var(--ink)] bg-[var(--data-well)] px-4 py-8 font-mono text-xs font-bold tracking-[0.08em] text-[var(--paper)] uppercase">
              <Spinner className="size-4" />
              <span>正在获取二维码...</span>
            </div>
          )}

          {step === "qr_code" && flowKind && (
            <div className="grid gap-4 xl:grid-cols-[14rem_minmax(0,1fr)]">
              <div className="border-2 border-[var(--ink)] bg-[var(--ink)] px-4 py-4 text-[var(--paper)]">
                <Badge variant="outline">{LOGIN_FLOW_KIND_LABELS[flowKind]}</Badge>
                <p className="mt-4 font-mono text-[0.62rem] font-black tracking-[0.18em] text-[var(--bone)] uppercase">扫码状态</p>
                <div className="mt-3 text-sm font-black uppercase">
                  {pollStatus || "等待扫描..."}
                </div>
              </div>

              <div className="flex flex-col items-center gap-4 border-2 border-[var(--ink)] bg-[var(--bone)] px-4 py-4">
                {qrImageData && (
                  <img
                    src={`data:image/png;base64,${qrImageData}`}
                    alt="登录二维码"
                    className="size-52 border-2 border-[var(--ink)] bg-[var(--paper)] p-2"
                  />
                )}

                {qrCodeUrl && !qrImageData && (
                  <img
                    src={qrCodeUrl}
                    alt="微信登录二维码"
                    className="size-52 border-2 border-[var(--ink)] bg-[var(--paper)] p-2"
                  />
                )}

                <div className="flex min-h-10 items-center gap-2 font-mono text-[0.68rem] font-bold tracking-[0.08em] uppercase">
                  {isTerminalPollStatus(pollStatus) ? (
                    <>
                      <span className="text-destructive">{pollStatus}</span>
                      <Button variant="outline" size="sm" onClick={handleRefreshQr}>
                        刷新二维码
                      </Button>
                    </>
                  ) : (
                    <>
                      {pollStatus ? <span className="text-[var(--steel)]">{pollStatus}</span> : null}
                      {!pollStatus && (
                        <div className="flex items-center gap-2 text-[var(--steel)]">
                          <Spinner className="size-3" />
                          <span>等待扫描...</span>
                        </div>
                      )}
                    </>
                  )}
                </div>
              </div>
            </div>
          )}

          {step === "fetching_token" && (
            <div className="flex min-h-56 items-center justify-center gap-2 border-2 border-[var(--ink)] bg-[var(--data-well)] px-4 py-8 font-mono text-xs font-bold tracking-[0.08em] text-[var(--paper)] uppercase">
              <Spinner className="size-4" />
              <span>正在获取访问令牌...</span>
            </div>
          )}

          {step === "success" && (
            <div className="flex min-h-40 items-center justify-center border-2 border-[var(--ink)] bg-[var(--bone)] px-4 py-8 text-center text-sm font-black uppercase text-[var(--ink)]">
              账号添加成功
            </div>
          )}

          {step === "error" && (
            <div className="space-y-4 border-2 border-[var(--alert-red)] bg-[var(--bone)] px-4 py-4">
              <p className="font-mono text-[0.68rem] font-black tracking-[0.08em] text-[var(--alert-red)] uppercase">{errorMessage}</p>
              <div className="flex flex-wrap gap-2">
                <Button variant="outline" size="sm" onClick={() => { resetState(); }}>
                  重新选择
                </Button>
                {flowKind && (
                  <Button variant="default" size="sm" onClick={handleRefreshQr}>
                    重新扫码
                  </Button>
                )}
              </div>
            </div>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}

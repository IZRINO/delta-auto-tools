import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  RiCheckboxCircleLine,
  RiErrorWarningLine,
  RiExternalLinkLine,
  RiRefreshLine,
  RiSettings3Line,
  RiTimeLine,
} from "@remixicon/react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { toast } from "sonner";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  AppPage,
  CardBody,
  ControlTile,
  PageHero,
  SectionHeader,
  SignalTile,
  TacticalCard,
} from "@/components/app/app-ui";
import {
  DEFAULT_STRATEGY_REFRESH_SECONDS,
  STRATEGY_REFRESH_INTERVAL_SECONDS,
  STRATEGY_SITES,
  formatStrategyRefreshLabel,
  injectBaseHrefIntoHtml,
  nextRefreshDelayMs,
  normalizeStrategyRefreshSeconds,
  readStoredRefreshSeconds,
  writeStoredRefreshSeconds,
  type StrategyFetchResponse,
  type StrategyRefreshInterval,
  type StrategySite,
} from "@/components/app/strategy-utils";
import { getErrorMessage } from "@/lib/error-utils";
import { useNativeShell } from "@/hooks/use-native-shell";
import { cn } from "@/lib/utils";

const STORAGE_KEY = "refresh-seconds";
/** 单次代理拉取的失败兜底超时（毫秒）。 */
const FETCH_TIMEOUT_MS = 18_000;

const REFRESH_BUCKET_LABELS: Record<number, string> = {
  30: "30 秒",
  60: "1 分钟",
  120: "2 分钟",
  300: "5 分钟",
  600: "10 分钟",
};

type LoadStatus = "idle" | "loading" | "loaded" | "error";

type SiteRuntime = {
  /** 拉取次数（用于 force-refresh 与 cache-busting） */
  nonce: number;
  /** 当前 srcDoc 内容（已注入 `<base href>`） */
  srcDoc: string | null;
  /** 当前拉取的最终 URL（用于 UI 展示与状态） */
  finalUrl: string | null;
  status: LoadStatus;
  /** 最近一次错误的展示文本 */
  errorMessage: string | null;
  /** 当前正在倒计时的剩余毫秒数；null 表示已关闭 */
  countdownMs: number | null;
  /** 最近一次成功拉取的时间戳（毫秒） */
  lastLoadedAt: number | null;
};

function buildInitialRuntimes(): Record<string, SiteRuntime> {
  const runtime: Record<string, SiteRuntime> = {};
  for (const site of STRATEGY_SITES) {
    runtime[site.id] = {
      nonce: 0,
      srcDoc: null,
      finalUrl: null,
      status: "idle",
      errorMessage: null,
      countdownMs: null,
      lastLoadedAt: null,
    };
  }
  return runtime;
}

function withTimeout<T>(promise: Promise<T>, ms: number): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timer = window.setTimeout(() => reject(new Error(`请求超过 ${Math.round(ms / 1000)} 秒未响应`)), ms);
    promise.then(
      (value) => {
        window.clearTimeout(timer);
        resolve(value);
      },
      (error) => {
        window.clearTimeout(timer);
        reject(error);
      },
    );
  });
}

export function StrategyPage() {
  const isNativeShell = useNativeShell();
  const firstSiteId = STRATEGY_SITES[0]?.id ?? "kkrb";

  return (
    <AppPage>
      <PageHero
        eyebrow="Big-Category Utility"
        title="攻略网站工作台"
        description="通过桌面代理拉取攻略页面，模拟完整 Chrome 浏览器请求头，避开 WebView UA 引发的人机验证。支持按站点自动刷新、立即刷新与外部打开。"
        badges={
          <>
            <Badge variant="secondary">大类工具</Badge>
            <Badge variant="outline">Chrome UA 代理</Badge>
          </>
        }
        stats={
          <>
            <SignalTile
              label="已集成站点"
              value={STRATEGY_SITES.length}
              icon={<RiCheckboxCircleLine />}
              detail="按站点切换 Tab，全屏查看。"
            />
            <SignalTile
              label="默认刷新"
              value={formatStrategyRefreshLabel(DEFAULT_STRATEGY_REFRESH_SECONDS)}
              icon={<RiTimeLine />}
              detail="每个站点可独立覆盖。"
            />
            <SignalTile
              label="运行模式"
              value={isNativeShell ? "桌面代理" : "浏览器预览"}
              icon={<RiSettings3Line />}
              detail="桌面端走 Rust 端 fetch 代理，预览模式只读。"
            />
          </>
        }
      />

      <TacticalCard>
        <Tabs defaultValue={firstSiteId} className="min-h-0">
          <CardBody className="flex flex-col gap-4">
            <TabsList variant="line" className="self-start">
              {STRATEGY_SITES.map((site) => (
                <TabsTrigger key={site.id} value={site.id}>
                  <img alt="" aria-hidden className="size-4 rounded-sm" src={site.favicon} />
                  <span>{site.label}</span>
                </TabsTrigger>
              ))}
            </TabsList>

            {STRATEGY_SITES.map((site) => (
              <TabsContent key={site.id} value={site.id} className="flex flex-col gap-4">
                <StrategySitePanel site={site} isNativeShell={isNativeShell} />
              </TabsContent>
            ))}
          </CardBody>
        </Tabs>
      </TacticalCard>
    </AppPage>
  );
}

type StrategySitePanelProps = {
  site: StrategySite;
  isNativeShell: boolean;
};

function StrategySitePanel({ site, isNativeShell }: StrategySitePanelProps) {
  const storageKey = `${site.id}:${STORAGE_KEY}`;
  const [intervalSeconds, setIntervalSecondsState] = useState<StrategyRefreshInterval>(() =>
    readStoredRefreshSeconds(storageKey),
  );
  const [runtime, setRuntime] = useState<SiteRuntime>(
    () => buildInitialRuntimes()[site.id] ?? {
      nonce: 0,
      srcDoc: null,
      finalUrl: null,
      status: "idle",
      errorMessage: null,
      countdownMs: null,
      lastLoadedAt: null,
    },
  );
  const autoRefreshRef = useRef<number | null>(null);
  const countdownTickRef = useRef<number | null>(null);
  const mountedRef = useRef(true);

  const updateInterval = useCallback(
    (next: StrategyRefreshInterval) => {
      const normalized = normalizeStrategyRefreshSeconds(next);
      setIntervalSecondsState(normalized);
      writeStoredRefreshSeconds(storageKey, normalized);
    },
    [storageKey],
  );

  const fetchPage = useCallback(
    async (mode: "auto" | "manual" = "auto") => {
      if (!isNativeShell) {
        setRuntime((current) => ({
          ...current,
          status: "error",
          errorMessage: "浏览器预览模式下无法调用代理，请在桌面端打开。",
        }));
        return;
      }
      setRuntime((current) => ({
        ...current,
        status: "loading",
        errorMessage: null,
        countdownMs: intervalSeconds === null ? null : nextRefreshDelayMs(intervalSeconds),
      }));
      try {
        const response = await withTimeout(
          invoke<StrategyFetchResponse>("strategy_fetch_page", {
            request: { url: site.url },
          }),
          FETCH_TIMEOUT_MS,
        );
        if (!mountedRef.current) {
          return;
        }
        if (response.status >= 400) {
          throw new Error(`HTTP ${response.status}：目标站返回错误。`);
        }
        const srcDoc = injectBaseHrefIntoHtml(response.html, response.finalUrl);
        setRuntime((current) => ({
          ...current,
          srcDoc,
          finalUrl: response.finalUrl,
          status: "loaded",
          errorMessage: null,
          nonce: current.nonce + 1,
          lastLoadedAt: Date.now(),
          countdownMs: intervalSeconds === null ? null : nextRefreshDelayMs(intervalSeconds),
        }));
        if (mode === "manual") {
          toast.success(`${site.shortLabel} 已刷新（${(response.byteLength / 1024).toFixed(1)} KB）`);
        }
      } catch (error) {
        if (!mountedRef.current) {
          return;
        }
        const message = getErrorMessage(error);
        setRuntime((current) => ({
          ...current,
          status: "error",
          errorMessage: message,
        }));
        if (mode === "manual") {
          toast.error(`${site.shortLabel} 刷新失败：${message}`);
        }
      }
    },
    [isNativeShell, intervalSeconds, site.shortLabel, site.url],
  );

  // 首次进入面板时主动拉取一次。
  useEffect(() => {
    mountedRef.current = true;
    if (isNativeShell && runtime.status === "idle" && runtime.srcDoc === null) {
      void fetchPage("auto");
    }
    return () => {
      mountedRef.current = false;
    };
    // 仅在面板挂载 + 切到非 idle 时拉取；显式排除 runtime / fetchPage 避免循环。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isNativeShell, site.id]);

  // 自动刷新主循环：到点后调用 fetchPage("auto")，并重置倒计时。
  useEffect(() => {
    if (autoRefreshRef.current !== null) {
      window.clearInterval(autoRefreshRef.current);
      autoRefreshRef.current = null;
    }
    if (intervalSeconds === null) {
      setRuntime((current) =>
        current.countdownMs === null ? current : { ...current, countdownMs: null },
      );
      return;
    }

    const delay = nextRefreshDelayMs(intervalSeconds) ?? 0;
    setRuntime((current) => ({ ...current, countdownMs: delay }));

    autoRefreshRef.current = window.setInterval(() => {
      void fetchPage("auto");
    }, delay);

    return () => {
      if (autoRefreshRef.current !== null) {
        window.clearInterval(autoRefreshRef.current);
        autoRefreshRef.current = null;
      }
    };
  }, [intervalSeconds, fetchPage]);

  // 倒计时：每秒递减，仅在自动刷新启用时运行。
  useEffect(() => {
    if (countdownTickRef.current !== null) {
      window.clearInterval(countdownTickRef.current);
      countdownTickRef.current = null;
    }
    if (intervalSeconds === null || runtime.status === "error") {
      return;
    }
    countdownTickRef.current = window.setInterval(() => {
      setRuntime((current) => {
        if (current.countdownMs === null || current.status === "error") {
          return current;
        }
        const nextMs = current.countdownMs - 1000;
        return {
          ...current,
          countdownMs: nextMs <= 0 ? nextRefreshDelayMs(intervalSeconds) : nextMs,
        };
      });
    }, 1000);
    return () => {
      if (countdownTickRef.current !== null) {
        window.clearInterval(countdownTickRef.current);
        countdownTickRef.current = null;
      }
    };
  }, [intervalSeconds, runtime.status]);

  const triggerRefresh = useCallback(() => {
    void fetchPage("manual");
  }, [fetchPage]);

  const handleOpenExternal = useCallback(async () => {
    if (isNativeShell) {
      try {
        await openUrl(site.externalUrl);
        return;
      } catch (error) {
        toast.error(`外部打开失败：${error instanceof Error ? error.message : String(error)}`);
        return;
      }
    }
    window.open(site.externalUrl, "_blank", "noopener,noreferrer");
  }, [isNativeShell, site.externalUrl]);

  const countdownText = useMemo(() => {
    if (intervalSeconds === null) {
      return "自动刷新已关闭";
    }
    if (runtime.status === "error") {
      return "代理拉取失败，请用\"立即刷新\"重试，或点\"浏览器打开\"查看。";
    }
    if (runtime.status === "loading" && runtime.countdownMs === null) {
      return "正在拉取代理响应…";
    }
    if (runtime.countdownMs === null) {
      return "准备中…";
    }
    const totalSeconds = Math.max(1, Math.round(runtime.countdownMs / 1000));
    return `下次刷新 ${totalSeconds} 秒后`;
  }, [intervalSeconds, runtime.countdownMs, runtime.status]);

  const statusBadge = (() => {
    switch (runtime.status) {
      case "loading":
        return <Badge variant="outline">拉取中</Badge>;
      case "loaded":
        return <Badge variant="secondary">已加载</Badge>;
      case "error":
        return <Badge variant="destructive">拉取失败</Badge>;
      default:
        return <Badge variant="outline">待加载</Badge>;
    }
  })();

  const lastLoadedLabel = runtime.lastLoadedAt
    ? new Date(runtime.lastLoadedAt).toLocaleTimeString("zh-CN", { hour12: false })
    : "—";

  return (
    <TacticalCard size="sm" className="flex flex-col gap-4">
      <SectionHeader
        eyebrow={`Station · ${site.shortLabel}`}
        icon={<img alt="" aria-hidden className="size-5 rounded-sm" src={site.favicon} />}
        title={site.label}
        description={site.description}
        badge={statusBadge}
      />

      <CardBody className="flex flex-col gap-4">
        <ControlTile>
          <FieldGroup className="grid gap-3 lg:grid-cols-[1fr_auto_auto_auto] lg:items-end">
            <Field>
              <FieldLabel>自动刷新设置</FieldLabel>
              <FieldContent>
                <Select
                  value={intervalSeconds === null ? "off" : String(intervalSeconds)}
                  onValueChange={(value) => {
                    if (value === "off") {
                      updateInterval(null);
                      return;
                    }
                    const parsed = Number.parseInt(value, 10);
                    updateInterval(normalizeStrategyRefreshSeconds(parsed));
                  }}
                >
                  <SelectTrigger>
                    <SelectValue placeholder="选择刷新间隔" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="off">{formatStrategyRefreshLabel(null)}</SelectItem>
                    {STRATEGY_REFRESH_INTERVAL_SECONDS.map((seconds) => (
                      <SelectItem key={seconds} value={String(seconds)}>
                        {REFRESH_BUCKET_LABELS[seconds] ?? formatStrategyRefreshLabel(seconds)}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
                <FieldDescription>{countdownText}</FieldDescription>
              </FieldContent>
            </Field>
            <Field>
              <FieldLabel>立即刷新</FieldLabel>
              <FieldContent>
                <Button
                  type="button"
                  variant="outline"
                  onClick={triggerRefresh}
                  disabled={runtime.status === "loading"}
                >
                  <RiRefreshLine data-icon="inline-start" />
                  立即刷新
                </Button>
              </FieldContent>
            </Field>
            <Field>
              <FieldLabel>外部打开</FieldLabel>
              <FieldContent>
                <Button type="button" variant="secondary" onClick={handleOpenExternal}>
                  <RiExternalLinkLine data-icon="inline-start" />
                  浏览器打开
                </Button>
              </FieldContent>
            </Field>
            <Field>
              <FieldLabel>最近一次拉取</FieldLabel>
              <FieldContent>
                <div className="flex h-9 items-center rounded-md border border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-tile),color-mix(in_oklch,var(--card)_38%,transparent))] px-3 font-mono text-xs text-muted-foreground">
                  {lastLoadedLabel}
                </div>
              </FieldContent>
            </Field>
          </FieldGroup>
        </ControlTile>

        {runtime.status === "error" ? (
          <Alert variant="destructive">
            <RiErrorWarningLine />
            <AlertTitle>{site.shortLabel} 代理拉取失败</AlertTitle>
            <AlertDescription>
              {runtime.errorMessage ?? "未知错误。"}建议点击"浏览器打开"在系统浏览器中查看，刷新频率与人机验证问题可在真实浏览器里完成。
            </AlertDescription>
          </Alert>
        ) : null}

        <StrategyFrame status={runtime.status} srcDoc={runtime.srcDoc} site={site} />
      </CardBody>
    </TacticalCard>
  );
}

type StrategyFrameProps = {
  status: LoadStatus;
  srcDoc: string | null;
  site: StrategySite;
};

function StrategyFrame({ status, srcDoc, site }: StrategyFrameProps) {
  if (status === "error") {
    // 已由 Alert 区域展示错误信息，iframe 留白以避免视觉噪声。
    return (
      <div
        className={cn(
          "flex min-h-72 flex-col items-center justify-center gap-2 rounded-lg border border-dashed border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-tile),color-mix(in_oklch,var(--card)_36%,transparent))] px-6 py-10 text-center text-sm text-muted-foreground",
        )}
      >
        <p className="font-medium text-foreground">未能拉取 {site.shortLabel} 内容</p>
        <p className="max-w-md text-xs/relaxed">请点上方"立即刷新"重试，或用"浏览器打开"在系统浏览器中查看。</p>
      </div>
    );
  }

  if (status === "loading" || srcDoc === null) {
    return (
      <div
        className={cn(
          "flex min-h-72 flex-col items-center justify-center gap-2 rounded-lg border border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-tile),color-mix(in_oklch,var(--card)_36%,transparent))] px-6 py-10 text-center text-sm text-muted-foreground",
        )}
      >
        <RiTimeLine className="size-5 animate-pulse text-primary" />
        <p className="font-medium text-foreground">正在通过桌面代理拉取 {site.shortLabel}…</p>
        <p className="max-w-md text-xs/relaxed">代理使用 Chrome 135 User-Agent + 完整 Sec-Ch-Ua / Sec-Fetch-* 请求头，避开 WebView UA 引发的人机验证。</p>
      </div>
    );
  }

  return (
    <iframe
      title={site.label}
      srcDoc={srcDoc}
      sandbox="allow-scripts allow-forms allow-popups allow-popups-to-escape-sandbox allow-same-origin"
      referrerPolicy="no-referrer-when-downgrade"
      className="block h-[64vh] min-h-[480px] w-full rounded-lg border border-[var(--surface-border)] bg-background"
    />
  );
}

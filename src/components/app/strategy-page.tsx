import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import {
  RiCheckboxCircleLine,
  RiExternalLinkLine,
  RiRefreshLine,
  RiSettings3Line,
  RiTimeLine,
  RiWifiOffLine,
} from "@remixicon/react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { toast } from "sonner";

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
  nextRefreshDelayMs,
  normalizeStrategyRefreshSeconds,
  readStoredRefreshSeconds,
  writeStoredRefreshSeconds,
  type StrategyRefreshInterval,
  type StrategySite,
} from "@/components/app/strategy-utils";
import { useNativeShell } from "@/hooks/use-native-shell";
import { cn } from "@/lib/utils";

const STORAGE_KEY = "refresh-seconds";
const IFRAME_LOAD_TIMEOUT_MS = 12_000;

const REFRESH_BUCKET_LABELS: Record<number, string> = {
  30: "30 秒",
  60: "1 分钟",
  120: "2 分钟",
  300: "5 分钟",
  600: "10 分钟",
};

type SiteRuntime = {
  /** iframe `src` 每次刷新时累加，绕过同源缓存 */
  nonce: number;
  /** 是否已成功加载（onLoad 触发且未超时） */
  loaded: boolean;
  /** 是否在加载超时后被标记为"无法内嵌" */
  blocked: boolean;
  /** 当前正在倒计时的剩余毫秒数；null 表示已关闭 */
  countdownMs: number | null;
};

function buildInitialRuntimes(): Record<string, SiteRuntime> {
  const runtime: Record<string, SiteRuntime> = {};
  for (const site of STRATEGY_SITES) {
    runtime[site.id] = {
      nonce: 0,
      loaded: false,
      blocked: false,
      countdownMs: null,
    };
  }
  return runtime;
}

export function StrategyPage() {
  const isNativeShell = useNativeShell();

  return (
    <AppPage>
      <PageHero
        eyebrow="Big-Category Utility"
        title="攻略网站工作台"
        description="把常用攻略站点整合到一个面板，可独立设置自动刷新间隔，支持一键外部打开与内嵌回退。"
        badges={
          <>
            <Badge variant="secondary">大类工具</Badge>
            <Badge variant="outline">Web / Iframe</Badge>
          </>
        }
        stats={
          <>
            <SignalTile
              label="已集成站点"
              value={STRATEGY_SITES.length}
              icon={<RiCheckboxCircleLine />}
              detail="按需求添加 KOL 攻略入口。"
            />
            <SignalTile
              label="默认刷新"
              value={formatStrategyRefreshLabel(DEFAULT_STRATEGY_REFRESH_SECONDS)}
              icon={<RiTimeLine />}
              detail="每张卡片可独立覆盖。"
            />
            <SignalTile
              label="运行模式"
              value={isNativeShell ? "桌面" : "浏览器预览"}
              icon={<RiSettings3Line />}
              detail="非 Tauri 模式下仍可访问页面与按钮。"
            />
          </>
        }
      />

      <TacticalCard>
        <CardBody className="grid gap-5 xl:grid-cols-2">
          {STRATEGY_SITES.map((site, index) => (
            <StrategySiteCard
              key={site.id}
              site={site}
              index={index}
              isNativeShell={isNativeShell}
            />
          ))}
        </CardBody>
      </TacticalCard>
    </AppPage>
  );
}

type StrategySiteCardProps = {
  site: StrategySite;
  index: number;
  isNativeShell: boolean;
};

function StrategySiteCard({ site, index, isNativeShell }: StrategySiteCardProps) {
  const storageKey = `${site.id}:${STORAGE_KEY}`;
  const [intervalSeconds, setIntervalSecondsState] = useState<StrategyRefreshInterval>(() =>
    readStoredRefreshSeconds(storageKey),
  );
  const [runtime, setRuntime] = useState<SiteRuntime>(
    () => buildInitialRuntimes()[site.id] ?? { nonce: 0, loaded: false, blocked: false, countdownMs: null },
  );
  const autoRefreshRef = useRef<number | null>(null);
  const countdownTickRef = useRef<number | null>(null);
  const loadWatchdogRef = useRef<number | null>(null);

  const updateInterval = useCallback(
    (next: StrategyRefreshInterval) => {
      const normalized = normalizeStrategyRefreshSeconds(next);
      setIntervalSecondsState(normalized);
      writeStoredRefreshSeconds(storageKey, normalized);
    },
    [storageKey],
  );

  // 倒计时：每秒递减，仅在自动刷新启用时运行。
  useEffect(() => {
    if (countdownTickRef.current !== null) {
      window.clearInterval(countdownTickRef.current);
      countdownTickRef.current = null;
    }
    if (intervalSeconds === null) {
      return;
    }
    countdownTickRef.current = window.setInterval(() => {
      setRuntime((current) => {
        if (current.countdownMs === null || current.blocked) {
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
  }, [intervalSeconds]);

  // 自动刷新主循环：每次到期时递增 nonce 强制刷新 iframe，并重置倒计时。
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
      setRuntime((current) => {
        if (current.blocked) {
          return { ...current, countdownMs: null };
        }
        return {
          ...current,
          nonce: current.nonce + 1,
          loaded: false,
          blocked: false,
          countdownMs: nextRefreshDelayMs(intervalSeconds),
        };
      });
    }, delay);

    return () => {
      if (autoRefreshRef.current !== null) {
        window.clearInterval(autoRefreshRef.current);
        autoRefreshRef.current = null;
      }
    };
  }, [intervalSeconds]);

  // 加载看门狗：若 iframe 超过 IFRAME_LOAD_TIMEOUT_MS 仍未 onLoad，标记为 blocked。
  useEffect(() => {
    if (runtime.loaded || runtime.blocked) {
      return;
    }
    if (loadWatchdogRef.current !== null) {
      window.clearTimeout(loadWatchdogRef.current);
    }
    loadWatchdogRef.current = window.setTimeout(() => {
      setRuntime((current) => (current.loaded ? current : { ...current, blocked: true, countdownMs: null }));
    }, IFRAME_LOAD_TIMEOUT_MS);
    return () => {
      if (loadWatchdogRef.current !== null) {
        window.clearTimeout(loadWatchdogRef.current);
        loadWatchdogRef.current = null;
      }
    };
  }, [runtime.nonce, runtime.loaded, runtime.blocked]);

  const triggerRefresh = useCallback(() => {
    if (loadWatchdogRef.current !== null) {
      window.clearTimeout(loadWatchdogRef.current);
      loadWatchdogRef.current = null;
    }
    setRuntime((current) => ({
      ...current,
      nonce: current.nonce + 1,
      loaded: false,
      blocked: false,
      countdownMs: intervalSeconds === null ? null : nextRefreshDelayMs(intervalSeconds),
    }));
  }, [intervalSeconds]);

  const handleIframeLoad = useCallback(() => {
    if (loadWatchdogRef.current !== null) {
      window.clearTimeout(loadWatchdogRef.current);
      loadWatchdogRef.current = null;
    }
    setRuntime((current) => ({ ...current, loaded: true, blocked: false }));
  }, []);

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
    // 浏览器预览模式：直接用 window.open，避开 Tauri 桥。
    window.open(site.externalUrl, "_blank", "noopener,noreferrer");
  }, [isNativeShell, site.externalUrl]);

  const countdownText = useMemo(() => {
    if (intervalSeconds === null) {
      return "自动刷新已关闭";
    }
    if (runtime.blocked) {
      return "站点拒绝内嵌，已暂停自动刷新";
    }
    if (runtime.countdownMs === null) {
      return "准备中…";
    }
    const totalSeconds = Math.max(1, Math.round(runtime.countdownMs / 1000));
    return `下次刷新 ${totalSeconds} 秒后`;
  }, [intervalSeconds, runtime.blocked, runtime.countdownMs]);

  const iframeSrc = `${site.url}${site.url.includes("?") ? "&" : "?"}_t=${runtime.nonce}`;
  const statusBadge = (() => {
    if (runtime.blocked) {
      return <Badge variant="destructive">拒绝内嵌</Badge>;
    }
    if (runtime.loaded) {
      return <Badge variant="secondary">已加载</Badge>;
    }
    return <Badge variant="outline">加载中</Badge>;
  })();
  return (
    <TacticalCard className="flex flex-col gap-3" size="sm">
      <SectionHeader
        eyebrow={`Station 0${index + 1}`}
        icon={<img alt="" className="size-5 rounded-sm" src={site.favicon} />}
        title={site.label}
        description={site.description}
        badge={statusBadge}
      />
      <CardBody className="flex flex-col gap-4">
        <ControlTile>
          <FieldGroup className="grid gap-3 sm:grid-cols-[1fr_auto_auto] sm:items-end">
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
              <FieldLabel>手动刷新</FieldLabel>
              <FieldContent>
                <Button
                  type="button"
                  variant="outline"
                  onClick={triggerRefresh}
                  disabled={runtime.blocked}
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
          </FieldGroup>
        </ControlTile>

        <StrategySiteFrame
          site={site}
          src={iframeSrc}
          blocked={runtime.blocked}
          loaded={runtime.loaded}
          onLoad={handleIframeLoad}
          onOpenExternal={handleOpenExternal}
        />
      </CardBody>
    </TacticalCard>
  );
}

type StrategySiteFrameProps = {
  site: StrategySite;
  src: string;
  blocked: boolean;
  loaded: boolean;
  onLoad: () => void;
  onOpenExternal: () => void;
};

function StrategySiteFrame({ site, src, blocked, loaded, onLoad, onOpenExternal }: StrategySiteFrameProps) {
  if (blocked) {
    return (
      <div
        className={cn(
          "flex min-h-72 flex-col items-center justify-center gap-3 rounded-lg border border-dashed border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-tile),color-mix(in_oklch,var(--card)_36%,transparent))] px-6 py-10 text-center text-sm text-muted-foreground",
        )}
      >
        <RiWifiOffLine className="size-6 text-primary" />
        <p className="font-medium text-foreground">{site.shortLabel} 拒绝被内嵌展示</p>
        <p className="max-w-md text-xs/relaxed">
          站点设置了 X-Frame-Options / CSP frame-ancestors。请使用"浏览器打开"按钮在系统浏览器查看，自动刷新已暂停。
        </p>
        <Button type="button" variant="secondary" onClick={onOpenExternal}>
          <RiExternalLinkLine data-icon="inline-start" />
          在系统浏览器打开
        </Button>
      </div>
    );
  }
  return (
    <div className="overflow-hidden rounded-lg border border-[var(--surface-border)] bg-background">
      <iframe
        title={site.label}
        src={src}
        onLoad={onLoad}
        loading="lazy"
        referrerPolicy="no-referrer-when-downgrade"
        sandbox="allow-scripts allow-same-origin allow-forms allow-popups allow-popups-to-escape-sandbox"
        className="block h-[28rem] w-full bg-background"
      />
      {!loaded ? (
        <div className="flex items-center gap-2 border-t border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-tile),color-mix(in_oklch,var(--card)_30%,transparent))] px-3 py-2 text-xs text-muted-foreground">
          <RiTimeLine className="size-3.5" />
          正在加载 {site.shortLabel}…若长时间无内容，请改用"浏览器打开"。
        </div>
      ) : null}
    </div>
  );
}

export type { StrategySite };
export type StrategyPageNode = ReactNode;

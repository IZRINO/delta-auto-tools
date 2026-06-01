import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  RiAddLine,
  RiCheckboxCircleLine,
  RiDeleteBinLine,
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
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
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
  BUILTIN_STRATEGY_SITES,
  DEFAULT_STRATEGY_REFRESH_SECONDS,
  STRATEGY_REFRESH_INTERVAL_SECONDS,
  createStrategySite,
  formatStrategyRefreshLabel,
  injectBaseHrefIntoHtml,
  mergeStrategySites,
  nextRefreshDelayMs,
  normalizeStrategyRefreshSeconds,
  readStoredRefreshSeconds,
  readStoredUserSites,
  writeStoredRefreshSeconds,
  writeStoredUserSites,
  type StrategyFetchResponse,
  type StrategyRefreshInterval,
  type StrategySite,
  type UserStrategySite,
} from "@/components/app/strategy-utils";
import { getErrorMessage } from "@/lib/error-utils";
import { useNativeShell } from "@/hooks/use-native-shell";
import { cn } from "@/lib/utils";

const STORAGE_KEY = "refresh-seconds";
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
  nonce: number;
  srcDoc: string | null;
  finalUrl: string | null;
  status: LoadStatus;
  errorMessage: string | null;
  countdownMs: number | null;
  lastLoadedAt: number | null;
};

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
  const [userSites, setUserSites] = useState<StrategySite[]>(() => readStoredUserSites());
  const allSites = useMemo(() => mergeStrategySites(BUILTIN_STRATEGY_SITES, userSites), [userSites]);
  const [activeId, setActiveId] = useState<string>(() => BUILTIN_STRATEGY_SITES[0]?.id ?? allSites[0]?.id ?? "");

  // 当用户删除了 activeId 对应的卡片，自动回退到第一个可用 Tab。
  useEffect(() => {
    if (allSites.length === 0) {
      return;
    }
    if (!allSites.some((site) => site.id === activeId)) {
      setActiveId(allSites[0].id);
    }
  }, [allSites, activeId]);

  const handleAddSite = useCallback((draft: UserStrategySite) => {
    const created = createStrategySite(draft);
    if (!created) {
      toast.error("网址无效：检查简称、标签与 URL 格式（必须以 http:// 或 https:// 开头）");
      return false;
    }
    setUserSites((current) => {
      const next = [...current, created];
      writeStoredUserSites(next);
      return next;
    });
    setActiveId(created.id);
    toast.success(`已新增攻略网站：${created.label}`);
    return true;
  }, []);

  const handleDeleteSite = useCallback((id: string) => {
    setUserSites((current) => {
      const target = current.find((site) => site.id === id);
      if (!target) {
        return current;
      }
      const next = current.filter((site) => site.id !== id);
      writeStoredUserSites(next);
      toast.success(`已删除攻略网站：${target.label}`);
      return next;
    });
  }, []);

  return (
    <AppPage>
      <PageHero
        eyebrow="Big-Category Utility"
        title="攻略网站工作台"
        description="通过桌面代理拉取攻略页面，模拟完整 Chrome 浏览器请求头，避开 WebView UA 引发的人机验证。支持按站点自动刷新、立即刷新、外部打开，以及自定义新增/删除攻略网站。"
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
              value={allSites.length}
              icon={<RiCheckboxCircleLine />}
              detail={`${BUILTIN_STRATEGY_SITES.length} 个内置 + ${userSites.length} 个自定义`}
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
              detail="桌面端走 Rust 端 fetch 代理。"
            />
          </>
        }
      />

      <TacticalCard>
        <Tabs value={activeId} onValueChange={setActiveId} className="min-h-0">
          <CardBody className="flex flex-col gap-4">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <TabsList variant="line" className="self-start">
                {allSites.map((site) => (
                  <TabsTrigger key={site.id} value={site.id}>
                    <img alt="" aria-hidden className="size-4 rounded-sm" src={site.favicon} />
                    <span>{site.label}</span>
                  </TabsTrigger>
                ))}
              </TabsList>
              <NewSiteDialog onSubmit={handleAddSite} />
            </div>

            {allSites.length === 0 ? (
              <div className="flex min-h-72 flex-col items-center justify-center gap-3 rounded-lg border border-dashed border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-tile),color-mix(in_oklch,var(--card)_36%,transparent))] px-6 py-10 text-center text-sm text-muted-foreground">
                <p className="font-medium text-foreground">还没有任何攻略网站</p>
                <p className="max-w-md text-xs/relaxed">点上方"新增攻略网站"按钮，填入 URL 后即可加入工作台。</p>
              </div>
            ) : (
              allSites.map((site) => (
                <TabsContent key={site.id} value={site.id} className="flex flex-col gap-4">
                  <StrategySitePanel
                    site={site}
                    isNativeShell={isNativeShell}
                    onDelete={site.builtin ? null : () => handleDeleteSite(site.id)}
                  />
                </TabsContent>
              ))
            )}
          </CardBody>
        </Tabs>
      </TacticalCard>
    </AppPage>
  );
}

type NewSiteDialogProps = {
  onSubmit: (draft: UserStrategySite) => boolean;
};

function NewSiteDialog({ onSubmit }: NewSiteDialogProps) {
  const [open, setOpen] = useState(false);
  const [shortLabel, setShortLabel] = useState("");
  const [label, setLabel] = useState("");
  const [url, setUrl] = useState("");
  const [description, setDescription] = useState("");

  const reset = useCallback(() => {
    setShortLabel("");
    setLabel("");
    setUrl("");
    setDescription("");
  }, []);

  const handleSubmit = useCallback(
    (event: React.FormEvent<HTMLFormElement>) => {
      event.preventDefault();
      const ok = onSubmit({ shortLabel, label, url, description });
      if (ok) {
        reset();
        setOpen(false);
      }
    },
    [description, label, onSubmit, reset, shortLabel, url],
  );

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button type="button" variant="default" size="sm">
          <RiAddLine data-icon="inline-start" />
          新增攻略网站
        </Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>新增攻略网站</DialogTitle>
          <DialogDescription>填入网址后，会在 Tabs 列表最右侧追加一个 Tab。</DialogDescription>
        </DialogHeader>
        <form className="grid gap-3" onSubmit={handleSubmit}>
          <FieldGroup className="grid gap-3 sm:grid-cols-2">
            <Field>
              <FieldLabel htmlFor="strategy-new-short">简称</FieldLabel>
              <FieldContent>
                <Input
                  id="strategy-new-short"
                  value={shortLabel}
                  maxLength={12}
                  placeholder="例如：KK 日报"
                  onChange={(event) => setShortLabel(event.target.value)}
                  required
                />
                <FieldDescription>2-6 个字符，显示在 Tab 上。</FieldDescription>
              </FieldContent>
            </Field>
            <Field>
              <FieldLabel htmlFor="strategy-new-label">完整标签</FieldLabel>
              <FieldContent>
                <Input
                  id="strategy-new-label"
                  value={label}
                  maxLength={32}
                  placeholder="例如：KK 日报攻略总览"
                  onChange={(event) => setLabel(event.target.value)}
                  required
                />
              </FieldContent>
            </Field>
            <Field className="sm:col-span-2">
              <FieldLabel htmlFor="strategy-new-url">URL</FieldLabel>
              <FieldContent>
                <Input
                  id="strategy-new-url"
                  value={url}
                  type="url"
                  placeholder="https://..."
                  onChange={(event) => setUrl(event.target.value)}
                  required
                />
                <FieldDescription>必须以 http:// 或 https:// 开头。</FieldDescription>
              </FieldContent>
            </Field>
            <Field className="sm:col-span-2">
              <FieldLabel htmlFor="strategy-new-description">简介</FieldLabel>
              <FieldContent>
                <Input
                  id="strategy-new-description"
                  value={description}
                  maxLength={64}
                  placeholder="一句话说明这个站点做什么"
                  onChange={(event) => setDescription(event.target.value)}
                />
              </FieldContent>
            </Field>
          </FieldGroup>
          <DialogFooter className="gap-2">
            <DialogClose asChild>
              <Button type="button" variant="ghost">取消</Button>
            </DialogClose>
            <Button type="submit" variant="default">新增</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

type StrategySitePanelProps = {
  site: StrategySite;
  isNativeShell: boolean;
  onDelete: (() => void) | null;
};

function StrategySitePanel({ site, isNativeShell, onDelete }: StrategySitePanelProps) {
  const storageKey = `${site.id}:${STORAGE_KEY}`;
  const [intervalSeconds, setIntervalSecondsState] = useState<StrategyRefreshInterval>(() =>
    readStoredRefreshSeconds(storageKey),
  );
  const [runtime, setRuntime] = useState<SiteRuntime>({
    nonce: 0,
    srcDoc: null,
    finalUrl: null,
    status: "idle",
    errorMessage: null,
    countdownMs: null,
    lastLoadedAt: null,
  });
  const autoRefreshRef = useRef<number | null>(null);
  const countdownTickRef = useRef<number | null>(null);
  const mountedRef = useRef(true);
  const siteRef = useRef(site);
  siteRef.current = site;

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
      const current = siteRef.current;
      if (!isNativeShell) {
        setRuntime((prev) => ({
          ...prev,
          status: "error",
          errorMessage: "浏览器预览模式下无法调用代理，请在桌面端打开。",
        }));
        return;
      }
      setRuntime((prev) => ({
        ...prev,
        status: "loading",
        errorMessage: null,
        countdownMs: intervalSeconds === null ? null : nextRefreshDelayMs(intervalSeconds),
      }));
      try {
        const response = await withTimeout(
          invoke<StrategyFetchResponse>("strategy_fetch_page", {
            request: { url: current.url },
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
        setRuntime((prev) => ({
          ...prev,
          srcDoc,
          finalUrl: response.finalUrl,
          status: "loaded",
          errorMessage: null,
          nonce: prev.nonce + 1,
          lastLoadedAt: Date.now(),
          countdownMs: intervalSeconds === null ? null : nextRefreshDelayMs(intervalSeconds),
        }));
        if (mode === "manual") {
          toast.success(`${current.shortLabel} 已刷新（${(response.byteLength / 1024).toFixed(1)} KB）`);
        }
      } catch (error) {
        if (!mountedRef.current) {
          return;
        }
        const message = getErrorMessage(error);
        setRuntime((prev) => ({
          ...prev,
          status: "error",
          errorMessage: message,
        }));
        if (mode === "manual") {
          toast.error(`${current.shortLabel} 刷新失败：${message}`);
        }
      }
    },
    [isNativeShell, intervalSeconds],
  );

  useEffect(() => {
    mountedRef.current = true;
    if (isNativeShell && runtime.status === "idle" && runtime.srcDoc === null) {
      void fetchPage("auto");
    }
    return () => {
      mountedRef.current = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isNativeShell, site.id]);

  useEffect(() => {
    if (autoRefreshRef.current !== null) {
      window.clearInterval(autoRefreshRef.current);
      autoRefreshRef.current = null;
    }
    if (intervalSeconds === null) {
      setRuntime((prev) => (prev.countdownMs === null ? prev : { ...prev, countdownMs: null }));
      return;
    }

    const delay = nextRefreshDelayMs(intervalSeconds) ?? 0;
    setRuntime((prev) => ({ ...prev, countdownMs: delay }));

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
    const current = siteRef.current;
    if (isNativeShell) {
      try {
        await openUrl(current.externalUrl);
        return;
      } catch (error) {
      }
    }
    window.open(current.externalUrl, "_blank", "noopener,noreferrer");
  }, [isNativeShell]);

  const handleDelete = useCallback(() => {
    if (!onDelete) {
      return;
    }
    onDelete();
  }, [onDelete]);

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
        actions={
          onDelete ? (
            <Button type="button" variant="ghost" size="sm" onClick={handleDelete}>
              <RiDeleteBinLine data-icon="inline-start" />
              删除此网站
            </Button>
          ) : null
        }
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
              {runtime.errorMessage ?? "未知错误。"}建议点击"浏览器打开"在系统浏览器中查看；如果是 JS 重定向循环，Rust 端已自动跟随 `document.cookie + location.href` 模式（最多 3 次），若仍失败说明站点升级了风控策略。
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
        <p className="max-w-md text-xs/relaxed">
          代理使用 Chrome 135 User-Agent + 完整 Sec-Ch-Ua / Sec-Fetch-* 请求头，避开 WebView UA 引发的人机验证；
          若站点是 JS 重定向（document.cookie + location.href）模式，Rust 端会自动跟随。
        </p>
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

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  RiAddLine,
  RiDeleteBinLine,
  RiExternalLinkLine,
  RiRefreshLine,
} from "@remixicon/react";
import { PhysicalPosition, PhysicalSize } from "@tauri-apps/api/dpi";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { openUrl } from "@tauri-apps/plugin-opener";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import {
  Field,
  FieldDescription,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Textarea } from "@/components/ui/textarea";
import { AppPage, SurfaceToggleGroup } from "@/components/app/app-ui";
import {
  BUILTIN_STRATEGY_SITES,
  createStrategySite,
  DEFAULT_STRATEGY_REFRESH_SECONDS,
  STRATEGY_REFRESH_OPTIONS,
  STRATEGY_CONTENT_MIN_HEIGHT,
  STRATEGY_CONTENT_MIN_WIDTH,
  mergeStrategySites,
  normalizeStrategyContentBounds,
  normalizeVisibleStrategyContentBounds,
  readStoredUserSites,
  readStrategyRefreshSeconds,
  writeStoredUserSites,
  writeStrategyRefreshSeconds,
  type StrategyContentBounds,
  type StrategyRefreshSeconds,
  type StrategySite,
} from "@/components/app/strategy-utils";
import { getErrorMessage } from "@/lib/error-utils";
import { useNativeShell } from "@/hooks/use-native-shell";

const CONTENT_WEBVIEW_LABEL = "strategy-content";
const STABLE_BOUNDS_ATTEMPTS = 30;
const EMPTY_USER_SITE_FORM = {
  shortLabel: "",
  label: "",
  url: "",
  description: "",
};


type StrategyContentWindowBounds = {
  x: number;
  y: number;
  width: number;
  height: number;
};

async function closeContentWindow(candidate: WebviewWindow | null): Promise<void> {
  const contentWindow = candidate ?? (await WebviewWindow.getByLabel(CONTENT_WEBVIEW_LABEL).catch(() => null));
  if (!contentWindow) {
    return;
  }
  await contentWindow.destroy().catch(() => undefined);
}

export function StrategyPage() {
  const isNativeShell = useNativeShell();
  const contentHostRef = useRef<HTMLDivElement | null>(null);
  const contentWindowRef = useRef<WebviewWindow | null>(null);
  const contentWindowReadyRef = useRef(false);
  const latestBoundsRef = useRef<StrategyContentBounds | null>(null);
  const [userSites, setUserSites] = useState<StrategySite[]>(() => readStoredUserSites());
  const allSites = useMemo(() => mergeStrategySites(BUILTIN_STRATEGY_SITES, userSites), [userSites]);
  const [activeId, setActiveId] = useState<string>(() => BUILTIN_STRATEGY_SITES[0]?.id ?? "");
  const activeSite = useMemo(() => {
    const matched = allSites.find((site) => site.id === activeId);
    return matched ?? allSites[0] ?? null;
  }, [activeId, allSites]);
  const activeUrl = activeSite?.url ?? "";
  const activeLabel = activeSite?.label ?? "未选择站点";
  const [reloadNonce, setReloadNonce] = useState(0);
  const [statusMessage, setStatusMessage] = useState("正在准备内部网页区域...");
  const [refreshSeconds, setRefreshSeconds] = useState<StrategyRefreshSeconds>(() =>
    readStrategyRefreshSeconds(BUILTIN_STRATEGY_SITES[0]?.id ?? ""),
  );
  const [remainingSeconds, setRemainingSeconds] = useState(0);
  const [createDialogOpen, setCreateDialogOpen] = useState(false);
  const [siteForm, setSiteForm] = useState(EMPTY_USER_SITE_FORM);
  const [siteFormError, setSiteFormError] = useState<string | null>(null);

  useEffect(() => {
    if (allSites.length === 0) {
      return;
    }
    if (!allSites.some((site) => site.id === activeId)) {
      setActiveId(allSites[0].id);
    }
  }, [allSites, activeId]);

  useEffect(() => {
    setRefreshSeconds(readStrategyRefreshSeconds(activeSite?.id ?? ""));
    setRemainingSeconds(0);
  }, [activeSite?.id]);

  const calculateContentBounds = useCallback((): StrategyContentBounds => {
    const rect = contentHostRef.current?.getBoundingClientRect();
    return normalizeStrategyContentBounds(rect);
  }, []);

  const calculateVisibleContentBounds = useCallback((): StrategyContentBounds | null => {
    const rect = contentHostRef.current?.getBoundingClientRect();
    return normalizeVisibleStrategyContentBounds(rect, {
      width: window.innerWidth,
      height: window.innerHeight,
    });
  }, []);

  const hasUsefulContentBounds = useCallback((bounds: StrategyContentBounds) => {
    return bounds.width > STRATEGY_CONTENT_MIN_WIDTH && bounds.height >= STRATEGY_CONTENT_MIN_HEIGHT;
  }, []);

  const waitForStableContentBounds = useCallback(async (): Promise<StrategyContentBounds> => {
    let last = calculateContentBounds();
    let lastUseful = hasUsefulContentBounds(last) ? last : null;
    for (let attempt = 0; attempt < STABLE_BOUNDS_ATTEMPTS; attempt += 1) {
      await new Promise<void>((resolve) => window.requestAnimationFrame(() => resolve()));
      const next = calculateContentBounds();
      if (hasUsefulContentBounds(next)) {
        lastUseful = next;
      }
      const hostHasUsefulSize =
        contentHostRef.current !== null &&
        next.width > STRATEGY_CONTENT_MIN_WIDTH &&
        next.height >= STRATEGY_CONTENT_MIN_HEIGHT;
      const isStable =
        hostHasUsefulSize &&
        next.x === last.x &&
        next.y === last.y &&
        next.width === last.width &&
        next.height === last.height;
      if (isStable) {
        return next;
      }
      last = next;
    }
    return lastUseful ?? calculateContentBounds();
  }, [calculateContentBounds, hasUsefulContentBounds]);

  const calculateContentWindowBounds = useCallback(async (bounds: StrategyContentBounds): Promise<StrategyContentWindowBounds> => {
    const appWindow = getCurrentWindow();
    const [innerPosition, scaleFactor] = await Promise.all([
      appWindow.innerPosition(),
      appWindow.scaleFactor(),
    ]);
    return {
      x: Math.round(innerPosition.x + bounds.x * scaleFactor),
      y: Math.round(innerPosition.y + bounds.y * scaleFactor),
      width: Math.max(1, Math.round(bounds.width * scaleFactor)),
      height: Math.max(1, Math.round(bounds.height * scaleFactor)),
    };
  }, []);

  const applyContentBounds = useCallback(async (contentWindow: WebviewWindow, bounds: StrategyContentBounds) => {
    const windowBounds = await calculateContentWindowBounds(bounds);
    await contentWindow.setPosition(new PhysicalPosition(windowBounds.x, windowBounds.y));
    await contentWindow.setSize(new PhysicalSize(windowBounds.width, windowBounds.height));
  }, [calculateContentWindowBounds]);

  const resizeContentWebview = useCallback(async () => {
    if (!isNativeShell) {
      return;
    }
    const bounds = calculateVisibleContentBounds();
    latestBoundsRef.current = bounds;
    const contentWindow = contentWindowRef.current;
    if (!contentWindow || !contentWindowReadyRef.current) {
      return;
    }
    if (!bounds) {
      await contentWindow.hide();
      return;
    }
    await applyContentBounds(contentWindow, bounds);
    await contentWindow.show();
  }, [applyContentBounds, calculateVisibleContentBounds, isNativeShell]);

  useEffect(() => {
    if (!isNativeShell || !activeUrl) {
      setStatusMessage(isNativeShell ? "未选择攻略网站。" : "浏览器预览模式无法创建 Tauri 内容窗口。");
      return;
    }

    let cancelled = false;
    const currentWindow = getCurrentWindow();
    contentWindowReadyRef.current = false;
    setStatusMessage(`正在加载 ${activeLabel}...`);

    async function mountWebview() {
      const existing = await WebviewWindow.getByLabel(CONTENT_WEBVIEW_LABEL).catch(() => null);
      await closeContentWindow(existing);
      if (cancelled) {
        return;
      }

      const hostBounds = await waitForStableContentBounds();
      const visibleBounds = calculateVisibleContentBounds();
      const initialBounds = visibleBounds ?? hostBounds;
      latestBoundsRef.current = visibleBounds;
      if (cancelled) {
        return;
      }

      const contentWindow = new WebviewWindow(CONTENT_WEBVIEW_LABEL, {
        url: activeUrl,
        width: initialBounds.width,
        height: initialBounds.height,
        decorations: false,
        focus: false,
        parent: currentWindow,
        resizable: false,
        shadow: false,
        skipTaskbar: true,
        visible: false,
        title: activeLabel,
      });
      contentWindowRef.current = contentWindow;
      void contentWindow.once("tauri://created", () => {
        if (cancelled || contentWindowRef.current !== contentWindow) {
          void closeContentWindow(contentWindow);
          return;
        }
        contentWindowReadyRef.current = true;
        const nextBounds = latestBoundsRef.current ?? calculateVisibleContentBounds();
        if (!nextBounds) {
          void contentWindow.hide();
          setStatusMessage("网页区域当前不在可视范围内。");
          return;
        }
        void applyContentBounds(contentWindow, nextBounds)
          .then(() => contentWindow.show())
          .then(() => {
            setStatusMessage(`已在主窗口内部加载：${activeLabel}`);
          })
          .catch((error: unknown) => {
            setStatusMessage(`调整网页视图位置失败：${getErrorMessage(error)}`);
          });
      });
      void contentWindow.once("tauri://error", (event) => {
        if (contentWindowRef.current === contentWindow) {
          contentWindowRef.current = null;
        }
        contentWindowReadyRef.current = false;
        setStatusMessage(`创建网页视图失败：${String(event.payload)}`);
      });
    }

    void mountWebview().catch((error: unknown) => {
      setStatusMessage(`创建网页视图失败：${getErrorMessage(error)}`);
    });

    return () => {
      cancelled = true;
      const current = contentWindowRef.current;
      contentWindowRef.current = null;
      contentWindowReadyRef.current = false;
      void closeContentWindow(current);
    };
  }, [activeLabel, activeUrl, applyContentBounds, calculateVisibleContentBounds, isNativeShell, reloadNonce, waitForStableContentBounds]);

  useEffect(() => {
    if (!isNativeShell) {
      return;
    }
    let frame = 0;
    let disposed = false;
    let windowUnlisteners: Array<() => void> = [];
    const scheduleResize = () => {
      window.cancelAnimationFrame(frame);
      frame = window.requestAnimationFrame(() => {
        void resizeContentWebview().catch((error: unknown) => {
          setStatusMessage(`调整网页视图尺寸失败：${getErrorMessage(error)}`);
        });
      });
    };
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(scheduleResize);
    if (contentHostRef.current && observer) {
      observer.observe(contentHostRef.current);
    }
    scheduleResize();
    window.addEventListener("resize", scheduleResize);
    window.addEventListener("scroll", scheduleResize, true);
    async function setupWindowListeners() {
      const appWindow = getCurrentWindow();
      const unlisteners = await Promise.all([
        appWindow.onMoved(scheduleResize),
        appWindow.onResized(scheduleResize),
        appWindow.onScaleChanged(scheduleResize),
      ]);
      if (disposed) {
        unlisteners.forEach((unlisten) => unlisten());
        return;
      }
      windowUnlisteners = unlisteners;
    }
    void setupWindowListeners().catch((error: unknown) => {
      setStatusMessage(`监听主窗口位置失败：${getErrorMessage(error)}`);
    });
    return () => {
      disposed = true;
      window.cancelAnimationFrame(frame);
      observer?.disconnect();
      window.removeEventListener("resize", scheduleResize);
      window.removeEventListener("scroll", scheduleResize, true);
      windowUnlisteners.forEach((unlisten) => unlisten());
    };
  }, [isNativeShell, resizeContentWebview]);

  useEffect(() => {
    if (!isNativeShell || !activeUrl || refreshSeconds === DEFAULT_STRATEGY_REFRESH_SECONDS) {
      setRemainingSeconds(0);
      return;
    }

    const dueAt = Date.now() + refreshSeconds * 1000;
    setRemainingSeconds(refreshSeconds);
    const interval = window.setInterval(() => {
      setRemainingSeconds(Math.max(0, Math.ceil((dueAt - Date.now()) / 1000)));
    }, 1000);
    const timeout = window.setTimeout(() => {
      setRemainingSeconds(0);
      setReloadNonce((current) => current + 1);
    }, refreshSeconds * 1000);

    return () => {
      window.clearInterval(interval);
      window.clearTimeout(timeout);
    };
  }, [activeUrl, isNativeShell, refreshSeconds, reloadNonce]);

  const handleRefresh = useCallback(() => {
    if (!activeUrl) {
      return;
    }
    setReloadNonce((current) => current + 1);
    toast.success(`已刷新当前站点：${activeLabel}`);
  }, [activeLabel, activeUrl]);

  const handleRefreshSecondsChange = useCallback((value: string) => {
    const nextSeconds = Number(value) as StrategyRefreshSeconds;
    const siteId = activeSite?.id ?? "";
    writeStrategyRefreshSeconds(siteId, nextSeconds);
    const normalized = readStrategyRefreshSeconds(siteId);
    setRefreshSeconds(normalized);
    setRemainingSeconds(normalized);
    toast.success(normalized === DEFAULT_STRATEGY_REFRESH_SECONDS ? "已关闭自动刷新。" : `已设置 ${activeLabel} 自动刷新档位。`);
  }, [activeLabel, activeSite?.id]);

  const handleOpenExternal = useCallback(async () => {
    if (!activeUrl) {
      return;
    }
    if (isNativeShell) {
      try {
        await openUrl(activeUrl);
        return;
      } catch {
        // 落到 window.open 兜底。
      }
    }
    window.open(activeUrl, "_blank", "noopener,noreferrer");
  }, [activeUrl, isNativeShell]);


  const handleCreateSite = useCallback(() => {
    const nextSite = createStrategySite(siteForm);
    if (!nextSite) {
      setSiteFormError("请填写简称、完整名称和 http:// 或 https:// 开头的 URL。");
      return;
    }
    const nextUserSites = [...userSites, nextSite];
    setUserSites(nextUserSites);
    writeStoredUserSites(nextUserSites);
    setActiveId(nextSite.id);
    setSiteForm(EMPTY_USER_SITE_FORM);
    setSiteFormError(null);
    setCreateDialogOpen(false);
    toast.success(`已新增攻略网站：${nextSite.label}`);
  }, [siteForm, userSites]);

  const handleDeleteActiveSite = useCallback(() => {
    if (!activeSite || activeSite.builtin) {
      return;
    }
    const nextUserSites = userSites.filter((site) => site.id !== activeSite.id);
    const nextAllSites = mergeStrategySites(BUILTIN_STRATEGY_SITES, nextUserSites);
    setUserSites(nextUserSites);
    writeStoredUserSites(nextUserSites);
    setActiveId(nextAllSites[0]?.id ?? "");
    toast.success(`已删除自定义攻略网站：${activeSite.label}`);
  }, [activeSite, userSites]);
  const activeRefreshOption = STRATEGY_REFRESH_OPTIONS.find((option) => option.seconds === refreshSeconds) ?? STRATEGY_REFRESH_OPTIONS[0];
  const refreshLabel = refreshSeconds > 0 ? `${activeRefreshOption.label} · ${remainingSeconds || refreshSeconds}s` : activeRefreshOption.label;

  return (
    <AppPage className="min-h-[calc(100dvh-4rem)] flex-1 gap-3 overflow-hidden">
      <div className="relative z-10 shrink-0 rounded-md border border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-card-strong),color-mix(in_srgb,var(--secondary)_32%,transparent))] px-3 py-2 shadow-[var(--shadow-card)]">
        <div className="flex flex-col gap-2 xl:flex-row xl:items-center">
          <div className="flex min-w-0 flex-1 items-center gap-2">
            <Badge variant="secondary" className="hidden shrink-0 sm:inline-flex">攻略</Badge>
            <Tabs value={activeSite?.id ?? activeId} onValueChange={setActiveId} className="min-w-0 flex-1">
              <div className="overflow-x-auto overflow-y-hidden pb-1">
                <SurfaceToggleGroup className="min-w-max">
                  <TabsList variant="line" className="min-w-max justify-start bg-transparent p-0">
                    {allSites.map((site) => (
                      <TabsTrigger key={site.id} value={site.id} className="max-w-36 shrink-0 font-mono text-[0.72rem] tracking-[0.08em]">
                        <img alt="" aria-hidden className="size-4 rounded-sm" src={site.favicon} />
                        <span className="truncate">{site.shortLabel || site.label}</span>
                      </TabsTrigger>
                    ))}
                  </TabsList>
                </SurfaceToggleGroup>
              </div>
            </Tabs>
          </div>

          <div className="flex min-w-0 flex-wrap items-center gap-2 xl:justify-end">
            <Tooltip>
              <TooltipTrigger asChild>
                <div className="min-w-0 max-w-full flex-1 truncate rounded-md border border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-tile),color-mix(in_srgb,var(--card)_48%,transparent))] px-3 py-2 font-mono text-[0.68rem] text-muted-foreground shadow-[var(--shadow-tile)] xl:max-w-80 xl:flex-none">
                  {activeUrl || "未选择站点"}
                </div>
              </TooltipTrigger>
              <TooltipContent side="bottom" align="start" className="max-w-md">
                <p className="font-mono text-xs break-all">{activeUrl || "未选择站点"}</p>
                {activeSite?.description ? <p className="mt-1 text-xs text-muted-foreground">{activeSite.description}</p> : null}
              </TooltipContent>
            </Tooltip>

            <Dialog open={createDialogOpen} onOpenChange={(open) => { setCreateDialogOpen(open); if (!open) setSiteFormError(null); }}>
              <DialogTrigger asChild>
                <Button type="button" size="sm" variant="outline">
                  <RiAddLine data-icon="inline-start" />
                  新增
                </Button>
              </DialogTrigger>
              <DialogContent>
                <DialogHeader>
                  <DialogTitle>新增攻略网站</DialogTitle>
                  <DialogDescription>添加后会保存到本机 localStorage，并立即切换到新站点。</DialogDescription>
                </DialogHeader>
                <FieldGroup className="gap-3">
                  <Field>
                    <FieldLabel htmlFor="strategy-site-short-label">简称</FieldLabel>
                    <Input id="strategy-site-short-label" value={siteForm.shortLabel} onChange={(event) => setSiteForm((current) => ({ ...current, shortLabel: event.currentTarget.value }))} placeholder="例如 攻略站" />
                  </Field>
                  <Field>
                    <FieldLabel htmlFor="strategy-site-label">完整名称</FieldLabel>
                    <Input id="strategy-site-label" value={siteForm.label} onChange={(event) => setSiteForm((current) => ({ ...current, label: event.currentTarget.value }))} placeholder="例如 自定义攻略站" />
                  </Field>
                  <Field>
                    <FieldLabel htmlFor="strategy-site-url">URL</FieldLabel>
                    <Input id="strategy-site-url" value={siteForm.url} onChange={(event) => setSiteForm((current) => ({ ...current, url: event.currentTarget.value }))} placeholder="https://example.com" />
                  </Field>
                  <Field>
                    <FieldLabel htmlFor="strategy-site-description">简介（可选）</FieldLabel>
                    <Textarea id="strategy-site-description" value={siteForm.description} onChange={(event) => setSiteForm((current) => ({ ...current, description: event.currentTarget.value }))} placeholder="用于在站点信息中展示" />
                  </Field>
                  {siteFormError ? <FieldDescription className="text-destructive">{siteFormError}</FieldDescription> : null}
                </FieldGroup>
                <DialogFooter>
                  <Button type="button" onClick={handleCreateSite}>
                    <RiAddLine data-icon="inline-start" />
                    创建站点
                  </Button>
                </DialogFooter>
              </DialogContent>
            </Dialog>

            {activeSite && !activeSite.builtin ? (
              <Button type="button" size="sm" variant="ghost" onClick={handleDeleteActiveSite}>
                <RiDeleteBinLine data-icon="inline-start" />
                删除
              </Button>
            ) : null}

            <Select value={String(refreshSeconds)} onValueChange={handleRefreshSecondsChange} disabled={!activeSite}>
              <SelectTrigger size="sm" className="w-36">
                <SelectValue placeholder="自动刷新">{refreshLabel}</SelectValue>
              </SelectTrigger>
              <SelectContent>
                {STRATEGY_REFRESH_OPTIONS.map((option) => (
                  <SelectItem key={option.seconds} value={String(option.seconds)}>
                    {option.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>

            <Button type="button" size="sm" variant="default" onClick={handleRefresh} disabled={!isNativeShell || !activeUrl}>
              <RiRefreshLine data-icon="inline-start" />
              刷新
            </Button>
            <Button type="button" size="sm" variant="secondary" onClick={handleOpenExternal} disabled={!activeUrl}>
              <RiExternalLinkLine data-icon="inline-start" />
              系统浏览器
            </Button>
          </div>
        </div>
      </div>

      <div
        ref={contentHostRef}
        className="relative z-0 min-h-0 flex-1 overflow-hidden rounded-md border border-[var(--surface-border-strong)] bg-[linear-gradient(145deg,var(--surface-tile),color-mix(in_srgb,var(--card)_38%,transparent))] shadow-[var(--shadow-card)]"
      >
        <div className="pointer-events-none absolute inset-0 flex items-center justify-center px-6 text-center text-sm text-muted-foreground">
          <div className="max-w-md rounded-md border border-[var(--surface-border)] bg-background/90 px-5 py-4 shadow-[var(--shadow-tile)]">
            <p className="font-medium text-foreground">{isNativeShell ? statusMessage : "该工具需要在桌面端使用"}</p>
            <p className="mt-2 text-xs/relaxed">
              {isNativeShell
                ? "网页内容会贴合此定位宿主区域；切换工具页时会自动关闭 strategy-content。"
                : "浏览器预览模式无法创建 Tauri 内容窗口，请在桌面端使用。"}
            </p>
          </div>
        </div>
      </div>
    </AppPage>
  );
}

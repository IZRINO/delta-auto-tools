import { useCallback, useEffect, useMemo, useRef, useState, type FormEvent } from "react";
import {
  RiAddLine,
  RiDeleteBinLine,
  RiExternalLinkLine,
  RiRefreshLine,
  RiWindowLine,
} from "@remixicon/react";
import { LogicalPosition, LogicalSize } from "@tauri-apps/api/dpi";
import { Webview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { openUrl } from "@tauri-apps/plugin-opener";
import { toast } from "sonner";

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
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  AppPage,
  CardBody,
  PageHero,
  SectionHeader,
  SignalTile,
  TacticalCard,
} from "@/components/app/app-ui";
import {
  BUILTIN_STRATEGY_SITES,
  DEFAULT_STRATEGY_REFRESH_SECONDS,
  STRATEGY_REFRESH_OPTIONS,
  createStrategySite,
  mergeStrategySites,
  readStoredUserSites,
  readStrategyRefreshSeconds,
  writeStoredUserSites,
  writeStrategyRefreshSeconds,
  type StrategyRefreshSeconds,
  type StrategySite,
  type UserStrategySite,
} from "@/components/app/strategy-utils";
import { getErrorMessage } from "@/lib/error-utils";
import { useNativeShell } from "@/hooks/use-native-shell";

const CONTENT_WEBVIEW_LABEL = "strategy-content";
const MIN_CONTENT_WIDTH = 320;
const MIN_CONTENT_HEIGHT = 360;

type ContentBounds = {
  x: number;
  y: number;
  width: number;
  height: number;
};

async function closeContentWebview(candidate: Webview | null): Promise<void> {
  const webview = candidate ?? await Webview.getByLabel(CONTENT_WEBVIEW_LABEL).catch(() => null);
  if (!webview) {
    return;
  }
  await webview.close().catch(() => undefined);
}

export function StrategyPage() {
  const isNativeShell = useNativeShell();
  const contentHostRef = useRef<HTMLDivElement | null>(null);
  const webviewRef = useRef<Webview | null>(null);
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

  const calculateContentBounds = useCallback((): ContentBounds => {
    const rect = contentHostRef.current?.getBoundingClientRect();
    return {
      x: Math.max(0, Math.round(rect?.left ?? 0)),
      y: Math.max(0, Math.round(rect?.top ?? 0)),
      width: Math.max(MIN_CONTENT_WIDTH, Math.round(rect?.width ?? MIN_CONTENT_WIDTH)),
      height: Math.max(MIN_CONTENT_HEIGHT, Math.round(rect?.height ?? MIN_CONTENT_HEIGHT)),
    };
  }, []);

  const resizeContentWebview = useCallback(async () => {
    if (!isNativeShell) {
      return;
    }
    const webview = webviewRef.current ?? await Webview.getByLabel(CONTENT_WEBVIEW_LABEL).catch(() => null);
    if (!webview) {
      return;
    }
    const bounds = calculateContentBounds();
    await webview.setPosition(new LogicalPosition(bounds.x, bounds.y));
    await webview.setSize(new LogicalSize(bounds.width, bounds.height));
  }, [calculateContentBounds, isNativeShell]);

  useEffect(() => {
    if (!isNativeShell || !activeUrl) {
      setStatusMessage(isNativeShell ? "未选择攻略网站。" : "浏览器预览模式无法创建 Tauri 子 WebView。");
      return;
    }

    let cancelled = false;
    const currentWindow = getCurrentWindow();
    const bounds = calculateContentBounds();
    setStatusMessage(`正在加载 ${activeLabel}...`);

    async function mountWebview() {
      const existing = await Webview.getByLabel(CONTENT_WEBVIEW_LABEL).catch(() => null);
      await closeContentWebview(existing);
      if (cancelled) {
        return;
      }

      const webview = new Webview(currentWindow, CONTENT_WEBVIEW_LABEL, {
        url: activeUrl,
        x: bounds.x,
        y: bounds.y,
        width: bounds.width,
        height: bounds.height,
        focus: false,
      });
      webviewRef.current = webview;
      void webview.once("tauri://created", () => {
        setStatusMessage(`已在主窗口内部加载：${activeLabel}`);
      });
      void webview.once("tauri://error", (event) => {
        setStatusMessage(`创建网页视图失败：${String(event.payload)}`);
      });
    }

    void mountWebview().catch((error: unknown) => {
      setStatusMessage(`创建网页视图失败：${getErrorMessage(error)}`);
    });

    return () => {
      cancelled = true;
      const current = webviewRef.current;
      webviewRef.current = null;
      void closeContentWebview(current);
    };
  }, [activeLabel, activeUrl, calculateContentBounds, isNativeShell, reloadNonce]);

  useEffect(() => {
    if (!isNativeShell) {
      return;
    }
    let frame = 0;
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
    return () => {
      window.cancelAnimationFrame(frame);
      observer?.disconnect();
      window.removeEventListener("resize", scheduleResize);
      window.removeEventListener("scroll", scheduleResize, true);
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

  const handleDeleteActiveSite = useCallback(() => {
    if (!activeSite || activeSite.builtin) {
      return;
    }
    setUserSites((current) => {
      const target = current.find((site) => site.id === activeSite.id);
      if (!target) {
        return current;
      }
      const next = current.filter((site) => site.id !== activeSite.id);
      writeStoredUserSites(next);
      toast.success(`已删除攻略网站：${target.label}`);
      return next;
    });
  }, [activeSite]);

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

  const activeRefreshOption = STRATEGY_REFRESH_OPTIONS.find((option) => option.seconds === refreshSeconds) ?? STRATEGY_REFRESH_OPTIONS[0];

  return (
    <AppPage className="min-h-[calc(100svh-4rem)]">
      <PageHero
        eyebrow="Big-Category Utility"
        title="攻略网站工作台"
        description="在主软件内部嵌入 WebView2 真实网页区域，站点 cookie、JS 跳转、localStorage、同源接口和人机验证都由目标站点自身处理，不再额外弹出攻略浏览器窗口。"
        badges={
          <>
            <Badge variant="secondary">大类工具</Badge>
            <Badge variant="outline">主窗口内嵌 WebView2</Badge>
          </>
        }
        stats={
          <>
            <SignalTile
              label="已集成站点"
              value={allSites.length}
              icon={<RiWindowLine />}
              detail={`${BUILTIN_STRATEGY_SITES.length} 个内置 + ${userSites.length} 个自定义`}
            />
            <SignalTile
              label="打开方式"
              value="软件内部"
              icon={<RiWindowLine />}
              detail="当前页面的 strategy-content 子 WebView 承载真实导航。"
            />
            <SignalTile
              label="自动刷新"
              value={activeRefreshOption.label}
              icon={<RiRefreshLine />}
              detail={refreshSeconds > 0 ? `${remainingSeconds || refreshSeconds} 秒后刷新当前站点。` : "关闭时只响应手动刷新。"}
            />
          </>
        }
      />

      <TacticalCard className="relative z-10 shrink-0">
        <CardBody className="flex flex-col gap-4">
          <div className="flex flex-wrap items-start justify-between gap-3">
            <SectionHeader
              eyebrow="Strategy Station"
              icon={activeSite ? <img alt="" aria-hidden className="size-5 rounded-sm" src={activeSite.favicon} /> : <RiWindowLine />}
              title={activeLabel}
              description={activeSite?.description || "选择一个攻略站点后，下方网页区域会在主窗口内部真实加载。"}
              badge={<Badge variant={isNativeShell ? "default" : "secondary"}>{isNativeShell ? "桌面 WebView2" : "预览不可用"}</Badge>}
              actions={
                <div className="flex flex-wrap items-center gap-2">
                  <NewSiteDialog onSubmit={handleAddSite} />
                  {activeSite && !activeSite.builtin ? (
                    <Button type="button" variant="ghost" size="sm" onClick={handleDeleteActiveSite}>
                      <RiDeleteBinLine data-icon="inline-start" />
                      删除此网站
                    </Button>
                  ) : null}
                </div>
              }
            />
          </div>

          <div className="flex flex-col gap-3">
            <Tabs value={activeSite?.id ?? activeId} onValueChange={setActiveId}>
              <TabsList variant="line" className="flex-wrap justify-start">
                {allSites.map((site) => (
                  <TabsTrigger key={site.id} value={site.id}>
                    <img alt="" aria-hidden className="size-4 rounded-sm" src={site.favicon} />
                    <span>{site.label}</span>
                  </TabsTrigger>
                ))}
              </TabsList>
            </Tabs>

            <FieldGroup className="grid gap-3 xl:grid-cols-[minmax(0,1fr)_auto_auto_auto] xl:items-end">
              <Field>
                <FieldLabel>当前 URL</FieldLabel>
                <FieldContent>
                  <div className="flex h-9 items-center truncate rounded-md border border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-tile),color-mix(in_oklch,var(--card)_38%,transparent))] px-3 font-mono text-xs text-muted-foreground">
                    {activeUrl || "未选择站点"}
                  </div>
                  <FieldDescription>
                    网页直接导航到目标 URL；如果站点需要真人确认，请在下方网页区域内完成。
                  </FieldDescription>
                </FieldContent>
              </Field>

              <Field className="min-w-48">
                <FieldLabel>自动刷新档位</FieldLabel>
                <FieldContent>
                  <Select value={String(refreshSeconds)} onValueChange={handleRefreshSecondsChange} disabled={!activeSite}>
                    <SelectTrigger>
                      <SelectValue placeholder="选择刷新档位" />
                    </SelectTrigger>
                    <SelectContent>
                      {STRATEGY_REFRESH_OPTIONS.map((option) => (
                        <SelectItem key={option.seconds} value={String(option.seconds)}>
                          {option.label}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                  <FieldDescription>
                    {refreshSeconds > 0 ? `剩余 ${remainingSeconds || refreshSeconds} 秒刷新当前站点。` : "关闭自动刷新。"}
                  </FieldDescription>
                </FieldContent>
              </Field>

              <Button type="button" variant="default" onClick={handleRefresh} disabled={!isNativeShell || !activeUrl}>
                <RiRefreshLine data-icon="inline-start" />
                手动刷新
              </Button>
              <Button type="button" variant="secondary" onClick={handleOpenExternal} disabled={!activeUrl}>
                <RiExternalLinkLine data-icon="inline-start" />
                系统浏览器打开
              </Button>
            </FieldGroup>
          </div>
        </CardBody>
      </TacticalCard>

      <div
        ref={contentHostRef}
        className="relative z-0 min-h-[560px] flex-1 overflow-hidden rounded-lg border border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-tile),color-mix(in_oklch,var(--card)_28%,transparent))]"
      >
        <div className="pointer-events-none absolute inset-0 flex items-center justify-center px-6 text-center text-sm text-muted-foreground">
          <div className="max-w-md rounded-lg border border-[var(--surface-border)] bg-background/82 px-5 py-4 shadow-sm backdrop-blur">
            <p className="font-medium text-foreground">{isNativeShell ? statusMessage : "该工具需要在桌面端使用"}</p>
            <p className="mt-2 text-xs/relaxed">
              {isNativeShell
                ? "网页内容会覆盖此定位宿主区域；切换工具页时会自动关闭 strategy-content。"
                : "浏览器预览模式无法创建 Tauri 子 WebView，请在桌面端使用。"}
            </p>
          </div>
        </div>
      </div>
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

  const reset = () => {
    setShortLabel("");
    setLabel("");
    setUrl("");
    setDescription("");
  };

  const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const accepted = onSubmit({ shortLabel, label, url, description });
    if (accepted) {
      reset();
      setOpen(false);
    }
  };

  return (
    <Dialog onOpenChange={setOpen} open={open}>
      <DialogTrigger asChild>
        <Button type="button" variant="outline">
          <RiAddLine data-icon="inline-start" />
          新增攻略网站
        </Button>
      </DialogTrigger>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>新增攻略网站</DialogTitle>
          <DialogDescription>
            填入简称、标签、URL 与简介。URL 必须以 http:// 或 https:// 开头。
          </DialogDescription>
        </DialogHeader>
        <form className="flex flex-col gap-4" onSubmit={handleSubmit}>
          <FieldGroup className="grid gap-3 md:grid-cols-2">
            <Field>
              <FieldLabel htmlFor="new-site-short">简称</FieldLabel>
              <FieldContent>
                <Input
                  id="new-site-short"
                  maxLength={6}
                  onChange={(event) => setShortLabel(event.currentTarget.value)}
                  placeholder="KK"
                  required
                  value={shortLabel}
                />
              </FieldContent>
              <FieldDescription>2-6 个字符的简短标签。</FieldDescription>
            </Field>
            <Field>
              <FieldLabel htmlFor="new-site-label">完整标签</FieldLabel>
              <FieldContent>
                <Input
                  id="new-site-label"
                  onChange={(event) => setLabel(event.currentTarget.value)}
                  placeholder="KK 日报攻略总览"
                  required
                  value={label}
                />
              </FieldContent>
            </Field>
            <Field className="md:col-span-2">
              <FieldLabel htmlFor="new-site-url">URL</FieldLabel>
              <FieldContent>
                <Input
                  id="new-site-url"
                  inputMode="url"
                  onChange={(event) => setUrl(event.currentTarget.value)}
                  placeholder="https://example.com/path"
                  required
                  type="url"
                  value={url}
                />
              </FieldContent>
              <FieldDescription>必须以 http:// 或 https:// 开头。</FieldDescription>
            </Field>
            <Field className="md:col-span-2">
              <FieldLabel htmlFor="new-site-description">简介</FieldLabel>
              <FieldContent>
                <Input
                  id="new-site-description"
                  onChange={(event) => setDescription(event.currentTarget.value)}
                  placeholder="可选，简要描述站点内容"
                  value={description}
                />
              </FieldContent>
            </Field>
          </FieldGroup>
          <DialogFooter className="gap-2">
            <DialogClose asChild>
              <Button type="button" variant="ghost">
                取消
              </Button>
            </DialogClose>
            <Button type="submit">
              <RiAddLine data-icon="inline-start" />
              添加到工作台
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
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
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { CardBody, SectionHeader, TacticalCard } from "@/components/app/app-ui";
import {
  BUILTIN_STRATEGY_SITES,
  mergeStrategySites,
  readStoredUserSites,
  type StrategySite,
} from "@/components/app/strategy-utils";
import { getErrorMessage } from "@/lib/error-utils";
import { useNativeShell } from "@/hooks/use-native-shell";

const CONTENT_WEBVIEW_LABEL = "strategy-content";

export function StrategyBrowserPage() {
  const isNativeShell = useNativeShell();
  const contentHostRef = useRef<HTMLDivElement | null>(null);
  const webviewRef = useRef<Webview | null>(null);
  const [userSites, setUserSites] = useState<StrategySite[]>(() => readStoredUserSites());
  const [reloadNonce, setReloadNonce] = useState(0);
  const [statusMessage, setStatusMessage] = useState("正在准备攻略浏览器...");
  const allSites = useMemo(() => mergeStrategySites(BUILTIN_STRATEGY_SITES, userSites), [userSites]);
  const initialParams = useMemo(() => {
    const params = new URLSearchParams(window.location.search);
    return {
      siteId: params.get("site") ?? "",
      url: params.get("url") ?? "",
    };
  }, []);
  const [activeId, setActiveId] = useState(() => initialParams.siteId || BUILTIN_STRATEGY_SITES[0]?.id || "");

  const activeSite = useMemo(() => {
    const matched = allSites.find((site) => site.id === activeId);
    if (matched) {
      return matched;
    }
    return allSites[0] ?? null;
  }, [activeId, allSites]);

  const activeUrl = activeSite?.url ?? initialParams.url;

  useEffect(() => {
    if (!activeSite && allSites.length > 0) {
      setActiveId(allSites[0].id);
      return;
    }
    if (activeSite && activeSite.id !== activeId) {
      setActiveId(activeSite.id);
    }
  }, [activeId, activeSite, allSites]);

  const calculateContentBounds = useCallback(() => {
    const rect = contentHostRef.current?.getBoundingClientRect();
    const x = Math.max(0, Math.round(rect?.left ?? 0));
    const y = Math.max(0, Math.round(rect?.top ?? 132));
    const width = Math.max(320, Math.round(rect?.width ?? window.innerWidth));
    const height = Math.max(240, Math.round(window.innerHeight - y));
    return { x, y, width, height };
  }, []);

  const resizeContentWebview = useCallback(async () => {
    if (!isNativeShell) {
      return;
    }
    const webview = webviewRef.current ?? await Webview.getByLabel(CONTENT_WEBVIEW_LABEL);
    if (!webview) {
      return;
    }
    const bounds = calculateContentBounds();
    await webview.setPosition(new LogicalPosition(bounds.x, bounds.y));
    await webview.setSize(new LogicalSize(bounds.width, bounds.height));
  }, [calculateContentBounds, isNativeShell]);

  useEffect(() => {
    if (!isNativeShell || !activeUrl) {
      return;
    }

    let cancelled = false;
    const currentWindow = getCurrentWindow();
    const bounds = calculateContentBounds();
    setStatusMessage(`正在加载 ${activeSite?.label ?? activeUrl}...`);
    void currentWindow.setTitle(activeSite ? `攻略浏览器 - ${activeSite.label}` : "攻略浏览器");

    async function mountWebview() {
      const existing = await Webview.getByLabel(CONTENT_WEBVIEW_LABEL);
      if (existing) {
        await existing.close();
      }
      if (cancelled) {
        return;
      }

      const webview = new Webview(currentWindow, CONTENT_WEBVIEW_LABEL, {
        url: activeUrl,
        x: bounds.x,
        y: bounds.y,
        width: bounds.width,
        height: bounds.height,
        focus: true,
      });
      webviewRef.current = webview;
      await webview.once("tauri://created", () => {
        setStatusMessage(`已加载真实 WebView：${activeSite?.label ?? activeUrl}`);
      });
      await webview.once("tauri://error", (event) => {
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
      if (current) {
        void current.close().catch(() => undefined);
      }
    };
  }, [activeSite, activeUrl, calculateContentBounds, isNativeShell, reloadNonce]);

  useEffect(() => {
    if (!isNativeShell) {
      return;
    }
    let frame = 0;
    const handleResize = () => {
      window.cancelAnimationFrame(frame);
      frame = window.requestAnimationFrame(() => {
        void resizeContentWebview().catch((error: unknown) => {
          setStatusMessage(`调整网页视图尺寸失败：${getErrorMessage(error)}`);
        });
      });
    };
    handleResize();
    window.addEventListener("resize", handleResize);
    return () => {
      window.cancelAnimationFrame(frame);
      window.removeEventListener("resize", handleResize);
    };
  }, [isNativeShell, resizeContentWebview]);

  const handleRefresh = useCallback(() => {
    setReloadNonce((current) => current + 1);
  }, []);

  const handleReloadSites = useCallback(() => {
    setUserSites(readStoredUserSites());
    toast.success("已重新读取自定义攻略网站。");
  }, []);

  const handleOpenExternal = useCallback(async () => {
    if (!activeUrl) {
      return;
    }
    try {
      await openUrl(activeUrl);
    } catch (error) {
      toast.error(`打开系统浏览器失败：${getErrorMessage(error)}`);
    }
  }, [activeUrl]);

  return (
    <div className="flex h-svh min-h-0 flex-col overflow-hidden bg-[radial-gradient(circle_at_top,color-mix(in_oklch,var(--primary)_10%,transparent),transparent_36%),var(--background)] p-3">
      <TacticalCard className="relative z-10 shrink-0">
        <CardBody className="flex flex-col gap-3">
          <div className="flex flex-wrap items-start justify-between gap-3">
            <SectionHeader
              eyebrow="Strategy Browser"
              icon={<RiWindowLine />}
              title="攻略浏览器"
              description="下方区域由独立 WebView2 真实导航加载，站点 cookie、JS 跳转、localStorage 和同源接口不经过 iframe/srcDoc。"
              badge={<Badge variant={isNativeShell ? "default" : "secondary"}>{isNativeShell ? "桌面 WebView2" : "预览不可用"}</Badge>}
            />
            <div className="flex flex-wrap items-center gap-2">
              <Button type="button" variant="default" onClick={handleRefresh} disabled={!isNativeShell || !activeUrl}>
                <RiRefreshLine data-icon="inline-start" />
                刷新当前 WebView
              </Button>
              <Button type="button" variant="secondary" onClick={handleReloadSites}>
                <RiRefreshLine data-icon="inline-start" />
                重新读取站点
              </Button>
              <Button type="button" variant="outline" onClick={handleOpenExternal} disabled={!activeUrl}>
                <RiExternalLinkLine data-icon="inline-start" />
                系统浏览器打开
              </Button>
            </div>
          </div>

          <div className="flex flex-wrap items-center gap-3">
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
            <div className="min-w-0 flex-1 truncate rounded-md border border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-tile),color-mix(in_oklch,var(--card)_38%,transparent))] px-3 py-2 font-mono text-xs text-muted-foreground">
              {activeUrl || "未选择站点"}
            </div>
          </div>
        </CardBody>
      </TacticalCard>

      <div
        ref={contentHostRef}
        className="relative mt-3 min-h-0 flex-1 overflow-hidden rounded-lg border border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-tile),color-mix(in_oklch,var(--card)_28%,transparent))]"
      >
        <div className="pointer-events-none absolute inset-0 flex items-center justify-center px-6 text-center text-sm text-muted-foreground">
          <div className="max-w-md rounded-lg border border-[var(--surface-border)] bg-background/82 px-5 py-4 shadow-sm backdrop-blur">
            <p className="font-medium text-foreground">{isNativeShell ? statusMessage : "该窗口需要在桌面端使用"}</p>
            <p className="mt-2 text-xs/relaxed">
              {isNativeShell
                ? "如果目标站点有人机验证，请直接在真实网页区域内完成。"
                : "浏览器预览模式无法创建 Tauri 子 WebView。"}
            </p>
          </div>
        </div>
      </div>
    </div>
  );
}

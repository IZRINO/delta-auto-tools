import { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState, type CSSProperties, type ReactNode } from "react";
import {
  RiCrosshair2Line,
  RiGamepadLine,
  RiRadarLine,
  RiStarFill,
  RiTimerLine,
  RiAccountPinCircleLine,
  RiBarChartBoxLine,
  RiToolsLine,
  RiCompassDiscoverLine,
} from "@remixicon/react";

import { DeltaAccountsProvider } from "@/hooks/use-delta-accounts";
import { FavoritesProvider, useFavorites } from "@/hooks/use-favorites";
import type { FavoriteCardKind } from "@/components/app/favorites-utils";
import {
  Sidebar,
  SidebarContent,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarInset,
  SidebarMenu,
  SidebarMenuBadge,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarProvider,
  SidebarSeparator,
} from "@/components/ui/sidebar";

import "./App.css";

const appShellStyle = {
  "--sidebar-width": "16rem",
  "--sidebar-width-icon": "3rem",
} as CSSProperties;

const overlayWindowModes = new Set([
  "overlay",
  "timer-display",
  "counter-display",
  "timer-position",
  "counter-position",
  "rapidfire-display",
  "rapidfire-position",
]);

const MorsePage = lazy(() =>
  import("@/components/app/morse-page").then((module) => ({ default: module.MorsePage })),
);
const TimerPage = lazy(() =>
  import("@/components/app/timer-page").then((module) => ({ default: module.TimerPage })),
);
const RapidfirePage = lazy(() =>
  import("@/components/app/rapidfire-page").then((module) => ({ default: module.RapidfirePage })),
);
const DeltaAccountsPage = lazy(() =>
  import("@/components/app/delta-accounts-page").then((module) => ({ default: module.DeltaAccountsPage })),
);
const DeltaGamePage = lazy(() => import("@/components/app/delta-game-page").then((module) => ({ default: module.DeltaGamePage })));
const DeltaToolboxPage = lazy(() =>
  import("@/components/app/delta-toolbox-page").then((module) => ({ default: module.DeltaToolboxPage })),
);
const StrategyPage = lazy(() =>
  import("@/components/app/strategy-page").then((module) => ({ default: module.StrategyPage })),
);
const StrategyBrowserPage = lazy(() =>
  import("@/components/app/strategy-browser-page").then((module) => ({ default: module.StrategyBrowserPage })),
);
const FavoritesPage = lazy(() =>
  import("@/components/app/favorites-page").then((module) => ({ default: module.FavoritesPage })),
);
 const tools = [
  {
    id: "morse" as const,
    icon: RiRadarLine,
    label: "摩斯密码解析",
    short: "Morse",
  },
  {
    id: "timer" as const,
    icon: RiTimerLine,
    label: "计时\\计数器",
    short: "Timer",
  },
  {
    id: "rapidfire" as const,
    icon: RiGamepadLine,
    label: "连发器",
    short: "Rapidfire",
  },
  {
    id: "strategy" as const,
    icon: RiCompassDiscoverLine,
    label: "攻略网站",
    short: "Strategy",
  },
];

const deltaTools = [
  {
    id: "delta-accounts" as const,
    icon: RiAccountPinCircleLine,
    label: "账号管理",
    short: "Accounts",
  },
  {
    id: "delta-game" as const,
    icon: RiBarChartBoxLine,
    label: "游戏数据",
    short: "Game Data",
  },
  {
    id: "delta-toolbox" as const,
    icon: RiToolsLine,
    label: "工具箱",
    short: "Toolbox",
  },
];

type ToolId = (typeof tools)[number]["id"] | (typeof deltaTools)[number]["id"] | "favorites";

function ToolPageFallback() {
  return (
    <div className="flex min-h-[360px] items-center justify-center rounded-xl border border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-card-strong),var(--surface-tile))] px-6 text-center text-sm font-medium text-muted-foreground">
      正在装载工具面板...
    </div>
  );
}

function ToolPageSuspense({ children, fallback = <ToolPageFallback /> }: { children: ReactNode; fallback?: ReactNode }) {
  return <Suspense fallback={fallback}>{children}</Suspense>;
}

type ToolHighlight = { kind: "timer" | "counter"; cardId: string; nonce: number } | { kind: "rapidfire"; cardId: string; nonce: number } | null;

function renderToolPage(
  activeTool: ToolId,
  highlightCardId: ToolHighlight,
  onNavigateFavorite: (kind: FavoriteCardKind, cardId: string) => void,
) {
  switch (activeTool) {
    case "timer":
      return (
        <TimerPage
          highlightCardId={highlightCardId && (highlightCardId.kind === "timer" || highlightCardId.kind === "counter") ? highlightCardId : null}
        />
      );
    case "rapidfire":
      return (
        <RapidfirePage
          highlightCardId={highlightCardId && highlightCardId.kind === "rapidfire" ? highlightCardId : null}
        />
      );
    case "favorites":
      return <FavoritesPage onNavigate={onNavigateFavorite} />;
    case "delta-accounts":
      return <DeltaAccountsPage />;
    case "delta-game":
      return <DeltaGamePage />;
    case "delta-toolbox":
      return <DeltaToolboxPage />;
    case "strategy":
      return <StrategyPage />;
    case "morse":
      return <MorsePage />;
  }
}

function FavoritesSidebarGroup({ active, count, onClick }: { active: boolean; count: number; onClick: () => void }) {
  return (
    <SidebarGroup className="px-0 py-2">
      <SidebarGroupLabel className="font-mono tracking-[0.18em] uppercase">收藏</SidebarGroupLabel>
      <SidebarGroupContent>
        <SidebarMenu className="gap-2">
          <SidebarMenuItem>
            <SidebarMenuButton
              className="h-auto rounded-lg border border-transparent bg-transparent px-3 py-3 transition-all hover:border-sidebar-border/55 hover:bg-background/42 data-[active=true]:border-sidebar-border/70 data-[active=true]:bg-background/62"
              isActive={active}
              onClick={onClick}
              tooltip="收藏的计时器、计数器与连发器"
              type="button"
            >
              <RiStarFill className={active ? "text-amber-500" : "text-muted-foreground"} />
              <span className="flex min-w-0 flex-1 flex-col items-start">
                <span className="truncate text-sm">收藏夹</span>
                <span className="font-mono text-[0.62rem] tracking-[0.16em] text-muted-foreground uppercase">
                  FAV / {count} 项
                </span>
              </span>
              {count > 0 ? <SidebarMenuBadge>{count}</SidebarMenuBadge> : null}
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarGroupContent>
    </SidebarGroup>
  );
}

function App() {
  return (
    <FavoritesProvider>
      <DeltaAccountsProvider>
        <AppShell />
      </DeltaAccountsProvider>
    </FavoritesProvider>
  );
}

function AppShell() {
  const overlayMode = useMemo(() => {
    const params = new URLSearchParams(window.location.search);
    return params.get("mode");
  }, []);
  const [activeTool, setActiveTool] = useState<ToolId>("morse");
  const [highlightCardId, setHighlightCardId] = useState<ToolHighlight>(null);
  const highlightNonceRef = useRef(0);
  const favorites = useFavorites();
  const isOverlayWindowMode = overlayMode !== null && overlayWindowModes.has(overlayMode);

  const handleFavoritesNavigate = useCallback((kind: FavoriteCardKind, cardId: string) => {
    highlightNonceRef.current += 1;
    if (kind === "rapidfire") {
      setActiveTool("rapidfire");
      setHighlightCardId({ kind: "rapidfire", cardId, nonce: highlightNonceRef.current });
    } else {
      setActiveTool("timer");
      setHighlightCardId({ kind, cardId, nonce: highlightNonceRef.current });
    }
  }, []);

  useEffect(() => {
    if (!isOverlayWindowMode) return;
    document.body.dataset.overlayMode = "true";
    return () => {
      delete document.body.dataset.overlayMode;
    };
  }, [isOverlayWindowMode]);
  if (overlayMode === "overlay") {
    return (
      <ToolPageSuspense fallback={null}>
        <MorsePage overlayMode />
      </ToolPageSuspense>
    );
  }

  if (overlayMode === "timer-display") {
    return (
      <ToolPageSuspense fallback={null}>
        <TimerPage overlayMode="display" />
      </ToolPageSuspense>
    );
  }

  if (overlayMode === "counter-display") {
    return (
      <ToolPageSuspense fallback={null}>
        <TimerPage overlayMode="counter-display" />
      </ToolPageSuspense>
    );
  }

  if (overlayMode === "timer-position") {
    return (
      <ToolPageSuspense fallback={null}>
        <TimerPage overlayMode="position" />
      </ToolPageSuspense>
    );
  }

  if (overlayMode === "counter-position") {
    return (
      <ToolPageSuspense fallback={null}>
        <TimerPage overlayMode="counter-position" />
      </ToolPageSuspense>
    );
  }

  if (overlayMode === "rapidfire-display") {
    return (
      <ToolPageSuspense fallback={null}>
        <RapidfirePage overlayMode="display" />
      </ToolPageSuspense>
    );
  }

  if (overlayMode === "rapidfire-position") {
    return (
      <ToolPageSuspense fallback={null}>
        <RapidfirePage overlayMode="position" />
      </ToolPageSuspense>
    );
  }

  if (overlayMode === "strategy-browser") {
    return (
      <ToolPageSuspense fallback={null}>
        <StrategyBrowserPage />
      </ToolPageSuspense>
    );
  }

  return (
    <DeltaAccountsProvider>
      <SidebarProvider className="h-svh min-h-0 overflow-hidden bg-transparent" defaultOpen style={appShellStyle}>
        <Sidebar
          className="min-h-0 overflow-hidden border-r border-sidebar-border/60 bg-[linear-gradient(180deg,color-mix(in_oklch,var(--sidebar)_82%,transparent),color-mix(in_oklch,var(--sidebar)_64%,transparent))] backdrop-blur-2xl"
          collapsible="none"
          variant="sidebar"
        >
          <SidebarHeader className="p-4 pb-3">
            <div className="relative overflow-hidden rounded-xl border border-sidebar-border/65 bg-[linear-gradient(145deg,color-mix(in_oklch,var(--sidebar)_82%,transparent),color-mix(in_oklch,var(--background)_58%,transparent))] px-4 py-4 backdrop-blur-xl">
              <div className="absolute inset-x-0 top-0 h-px bg-[linear-gradient(90deg,transparent,var(--primary),transparent)]" />
              <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(circle,color-mix(in_oklch,var(--surface-dot)_38%,transparent)_0.7px,transparent_0.95px)] bg-[length:20px_20px] opacity-20" />
              <div className="flex items-center gap-3">
                <div className="relative flex size-11 shrink-0 items-center justify-center rounded-lg bg-primary text-primary-foreground">
                  <RiCrosshair2Line />
                </div>
                <div className="min-w-0">
                  <p className="text-sm font-semibold tracking-tight text-foreground">三角洲行动工具</p>
                  <p className="mt-1 truncate font-mono text-[0.62rem] tracking-[0.2em] text-muted-foreground uppercase">Delta Ops Console</p>
                </div>
              </div>
              <div className="mt-4 grid grid-cols-3 gap-2 text-center">
                <div className="rounded-lg border border-sidebar-border/60 bg-background/48 px-2 py-2 backdrop-blur-md">
                  <p className="font-mono text-sm font-semibold text-foreground">07</p>
                  <p className="mt-0.5 text-[0.62rem] text-muted-foreground">工具</p>
                </div>
                <div className="rounded-lg border border-sidebar-border/60 bg-background/48 px-2 py-2 backdrop-blur-md">
                  <p className="font-mono text-sm font-semibold text-foreground">07</p>
                  <p className="mt-0.5 text-[0.62rem] text-muted-foreground">窗口</p>
                </div>
                <div className="rounded-lg border border-sidebar-border/60 bg-background/48 px-2 py-2 backdrop-blur-md">
                  <p className="font-mono text-sm font-semibold text-foreground">2.0</p>
                  <p className="mt-0.5 text-[0.62rem] text-muted-foreground">Tauri</p>
                </div>
              </div>
            </div>
          </SidebarHeader>

          <SidebarSeparator className="mx-4 mt-1" />

          <SidebarContent className="min-h-0 overflow-y-auto overflow-x-hidden px-3 pb-3 [&::-webkit-scrollbar]:hidden [-ms-overflow-style:none] [scrollbar-width:none]">
            <FavoritesSidebarGroup active={activeTool === "favorites"} count={favorites.items.length} onClick={() => setActiveTool("favorites")} />

            <SidebarSeparator className="mx-1 my-1" />

            <SidebarGroup className="px-0 py-2">
              <SidebarGroupLabel className="font-mono tracking-[0.18em] uppercase">当前工具</SidebarGroupLabel>
              <SidebarGroupContent>
                <SidebarMenu className="gap-2">
                  {tools.map((tool, index) => {
                    const Icon = tool.icon;
                    const active = activeTool === tool.id;
                    return (
                      <SidebarMenuItem key={tool.id}>
                        <SidebarMenuButton
                          className="h-auto rounded-lg border border-transparent bg-transparent px-3 py-3 transition-all hover:border-sidebar-border/55 hover:bg-background/42 data-[active=true]:border-sidebar-border/70 data-[active=true]:bg-background/62"
                          isActive={active}
                          onClick={() => setActiveTool(tool.id)}
                          tooltip={tool.label}
                          type="button"
                        >
                          <Icon />
                          <span className="flex min-w-0 flex-1 flex-col items-start">
                            <span className="truncate text-sm">{tool.label}</span>
                            <span className="font-mono text-[0.62rem] tracking-[0.16em] text-muted-foreground uppercase">
                              0{index + 1} / {tool.short}
                            </span>
                          </span>
                        </SidebarMenuButton>
                      </SidebarMenuItem>
                    );
                  })}
                </SidebarMenu>
              </SidebarGroupContent>
            </SidebarGroup>

            <SidebarSeparator className="mx-1 my-1" />

            <SidebarGroup className="px-0 py-2">
              <SidebarGroupLabel className="font-mono tracking-[0.18em] uppercase">三角洲行动 API</SidebarGroupLabel>
              <SidebarGroupContent>
                <SidebarMenu className="gap-2">
                  {deltaTools.map((tool, index) => {
                    const Icon = tool.icon;
                    const active = activeTool === tool.id;
                    return (
                      <SidebarMenuItem key={tool.id}>
                        <SidebarMenuButton
                          className="h-auto rounded-lg border border-transparent bg-transparent px-3 py-3 transition-all hover:border-sidebar-border/55 hover:bg-background/42 data-[active=true]:border-sidebar-border/70 data-[active=true]:bg-background/62"
                          isActive={active}
                          onClick={() => setActiveTool(tool.id)}
                          tooltip={tool.label}
                          type="button"
                        >
                          <Icon />
                          <span className="flex min-w-0 flex-1 flex-col items-start">
                            <span className="truncate text-sm">{tool.label}</span>
                            <span className="font-mono text-[0.62rem] tracking-[0.16em] text-muted-foreground uppercase">
                              D{index + 1} / {tool.short}
                            </span>
                          </span>
                        </SidebarMenuButton>
                      </SidebarMenuItem>
                    );
                  })}
                </SidebarMenu>
              </SidebarGroupContent>
            </SidebarGroup>
          </SidebarContent>
        </Sidebar>

        <SidebarInset className="h-full min-h-0 overflow-hidden bg-transparent">
          <div className="flex h-full min-h-0 flex-1 flex-col overflow-hidden">
            <div className="min-h-0 flex-1 overflow-y-auto px-5 py-5 xl:px-6 xl:py-6 [&::-webkit-scrollbar]:hidden [-ms-overflow-style:none] [scrollbar-width:none]">
              <div className="flex min-h-full flex-col">
                <div className="flex min-h-full flex-col p-4 xl:p-5">
                  <ToolPageSuspense>{renderToolPage(activeTool, highlightCardId, handleFavoritesNavigate)}</ToolPageSuspense>
                </div>
              </div>
            </div>
          </div>
        </SidebarInset>
      </SidebarProvider>
    </DeltaAccountsProvider>
  );
}

export default App;

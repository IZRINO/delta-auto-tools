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
    <div className="flex min-h-[360px] items-center justify-center rounded-md border border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-card-strong),var(--surface-tile))] px-6 text-center font-mono text-sm font-semibold tracking-[0.08em] text-muted-foreground uppercase shadow-[var(--shadow-tile)]">
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
      <SidebarGroupLabel className="font-mono text-[0.62rem] tracking-[0.22em] text-sidebar-foreground/65 uppercase">收藏</SidebarGroupLabel>
      <SidebarGroupContent>
        <SidebarMenu className="gap-1.5">
          <SidebarMenuItem>
            <SidebarMenuButton
              className="relative h-auto rounded-md border border-transparent bg-transparent px-3 py-3 transition-[border-color,background-color,transform] hover:border-sidebar-border hover:bg-background/45 active:translate-y-px data-[active=true]:border-sidebar-primary/55 data-[active=true]:bg-background/70 data-[active=true]:before:absolute data-[active=true]:before:inset-y-2 data-[active=true]:before:left-0 data-[active=true]:before:w-1 data-[active=true]:before:bg-sidebar-primary"
              isActive={active}
              onClick={onClick}
              tooltip="收藏的计时器、计数器与连发器"
              type="button"
            >
              <RiStarFill className={active ? "text-primary" : "text-muted-foreground"} />
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


  return (
    <SidebarProvider className="h-dvh min-h-0 overflow-hidden bg-transparent" defaultOpen style={appShellStyle}>
      <a
        className="sr-only fixed left-4 top-4 z-50 rounded-md border border-[var(--surface-border)] bg-card px-3 py-2 font-mono text-sm font-semibold tracking-[0.08em] text-foreground uppercase shadow-[var(--shadow-card)] focus:not-sr-only focus:outline-none focus:ring-2 focus:ring-ring/50"
        href="#app-content"
      >
        跳到主内容
      </a>
      <Sidebar
        className="min-h-0 overflow-hidden border-r-2 border-sidebar-border bg-[linear-gradient(180deg,var(--sidebar),color-mix(in_srgb,var(--sidebar)_78%,var(--background)_22%))] shadow-[var(--shadow-shell)]"
        collapsible="none"
        variant="sidebar"
      >
        <SidebarHeader className="p-4 pb-3">
          <div className="relative overflow-hidden rounded-md border border-sidebar-border bg-[linear-gradient(145deg,color-mix(in_srgb,var(--sidebar)_86%,transparent),color-mix(in_srgb,var(--background)_48%,transparent))] px-4 py-4 shadow-[var(--shadow-subtle)]">
            <div className="absolute inset-x-0 top-0 h-1 bg-primary" />
            <div className="pointer-events-none absolute inset-0 bg-[linear-gradient(color-mix(in_srgb,var(--surface-dot)_22%,transparent)_1px,transparent_1px),linear-gradient(90deg,color-mix(in_srgb,var(--surface-dot)_22%,transparent)_1px,transparent_1px)] bg-[length:18px_18px] opacity-60" />
            <div className="relative flex items-center gap-3">
              <div className="relative flex size-11 shrink-0 items-center justify-center rounded-md border border-sidebar-primary/45 bg-primary text-primary-foreground shadow-[var(--shadow-tile)]">
                <RiCrosshair2Line />
              </div>
              <div className="min-w-0">
                <p className="text-sm font-semibold tracking-[-0.01em] text-foreground">三角洲行动工具</p>
                <p className="mt-1 truncate font-mono text-[0.62rem] tracking-[0.22em] text-muted-foreground uppercase">Delta Ops Console</p>
              </div>
            </div>
            <div className="relative mt-4 grid grid-cols-3 gap-2 text-center">
              <div className="rounded-md border border-sidebar-border bg-background/56 px-2 py-2 shadow-[var(--shadow-tile)]">
                <p className="font-mono text-sm font-semibold text-foreground">{tools.length + deltaTools.length}</p>
                <p className="mt-0.5 font-mono text-[0.6rem] tracking-[0.12em] text-muted-foreground uppercase">工具</p>
              </div>
              <div className="rounded-md border border-sidebar-border bg-background/56 px-2 py-2 shadow-[var(--shadow-tile)]">
                <p className="font-mono text-sm font-semibold text-foreground">{overlayWindowModes.size}</p>
                <p className="mt-0.5 font-mono text-[0.6rem] tracking-[0.12em] text-muted-foreground uppercase">窗口</p>
              </div>
              <div className="rounded-md border border-sidebar-border bg-background/56 px-2 py-2 shadow-[var(--shadow-tile)]">
                <p className="font-mono text-sm font-semibold text-foreground">本机</p>
                <p className="mt-0.5 font-mono text-[0.6rem] tracking-[0.12em] text-muted-foreground uppercase">Tauri</p>
              </div>
            </div>
          </div>
        </SidebarHeader>

        <SidebarSeparator className="mx-4 mt-1" />

        <SidebarContent className="min-h-0 overflow-y-auto overflow-x-hidden px-3 pb-3 [&::-webkit-scrollbar]:hidden [-ms-overflow-style:none] [scrollbar-width:none]">
          <FavoritesSidebarGroup active={activeTool === "favorites"} count={favorites.items.length} onClick={() => setActiveTool("favorites")} />

          <SidebarSeparator className="mx-1 my-1" />

          <SidebarGroup className="px-0 py-2">
            <SidebarGroupLabel className="font-mono text-[0.62rem] tracking-[0.22em] text-sidebar-foreground/65 uppercase">当前工具</SidebarGroupLabel>
            <SidebarGroupContent>
              <SidebarMenu className="gap-1.5">
                {tools.map((tool, index) => {
                  const Icon = tool.icon;
                  const active = activeTool === tool.id;
                  return (
                    <SidebarMenuItem key={tool.id}>
                      <SidebarMenuButton
                        className="relative h-auto rounded-md border border-transparent bg-transparent px-3 py-3 transition-[border-color,background-color,transform] hover:border-sidebar-border hover:bg-background/45 active:translate-y-px data-[active=true]:border-sidebar-primary/55 data-[active=true]:bg-background/70 data-[active=true]:shadow-[var(--shadow-tile)] data-[active=true]:before:absolute data-[active=true]:before:inset-y-2 data-[active=true]:before:left-0 data-[active=true]:before:w-1 data-[active=true]:before:bg-sidebar-primary"
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
            <SidebarGroupLabel className="font-mono text-[0.62rem] tracking-[0.22em] text-sidebar-foreground/65 uppercase">三角洲行动 API</SidebarGroupLabel>
            <SidebarGroupContent>
              <SidebarMenu className="gap-1.5">
                {deltaTools.map((tool, index) => {
                  const Icon = tool.icon;
                  const active = activeTool === tool.id;
                  return (
                    <SidebarMenuItem key={tool.id}>
                      <SidebarMenuButton
                        className="relative h-auto rounded-md border border-transparent bg-transparent px-3 py-3 transition-[border-color,background-color,transform] hover:border-sidebar-border hover:bg-background/45 active:translate-y-px data-[active=true]:border-sidebar-primary/55 data-[active=true]:bg-background/70 data-[active=true]:shadow-[var(--shadow-tile)] data-[active=true]:before:absolute data-[active=true]:before:inset-y-2 data-[active=true]:before:left-0 data-[active=true]:before:w-1 data-[active=true]:before:bg-sidebar-primary"
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

      <SidebarInset id="app-content" tabIndex={-1} className="h-full min-h-0 overflow-hidden bg-transparent focus:outline-none">
        <div className="flex h-full min-h-0 flex-1 flex-col overflow-hidden">
          <div className="min-h-0 flex-1 overflow-y-auto px-4 py-4 xl:px-5 xl:py-5 [&::-webkit-scrollbar]:hidden [-ms-overflow-style:none] [scrollbar-width:none]">
            <div className="mx-auto flex min-h-full w-full max-w-[1440px] flex-col">
              <div className="flex min-h-full flex-col p-3 xl:p-4">
                <ToolPageSuspense>{renderToolPage(activeTool, highlightCardId, handleFavoritesNavigate)}</ToolPageSuspense>
              </div>
            </div>
          </div>
        </div>
      </SidebarInset>
    </SidebarProvider>
  );
}

export default App;

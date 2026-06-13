import { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import {
  RiCrosshair2Line,
  RiGamepadLine,
  RiRadarLine,
  RiStarFill,
  RiTimerLine,
  RiSpeedUpLine,
  RiAccountPinCircleLine,
  RiBarChartBoxLine,
  RiToolsLine,
  RiCompassDiscoverLine,
  RiShutDownLine,
} from "@remixicon/react";

import { DeltaAccountsProvider } from "@/hooks/use-delta-accounts";
import { FavoritesProvider, useFavorites } from "@/hooks/use-favorites";
import { GlobalEnabledProvider, useGlobalEnabled } from "@/hooks/use-global-enabled";
import type { FavoriteCardKind } from "@/components/app/favorites-utils";
import { Switch } from "@/components/ui/switch";
import { cn } from "@/lib/utils";

import "./App.css";


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
const CounterPage = lazy(() =>
  import("@/components/app/counter-page").then((module) => ({ default: module.CounterPage })),
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
    id: "timer" as const,
    icon: RiTimerLine,
    label: "计时器",
    short: "Timer",
  },
  {
    id: "counter" as const,
    icon: RiSpeedUpLine,
    label: "计数器",
    short: "Counter",
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
    id: "morse" as const,
    icon: RiRadarLine,
    label: "摩斯密码解析",
    short: "Morse",
  },
];

const deltaApiTools = [
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

type ToolId = (typeof tools)[number]["id"] | (typeof deltaTools)[number]["id"] | (typeof deltaApiTools)[number]["id"] | "favorites";

function ToolPageFallback() {
  return (
    <div className="flex min-h-[360px] items-center justify-center border-2 border-[var(--chalk)] bg-[var(--slate)] px-6 text-center font-mono text-xs font-black tracking-[0.18em] text-[var(--zinc)] uppercase">
      [ 正在装载工具面板 ]
    </div>
  );
}

function ToolPageSuspense({ children, fallback = <ToolPageFallback /> }: { children: ReactNode; fallback?: ReactNode }) {
  return <Suspense fallback={fallback}>{children}</Suspense>;
}

type ToolHighlight = { kind: "timer"; cardId: string; nonce: number } | { kind: "counter"; cardId: string; nonce: number } | { kind: "rapidfire"; cardId: string; nonce: number } | null;

function renderToolPage(
  activeTool: ToolId,
  highlightCardId: ToolHighlight,
  onNavigateFavorite: (kind: FavoriteCardKind, cardId: string) => void,
) {
  switch (activeTool) {
    case "timer":
      return (
        <TimerPage
          highlightCardId={highlightCardId && highlightCardId.kind === "timer" ? highlightCardId : null}
        />
      );
    case "counter":
      return (
        <CounterPage
          highlightCardId={highlightCardId && highlightCardId.kind === "counter" ? highlightCardId : null}
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

function IndexRailItem({
  active,
  code,
  icon: Icon,
  label,
  meta,
  onClick,
}: {
  active: boolean;
  code: string;
  icon: typeof RiStarFill;
  label: string;
  meta: string;
  onClick: () => void;
}) {
  return (
    <button
      className="group relative grid w-full grid-cols-[0.25rem_1fr_2.25rem] items-stretch border-b border-[var(--chalk)] bg-[var(--carbon)] text-left transition-colors hover:bg-[var(--slate)] focus:outline-none focus-visible:outline-2 focus-visible:outline-offset-[-2px] focus-visible:outline-[var(--amber)] data-[active=true]:bg-[var(--chalk)] data-[active=true]:text-[var(--carbon)]"
      data-active={active}
      onClick={onClick}
      type="button"
    >
      <span className="bg-transparent group-data-[active=true]:bg-[var(--amber)]" aria-hidden="true" />
      <span className="min-w-0 px-3 py-3">
        <span className="block truncate text-sm font-black tracking-[-0.02em] uppercase">{label}</span>
        <span className="mt-1 block truncate font-mono text-[0.62rem] font-bold tracking-[0.14em] text-[var(--zinc)] uppercase group-data-[active=true]:text-[var(--slate)]">
          {code} / {meta}
        </span>
      </span>
      <span className="flex items-center justify-center border-l border-[var(--chalk)] text-[var(--amber)] group-data-[active=true]:text-[var(--carbon)]">
        <Icon className="size-4" aria-hidden="true" />
      </span>
    </button>
  );
}

function IndexRailSection({ children, title }: { children: ReactNode; title: string }) {
  return (
    <section className="border-t-2 border-[var(--chalk)]">
      <div className="flex h-8 items-center border-b border-[var(--chalk)] bg-[var(--chalk)] px-3 font-mono text-[0.62rem] font-black tracking-[0.24em] text-[var(--carbon)] uppercase">
        [ {title} ]
      </div>
      <div>{children}</div>
    </section>
  );
}

function GlobalDisabledBanner() {
  return (
    <div className="mb-2 border-2 border-[var(--alert-red)] bg-[var(--alert-red)]/10 px-3 py-2 font-mono text-xs font-black tracking-[0.12em] text-[var(--alert-red)] uppercase">
      [ 全局总开关已关闭 ] 所有自动化功能与热键均已暂停，请在顶部工具栏重新开启。
    </div>
  );
}

function GlobalEnabledConsumer() {
  const { globalEnabled } = useGlobalEnabled();
  if (globalEnabled) return null;
  return <GlobalDisabledBanner />;
}

function GlobalSwitch() {
  const { globalEnabled, setGlobalEnabled } = useGlobalEnabled();
  return (
    <div
      className={cn(
        "flex items-center gap-2 border-2 px-2 py-1.5 font-mono text-[0.58rem] font-black tracking-[0.14em] uppercase",
        globalEnabled
          ? "border-[var(--moss)] bg-[var(--moss)]/10 text-[var(--moss)]"
          : "border-[var(--alert-red)] bg-[var(--alert-red)]/10 text-[var(--alert-red)]",
      )}
    >
      <RiShutDownLine className="size-3.5" aria-hidden="true" />
      <span>{globalEnabled ? "全局开启" : "全局关闭"}</span>
      <Switch
        checked={globalEnabled}
        onCheckedChange={setGlobalEnabled}
        aria-label="全局总开关"
      />
    </div>
  );
}

function FavoritesIndexRailItem({ active, count, onClick }: { active: boolean; count: number; onClick: () => void }) {
  return (
    <IndexRailItem
      active={active}
      code="PIN"
      icon={RiStarFill}
      label="收藏夹"
      meta={`PINNED / ${count}`}
      onClick={onClick}
    />
  );
}

function App() {
  return (
    <FavoritesProvider>
      <DeltaAccountsProvider>
        <GlobalEnabledProvider>
          <AppShell />
        </GlobalEnabledProvider>
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
    } else if (kind === "counter") {
      setActiveTool("counter");
      setHighlightCardId({ kind: "counter", cardId, nonce: highlightNonceRef.current });
    } else {
      setActiveTool("timer");
      setHighlightCardId({ kind: "timer", cardId, nonce: highlightNonceRef.current });
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
        <CounterPage overlayMode="counter-display" />
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
        <CounterPage overlayMode="counter-position" />
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


  const activeMeta = [...tools, ...deltaTools, ...deltaApiTools].find((tool) => tool.id === activeTool);

  return (
    <div className="grid h-dvh min-h-0 grid-rows-[48px_1fr] overflow-hidden bg-transparent">
      <a
        className="sr-only fixed left-4 top-4 z-50 border-2 border-[var(--chalk)] bg-[var(--carbon)] px-3 py-2 font-mono text-sm font-black tracking-[0.16em] text-[var(--chalk)] uppercase focus:not-sr-only focus:outline-2 focus:outline-[var(--amber)]"
        href="#app-content"
      >
        跳到主内容
      </a>

      <header className="grid min-h-0 grid-cols-[240px_minmax(0,1fr)] border-b-2 border-[var(--chalk)] bg-[var(--carbon)] text-[var(--chalk)]">
        <div className="flex items-center gap-3 border-r-2 border-[var(--chalk)] px-3">
          <div className="flex size-8 items-center justify-center border-2 border-[var(--chalk)] bg-[var(--amber)] text-[var(--carbon)]">
            <RiCrosshair2Line className="size-5" aria-hidden="true" />
          </div>
          <div className="min-w-0 leading-none">
            <p className="truncate text-sm font-black tracking-[-0.03em] uppercase">三角洲行动工具</p>
            <p className="mt-1 truncate font-mono text-[0.58rem] font-bold tracking-[0.18em] text-[var(--zinc)] uppercase">Delta Auto Tools</p>
          </div>
        </div>
        <div className="flex min-h-0 items-center justify-between gap-3 border-l-2 border-[var(--chalk)] px-3">
          <div className="flex min-w-0 items-center gap-3">
            <span className="shrink-0 border-2 border-[var(--chalk)] bg-[var(--chalk)] px-2 py-1 font-heading text-base font-black tracking-[-0.06em] text-[var(--amber)] uppercase">
              {activeTool === "favorites" ? "PIN" : activeTool === "timer" ? "01" : activeTool === "counter" ? "02" : activeTool === "rapidfire" ? "03" : activeTool === "strategy" ? "04" : activeTool === "morse" ? "D1" : activeTool === "delta-accounts" ? "A1" : activeTool === "delta-game" ? "A2" : "A3"}
            </span>
            <div className="min-w-0">
              <p className="truncate text-xs font-black tracking-[-0.02em] uppercase">
                {activeMeta?.label ?? "收藏夹"}
              </p>
              <p className="mt-0.5 truncate font-mono text-[0.58rem] font-bold tracking-[0.14em] text-[var(--zinc)] uppercase">
                {activeMeta?.short ?? "Pinned"} / ACTIVE
              </p>
            </div>
          </div>
          <div className="flex items-center gap-3">
            <GlobalSwitch />
            <div className="hidden items-center border-2 border-[var(--chalk)] bg-[var(--chalk)] font-mono text-[0.58rem] font-black tracking-[0.14em] text-[var(--carbon)] uppercase sm:flex">
              <span className="bg-[var(--carbon)] px-3 py-2 text-[var(--chalk)]">DESKTOP</span>
              <span className="px-3 py-2 text-[var(--carbon)]">Tauri</span>
            </div>
          </div>
        </div>
      </header>

      <div className="grid min-h-0 grid-cols-[240px_minmax(0,1fr)] overflow-hidden">
        <aside className="min-h-0 overflow-y-auto border-r-2 border-[var(--chalk)] bg-[var(--slate)] [-ms-overflow-style:none] [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
          <div className="border-b-2 border-[var(--chalk)] bg-[var(--carbon)] px-3 py-2 font-mono text-[0.58rem] font-black tracking-[0.18em] text-[var(--zinc)] uppercase">
            工具索引 / 收藏 {favorites.items.length}
          </div>

          <FavoritesIndexRailItem active={activeTool === "favorites"} count={favorites.items.length} onClick={() => setActiveTool("favorites")} />

          <IndexRailSection title="通用工具">
            {tools.map((tool, index) => (
              <IndexRailItem
                active={activeTool === tool.id}
                code={`0${index + 1}`}
                icon={tool.icon}
                key={tool.id}
                label={tool.label}
                meta={tool.short}
                onClick={() => setActiveTool(tool.id)}
              />
            ))}
          </IndexRailSection>

          <IndexRailSection title="三角洲工具">
            {deltaTools.map((tool, index) => (
              <IndexRailItem
                active={activeTool === tool.id}
                code={`D${index + 1}`}
                icon={tool.icon}
                key={tool.id}
                label={tool.label}
                meta={tool.short}
                onClick={() => setActiveTool(tool.id)}
              />
            ))}
          </IndexRailSection>

          <IndexRailSection title="三角洲行动 API">
            {deltaApiTools.map((tool, index) => (
              <IndexRailItem
                active={activeTool === tool.id}
                code={`A${index + 1}`}
                icon={tool.icon}
                key={tool.id}
                label={tool.label}
                meta={tool.short}
                onClick={() => setActiveTool(tool.id)}
              />
            ))}
          </IndexRailSection>
        </aside>

        <main
          id="app-content"
          tabIndex={-1}
          className="min-h-0 overflow-y-auto bg-transparent focus:outline-none [-ms-overflow-style:none] [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
        >
          <div className="min-h-full w-full px-2 py-2 xl:px-3 xl:py-3">
            <GlobalEnabledConsumer />
            <ToolPageSuspense>{renderToolPage(activeTool, highlightCardId, handleFavoritesNavigate)}</ToolPageSuspense>
          </div>
        </main>
      </div>
    </div>
  );
}

export default App;

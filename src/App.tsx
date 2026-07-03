import {lazy, type ReactNode, Suspense, useCallback, useEffect, useMemo, useRef, useState} from "react";
import {
    RiCompassDiscoverLine,
    RiCrosshair2Line,
    RiGamepadLine,
    RiRadarLine,
    RiSettings3Line,
    RiShutDownLine,
    RiSpeedUpLine,
    RiStarFill,
    RiTimerLine,
    RiVolumeUpLine,
} from "@remixicon/react";

import {FavoritesProvider, useFavorites} from "@/hooks/use-favorites";
import {GlobalEnabledProvider, useGlobalEnabled} from "@/hooks/use-global-enabled";
import {ThemeProvider} from "@/hooks/use-theme";
import {ProfileProvider, useProfile} from "@/hooks/use-profile";
import {ProfileSwitcher} from "@/components/app/profile-switcher";
import type {FavoriteCardKind} from "@/components/app/favorites-utils";
import {Switch} from "@/components/ui/switch";
import {cn} from "@/lib/utils";

import "./App.css";

const overlayWindowModes = new Set([
    "overlay",
    "timer-display",
    "counter-display",
    "timer-position",
    "counter-position",
    "rapidfire-display",
    "rapidfire-position",
    "audio-overlay",
]);

const MorsePage = lazy(() =>
    import("@/components/app/morse-page").then((module) => ({default: module.MorsePage})),
);
const TimerPage = lazy(() =>
    import("@/components/app/timer-page").then((module) => ({default: module.TimerPage})),
);
const CounterPage = lazy(() =>
    import("@/components/app/counter-page").then((module) => ({default: module.CounterPage})),
);
const RapidfirePage = lazy(() =>
    import("@/components/app/rapidfire-page").then((module) => ({default: module.RapidfirePage})),
);
const StrategyPage = lazy(() =>
    import("@/components/app/strategy-page").then((module) => ({default: module.StrategyPage})),
);
const FavoritesPage = lazy(() =>
    import("@/components/app/favorites-page").then((module) => ({default: module.FavoritesPage})),
);
const AudioPage = lazy(() =>
    import("@/components/app/audio-page").then((module) => ({default: module.AudioPage})),
);

const AudioRegionOverlay = lazy(() =>
    import("@/components/app/audio-page").then((module) => ({default: module.AudioRegionOverlay})),
);

const SettingsDialog = lazy(() =>
    import("@/components/app/settings-page").then((module) => ({default: module.SettingsDialog})),
);

const tools = [
    {id: "timer" as const, icon: RiTimerLine, label: "计时器", short: "Timer"},
    {id: "counter" as const, icon: RiSpeedUpLine, label: "计数器", short: "Counter"},
    {id: "rapidfire" as const, icon: RiGamepadLine, label: "连发器", short: "Rapidfire"},
    {id: "strategy" as const, icon: RiCompassDiscoverLine, label: "攻略网站", short: "Strategy"},
    {id: "audio" as const, icon: RiVolumeUpLine, label: "音频", short: "Audio"},
];

const deltaTools = [
    {id: "morse" as const, icon: RiRadarLine, label: "摩斯密码解析", short: "Morse"},
];

type ToolId = (typeof tools)[number]["id"] | (typeof deltaTools)[number]["id"] | "favorites";

function ToolPageFallback() {
    return (
        <div
            className="flex min-h-[360px] items-center justify-center border-2 border-[var(--chalk)] bg-[var(--slate)] px-6 text-center font-mono text-xs font-black tracking-[0.18em] text-[var(--zinc)] uppercase">
            [ 正在装载工具面板 ]
        </div>
    );
}

function ToolPageSuspense({children, fallback = <ToolPageFallback/>}: { children: ReactNode; fallback?: ReactNode }) {
    return <Suspense fallback={fallback}>{children}</Suspense>;
}

type ToolHighlight = { kind: "timer"; cardId: string; nonce: number } | {
    kind: "counter";
    cardId: string;
    nonce: number
} | { kind: "rapidfire"; cardId: string; nonce: number } | null;

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
            return <FavoritesPage onNavigate={onNavigateFavorite}/>;
        case "strategy":
            return <StrategyPage/>;
        case "audio":
            return <AudioPage/>;
        case "morse":
            return <MorsePage/>;
    }
}

/* ────────── Top Tab Bar (visible only on <1024px) ────────── */

function TopTabItem({
                        active,
                        icon: Icon,
                        label,
                        onClick,
                    }: {
    active: boolean;
    icon: typeof RiStarFill;
    label: string;
    onClick: () => void;
}) {
    return (
        <button
            className={cn(
                "flex flex-col items-center justify-center gap-0.5 border-b-2 px-3 py-1.5 font-mono text-[0.58rem] font-black tracking-[0.12em] uppercase focus:outline-none focus-visible:outline-2 focus-visible:outline-offset-[-2px] focus-visible:outline-[var(--amber)]",
                active
                    ? "border-[var(--amber)] bg-[var(--chalk)] text-[var(--carbon)]"
                    : "border-transparent text-[var(--zinc)] hover:bg-[var(--slate)] hover:text-[var(--chalk)]",
            )}
            onClick={onClick}
            type="button"
        >
            <Icon className="size-4" aria-hidden="true"/>
            <span className="truncate">{label}</span>
        </button>
    );
}

function TopTabBar({activeTool, onToolClick}: { activeTool: ToolId; onToolClick: (id: ToolId) => void }) {
    return (
        <nav
            className="flex items-center overflow-x-auto border-b-2 border-[var(--chalk)] bg-[var(--carbon)] [-ms-overflow-style:none] [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
            <TopTabItem
                active={activeTool === "favorites"}
                icon={RiStarFill}
                label="收藏"
                onClick={() => onToolClick("favorites")}
            />
            {tools.map((tool) => (
                <TopTabItem
                    key={tool.id}
                    active={activeTool === tool.id}
                    icon={tool.icon}
                    label={tool.label}
                    onClick={() => onToolClick(tool.id)}
                />
            ))}
            {deltaTools.map((tool) => (
                <TopTabItem
                    key={tool.id}
                    active={activeTool === tool.id}
                    icon={tool.icon}
                    label={tool.label}
                    onClick={() => onToolClick(tool.id)}
                />
            ))}
        </nav>
    );
}

/* ────────── Left Index Rail (visible only on >=1024px) ────────── */

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
            <span className="bg-transparent group-data-[active=true]:bg-[var(--amber)]" aria-hidden="true"/>
            <span className="min-w-0 px-3 py-3">
        <span className="block truncate text-sm font-black tracking-[-0.02em] uppercase">{label}</span>
        <span
            className="mt-1 block truncate font-mono text-[0.62rem] font-bold tracking-[0.14em] text-[var(--zinc)] uppercase group-data-[active=true]:text-[var(--slate)]">
          {code} / {meta}
        </span>
      </span>
            <span
                className="flex items-center justify-center border-l border-[var(--chalk)] text-[var(--amber)] group-data-[active=true]:text-[var(--carbon)]">
        <Icon className="size-4" aria-hidden="true"/>
      </span>
        </button>
    );
}

function IndexRailSection({children, title}: { children: ReactNode; title: string }) {
    return (
        <section className="border-t-2 border-[var(--chalk)]">
            <div
                className="flex h-8 items-center border-b border-[var(--chalk)] bg-[var(--chalk)] px-3 font-mono text-[0.62rem] font-black tracking-[0.24em] text-[var(--carbon)] uppercase">
                [ {title} ]
            </div>
            <div>{children}</div>
        </section>
    );
}

function FavoritesIndexRailItem({active, count, onClick}: { active: boolean; count: number; onClick: () => void }) {
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

/* ────────── Global UI ────────── */

function GlobalDisabledBanner() {
    return (
        <div
            className="mb-2 border-2 border-[var(--alert-red)] bg-[var(--alert-red)]/10 px-3 py-2 font-mono text-xs font-black tracking-[0.12em] text-[var(--alert-red)] uppercase">
            [ 全局总开关已关闭 ] 所有自动化功能与热键均已暂停，请在顶部工具栏重新开启。
        </div>
    );
}

function GlobalEnabledConsumer({activeTool}: { activeTool: ToolId }) {
    const {globalEnabled} = useGlobalEnabled();
    if (globalEnabled || activeTool === "strategy") return null;
    return <GlobalDisabledBanner/>;
}

function GlobalSwitch() {
    const {globalEnabled, setGlobalEnabled} = useGlobalEnabled();
    return (
        <div
            className={cn(
                "flex items-center gap-2 border-2 px-2 py-1.5 font-mono text-[0.58rem] font-black tracking-[0.14em] uppercase",
                globalEnabled
                    ? "border-[var(--moss)] bg-[var(--moss)]/10 text-[var(--moss)]"
                    : "border-[var(--alert-red)] bg-[var(--alert-red)]/10 text-[var(--alert-red)]",
            )}
        >
            <RiShutDownLine className="size-3.5" aria-hidden="true"/>
            <span>{globalEnabled ? "全局开启" : "全局关闭"}</span>
            <Switch
                checked={globalEnabled}
                onCheckedChange={setGlobalEnabled}
                aria-label="全局总开关"
            />
        </div>
    );
}

/* ────────── App Root ────────── */

function App() {
    return (
        <FavoritesProvider>
            <GlobalEnabledProvider>
                <ThemeProvider>
                    <ProfileProvider>
                        <AppShell/>
                    </ProfileProvider>
                </ThemeProvider>
            </GlobalEnabledProvider>
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
    const [settingsOpen, setSettingsOpen] = useState(false);
    const favorites = useFavorites();
    const {reloadNonce} = useProfile();
    const isOverlayWindowMode = overlayMode !== null && overlayWindowModes.has(overlayMode);

    const handleFavoritesNavigate = useCallback((kind: FavoriteCardKind, cardId: string) => {
        highlightNonceRef.current += 1;
        if (kind === "rapidfire") {
            setActiveTool("rapidfire");
            setHighlightCardId({kind: "rapidfire", cardId, nonce: highlightNonceRef.current});
        } else if (kind === "counter") {
            setActiveTool("counter");
            setHighlightCardId({kind: "counter", cardId, nonce: highlightNonceRef.current});
        } else {
            setActiveTool("timer");
            setHighlightCardId({kind: "timer", cardId, nonce: highlightNonceRef.current});
        }
    }, []);

    useEffect(() => {
        if (!isOverlayWindowMode) return;
        // ponytail: daisyUI 把背景写在 :root，overlay 必须同时标记 html/body。
        document.documentElement.dataset.overlayMode = "true";
        document.body.dataset.overlayMode = "true";
        return () => {
            delete document.documentElement.dataset.overlayMode;
            delete document.body.dataset.overlayMode;
        };
    }, [isOverlayWindowMode]);

    if (overlayMode === "overlay") {
        return (
            <ToolPageSuspense fallback={null}>
                <MorsePage overlayMode/>
            </ToolPageSuspense>
        );
    }

    if (overlayMode === "timer-display") {
        return (
            <ToolPageSuspense fallback={null}>
                <TimerPage overlayMode="display"/>
            </ToolPageSuspense>
        );
    }

    if (overlayMode === "counter-display") {
        return (
            <ToolPageSuspense fallback={null}>
                <CounterPage overlayMode="counter-display"/>
            </ToolPageSuspense>
        );
    }

    if (overlayMode === "timer-position") {
        return (
            <ToolPageSuspense fallback={null}>
                <TimerPage overlayMode="position"/>
            </ToolPageSuspense>
        );
    }

    if (overlayMode === "counter-position") {
        return (
            <ToolPageSuspense fallback={null}>
                <CounterPage overlayMode="counter-position"/>
            </ToolPageSuspense>
        );
    }

    if (overlayMode === "rapidfire-display") {
        return (
            <ToolPageSuspense fallback={null}>
                <RapidfirePage overlayMode="display"/>
            </ToolPageSuspense>
        );
    }

    if (overlayMode === "rapidfire-position") {
        return (
            <ToolPageSuspense fallback={null}>
                <RapidfirePage overlayMode="position"/>
            </ToolPageSuspense>
        );
    }

    if (overlayMode === "audio-overlay") {
        return (
            <ToolPageSuspense fallback={null}>
                <AudioRegionOverlay/>
            </ToolPageSuspense>
        );
    }

    const activeMeta = [...tools, ...deltaTools].find((tool) => tool.id === activeTool);

    return (
        <div className="grid h-dvh min-h-0 grid-rows-[48px_1fr] overflow-hidden bg-transparent">
            <a
                className="sr-only fixed left-4 top-4 z-50 border-2 border-[var(--chalk)] bg-[var(--carbon)] px-3 py-2 font-mono text-sm font-black tracking-[0.16em] text-[var(--chalk)] uppercase focus:not-sr-only focus:outline-2 focus:outline-[var(--amber)]"
                href="#app-content"
            >
                跳到主内容
            </a>

            {/* Top Manifest Bar */}
            <header
                className="grid min-h-0 grid-cols-[240px_minmax(0,1fr)] border-b-2 border-[var(--chalk)] bg-[var(--carbon)] text-[var(--chalk)]">
                <div className="flex items-center gap-3 border-r-2 border-[var(--chalk)] px-3">
                    <div
                        className="flex size-8 items-center justify-center border-2 border-[var(--chalk)] bg-[var(--amber)] text-[var(--carbon)]">
                        <RiCrosshair2Line className="size-5" aria-hidden="true"/>
                    </div>
                    <div className="min-w-0 leading-none">
                        <p className="truncate text-sm font-black tracking-[-0.03em] uppercase">三角洲行动工具</p>
                        <p className="mt-1 truncate font-mono text-[0.58rem] font-bold tracking-[0.18em] text-[var(--zinc)] uppercase">Delta
                            Auto Tools</p>
                    </div>
                </div>
                <div className="flex min-h-0 items-center justify-between gap-3 border-l-2 border-[var(--chalk)] px-3">
                    <div className="flex min-w-0 items-center gap-3">
            <span
                className="shrink-0 border-2 border-[var(--chalk)] bg-[var(--chalk)] px-2 py-1 font-heading text-base font-black tracking-[-0.06em] text-[var(--amber)] uppercase">
              {activeTool === "favorites" ? "PIN" : activeTool === "timer" ? "01" : activeTool === "counter" ? "02" : activeTool === "rapidfire" ? "03" : activeTool === "strategy" ? "04" : activeTool === "audio" ? "05" : "D1"}
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
                        <ProfileSwitcher/>
                        <GlobalSwitch/>
                        <div
                            className="hidden items-center border-2 border-[var(--chalk)] bg-[var(--chalk)] font-mono text-[0.58rem] font-black tracking-[0.14em] text-[var(--carbon)] uppercase sm:flex">
                            <span className="bg-[var(--carbon)] px-3 py-2 text-[var(--chalk)]">DESKTOP</span>
                            <span className="px-3 py-2 text-[var(--carbon)]">Tauri</span>
                        </div>
                    </div>
                </div>
            </header>

            {/* Top Tab Bar (mobile/tablet <1024px) */}
            <div className="lg:hidden">
                <TopTabBar activeTool={activeTool} onToolClick={setActiveTool}/>
            </div>

            <div className="grid min-h-0 grid-cols-1 overflow-hidden lg:grid-cols-[240px_minmax(0,1fr)]">
                {/* Left Index Rail (desktop >=1024px) */}
                <aside className="hidden min-h-0 flex-col border-r-2 border-[var(--chalk)] bg-[var(--slate)] lg:flex">
                    <div
                        className="min-h-0 flex-1 overflow-y-auto [-ms-overflow-style:none] [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
                        <div
                            className="border-b-2 border-[var(--chalk)] bg-[var(--carbon)] px-3 py-2 font-mono text-[0.58rem] font-black tracking-[0.18em] text-[var(--zinc)] uppercase">
                            工具索引 / 收藏 {favorites.items.length}
                        </div>

                        <FavoritesIndexRailItem active={activeTool === "favorites"} count={favorites.items.length}
                                                onClick={() => setActiveTool("favorites")}/>

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
                    </div>

                    {/* 设置 — 固定在 Rail 底部（含主题/配置/关于） */}
                    <div className="border-t-2 border-[var(--chalk)] bg-[var(--carbon)]">
                        <button
                            className="group grid w-full grid-cols-[0.25rem_1fr_2.25rem] items-stretch text-left transition-colors hover:bg-[var(--slate)] focus:outline-none focus-visible:outline-2 focus-visible:outline-offset-[-2px] focus-visible:outline-[var(--amber)]"
                            onClick={() => setSettingsOpen(true)}
                            type="button"
                        >
                            <span className="bg-[var(--amber)]" aria-hidden="true"/>
                            <span className="min-w-0 px-3 py-3">
                <span className="block truncate text-sm font-black tracking-[-0.02em] uppercase">设置</span>
                <span
                    className="mt-1 block truncate font-mono text-[0.62rem] font-bold tracking-[0.14em] text-[var(--zinc)] uppercase group-hover:text-[var(--chalk)]">
                  SYS / SETTINGS
                </span>
              </span>
                            <span
                                className="flex items-center justify-center border-l border-[var(--chalk)] text-[var(--amber)]">
                <RiSettings3Line className="size-4" aria-hidden="true"/>
              </span>
                        </button>
                    </div>
                </aside>

                <main
                    id="app-content"
                    tabIndex={-1}
                    className="min-h-0 overflow-y-auto bg-transparent focus:outline-none [-ms-overflow-style:none] [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
                >
                    <div
                        key={reloadNonce}
                        className={cn("mx-auto min-h-full w-full px-2 py-2 xl:px-3 xl:py-3", activeTool === "strategy" ? "max-w-none" : "max-w-7xl")}>
                        <GlobalEnabledConsumer activeTool={activeTool}/>
                        <ToolPageSuspense>{renderToolPage(activeTool, highlightCardId, handleFavoritesNavigate)}</ToolPageSuspense>
                    </div>
                </main>
            </div>

            {/* 统一设置 Dialog（主题 / 配置 / 关于） */}
            <ToolPageSuspense fallback={null}>
                <SettingsDialog open={settingsOpen} onOpenChange={setSettingsOpen}/>
            </ToolPageSuspense>
        </div>
    );
}

export default App;

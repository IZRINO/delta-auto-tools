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
import {publishSettingsDialogState} from "@/components/app/settings-dialog-events";
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
    "recognition-overlay",
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
const RecognitionPage = lazy(() =>
    import("@/components/app/recognition-page").then((module) => ({default: module.RecognitionPage})),
);

const RecognitionRegionOverlay = lazy(() =>
    import("@/components/app/recognition-page").then((module) => ({default: module.RecognitionRegionOverlay})),
);

const SettingsDialog = lazy(() =>
    import("@/components/app/settings-page").then((module) => ({default: module.SettingsDialog})),
);

const tools = [
    {id: "timer" as const, icon: RiTimerLine, label: "计时器", short: "Timer"},
    {id: "counter" as const, icon: RiSpeedUpLine, label: "计数器", short: "Counter"},
    {id: "rapidfire" as const, icon: RiGamepadLine, label: "连发器", short: "Rapidfire"},
    {id: "strategy" as const, icon: RiCompassDiscoverLine, label: "攻略网站", short: "Strategy"},
    {id: "recognition" as const, icon: RiVolumeUpLine, label: "识别触发", short: "Recognition"},
];

const deltaTools = [
    {id: "morse" as const, icon: RiRadarLine, label: "摩斯密码解析", short: "Morse"},
];

type ToolId = (typeof tools)[number]["id"] | (typeof deltaTools)[number]["id"] | "favorites";

function ToolPageFallback() {
    return (
        <div className="flex min-h-[360px] items-center justify-center rounded-box bg-base-200 px-6 text-center text-sm text-base-content/70">
            <span className="loading loading-spinner loading-sm mr-2"/>
            正在装载工具面板
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
        case "recognition":
            return <RecognitionPage/>;
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
                "btn btn-ghost btn-sm h-12 min-h-12 shrink-0 flex-col gap-0.5 rounded-none border-b-2 border-transparent px-3 text-xs font-medium",
                active
                    ? "border-primary bg-base-200 text-base-content"
                    : "text-base-content/60 hover:bg-base-200 hover:text-base-content",
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
        <nav className="flex items-center overflow-x-auto border-b border-base-300 bg-base-100 [-ms-overflow-style:none] [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
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
            className={cn(
                "btn btn-ghost h-auto min-h-14 w-full justify-start rounded-box px-3 py-2 text-left",
                active && "btn-active",
            )}
            data-active={active}
            onClick={onClick}
            type="button"
        >
            <Icon className="size-4 shrink-0 text-primary" aria-hidden="true"/>
            <span className="min-w-0">
                <span className="block truncate text-sm font-semibold">{label}</span>
                <span className="mt-0.5 block truncate text-xs font-normal text-base-content/60">
                    {code} / {meta}
                </span>
            </span>
        </button>
    );
}

function IndexRailSection({children, title}: { children: ReactNode; title: string }) {
    return (
        <section>
            <div className="menu-title px-3 pt-3 pb-1 text-xs text-base-content/60">{title}</div>
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
        <div role="alert" className="alert alert-error alert-soft mb-3">
            <RiShutDownLine className="size-4" aria-hidden="true"/>
            <span>全局总开关已关闭。所有自动化功能与热键均已暂停，请在顶部工具栏重新开启。</span>
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
                "join join-horizontal items-center rounded-field border border-base-300 bg-base-100 px-2 py-1 text-xs",
                globalEnabled
                    ? "text-success"
                    : "text-error",
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

    const handleSettingsOpenChange = useCallback((open: boolean) => {
        publishSettingsDialogState(open);
        setSettingsOpen(open);
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

    if (overlayMode === "recognition-overlay") {
        return (
            <ToolPageSuspense fallback={null}>
                <RecognitionRegionOverlay/>
            </ToolPageSuspense>
        );
    }

    const activeMeta = [...tools, ...deltaTools].find((tool) => tool.id === activeTool);

    return (
        <div className="grid h-dvh min-h-0 grid-rows-[48px_1fr] overflow-hidden bg-base-100">
            <a
                className="sr-only fixed left-4 top-4 z-50 rounded-field bg-base-200 px-3 py-2 text-sm text-base-content focus:not-sr-only focus:outline-2 focus:outline-primary"
                href="#app-content"
            >
                跳到主内容
            </a>

            <header className="navbar min-h-12 border-b border-base-300 bg-base-200 px-3 text-base-content">
                <div className="navbar-start min-w-0 gap-3">
                    <div className="flex size-9 items-center justify-center rounded-field bg-primary text-primary-content">
                        <RiCrosshair2Line className="size-5" aria-hidden="true"/>
                    </div>
                    <div className="min-w-0 leading-none">
                        <p className="truncate text-sm font-semibold">三角洲行动工具</p>
                        <p className="mt-0.5 truncate text-xs text-base-content/60">Delta Auto Tools</p>
                    </div>
                </div>
                <div className="navbar-center hidden min-w-0 items-center gap-3 lg:flex">
                    <span className="badge badge-primary badge-sm">
                        {activeTool === "favorites" ? "PIN" : activeTool === "timer" ? "01" : activeTool === "counter" ? "02" : activeTool === "rapidfire" ? "03" : activeTool === "strategy" ? "04" : activeTool === "recognition" ? "05" : "D1"}
                    </span>
                        <div className="min-w-0">
                            <p className="truncate text-sm font-medium">
                                {activeMeta?.label ?? "收藏夹"}
                            </p>
                            <p className="mt-0.5 truncate text-xs text-base-content/60">
                                {activeMeta?.short ?? "Pinned"} / ACTIVE
                            </p>
                        </div>
                </div>
                <div className="navbar-end gap-2">
                        <ProfileSwitcher/>
                        <GlobalSwitch/>
                        <span className="badge badge-neutral hidden sm:inline-flex">Tauri</span>
                </div>
            </header>

            {/* Top Tab Bar (mobile/tablet <1024px) */}
            <div className="lg:hidden">
                <TopTabBar activeTool={activeTool} onToolClick={setActiveTool}/>
            </div>

            <div className="grid min-h-0 grid-cols-1 overflow-hidden lg:grid-cols-[240px_minmax(0,1fr)]">
                {/* Left Index Rail (desktop >=1024px) */}
                <aside className="hidden min-h-0 flex-col border-r border-base-300 bg-base-200 lg:flex">
                    <div
                        className="menu min-h-0 flex-1 flex-nowrap overflow-y-auto p-2 [-ms-overflow-style:none] [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
                        <div className="menu-title px-3 pb-1 text-xs text-base-content/60">
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
                    <div className="border-t border-base-300 bg-base-200 p-2">
                        <button
                            className="btn btn-ghost h-auto min-h-14 w-full justify-start rounded-box px-3 py-2 text-left"
                            onClick={() => handleSettingsOpenChange(true)}
                            type="button"
                        >
                            <RiSettings3Line className="size-4 shrink-0 text-primary" aria-hidden="true"/>
                            <span className="min-w-0">
                                <span className="block truncate text-sm font-semibold">设置</span>
                                <span className="mt-0.5 block truncate text-xs font-normal text-base-content/60">
                                    SYS / SETTINGS
                                </span>
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
                <SettingsDialog open={settingsOpen} onOpenChange={handleSettingsOpenChange}/>
            </ToolPageSuspense>
        </div>
    );
}

export default App;

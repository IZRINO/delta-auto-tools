import {lazy, type ReactNode, Suspense, useCallback, useEffect, useMemo, useRef, useState} from "react";
import {RiShutDownLine} from "@remixicon/react";

import {FavoritesProvider} from "@/hooks/use-favorites";
import {GlobalEnabledProvider, useGlobalEnabled} from "@/hooks/use-global-enabled";
import {ThemeProvider, useTheme} from "@/hooks/use-theme";
import {ProfileProvider} from "@/hooks/use-profile";
import {PagePreviewBanner, SoftAlert} from "@/components/app/app-ui";
import {BlackmarkSettingsPage} from "@/components/app/blackmark-settings-page";
import {BlackmarkShell, isToolId} from "@/components/app/blackmark-shell";
import {ConsoleShell} from "@/components/app/console-shell";
import type {FavoriteCardKind} from "@/components/app/favorites-utils";
import {publishSettingsDialogState} from "@/components/app/settings-dialog-events";
import type {ToolId} from "@/components/app/tool-nav";
import {useNativeShell} from "@/hooks/use-native-shell";
import {isOverlayWindowMode} from "@/lib/overlay-windows";

import "./App.css";

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
const SpecialOpsPage = lazy(() =>
    import("@/components/app/special-ops-page").then((module) => ({default: module.SpecialOpsPage})),
);
const PrivacyScreenPage = lazy(() =>
    import("@/components/app/privacy-screen-page").then((module) => ({default: module.PrivacyScreenPage})),
);
const SpecialOpsCalibrationOverlay = lazy(() =>
    import("@/components/app/special-ops-calibration-overlay").then((module) => ({default: module.SpecialOpsCalibrationOverlay})),
);
const SpecialOpsOperationOverlay = lazy(() =>
    import("@/components/app/special-ops-operation-overlay").then((module) => ({default: module.SpecialOpsOperationOverlay})),
);
const RecognitionRegionOverlay = lazy(() =>
    import("@/components/app/recognition-page").then((module) => ({default: module.RecognitionRegionOverlay})),
);

function ToolPageFallback() {
    return (
        <div className="flex min-h-[360px] items-center justify-center rounded-box bg-base-200 px-6 text-center text-sm text-base-content/70">
            <span className="loading loading-spinner loading-sm mr-2"/>
            正在装载工具面板
        </div>
    );
}

function ToolPageSuspense({children, fallback = <ToolPageFallback/>}: {children: ReactNode; fallback?: ReactNode}) {
    return <Suspense fallback={fallback}>{children}</Suspense>;
}

type ToolHighlight = {kind: "timer"; cardId: string; nonce: number} | {
    kind: "counter";
    cardId: string;
    nonce: number;
} | {kind: "rapidfire"; cardId: string; nonce: number} | null;

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
        case "specialOps":
            return <SpecialOpsPage/>;
        case "privacyScreen":
            return <PrivacyScreenPage/>;
    }
}

function GlobalDisabledBanner() {
    return (
        <SoftAlert className="mb-3">
            <RiShutDownLine className="size-4" aria-hidden="true"/>
            <span>全局已关闭</span>
        </SoftAlert>
    );
}

function GlobalEnabledConsumer({activeTool}: {activeTool: ToolId}) {
    const {globalEnabled} = useGlobalEnabled();
    const {uiWorld} = useTheme();
    if (globalEnabled || activeTool === "strategy") return null;
    if (uiWorld === "blackmark") {
        return (
            <div className="bm-alert mb-0 mt-6" data-tone="warning">
                全局已关闭
            </div>
        );
    }
    return <GlobalDisabledBanner/>;
}

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
    const [settingsPane, setSettingsPane] = useState(false);
    const isNativeShell = useNativeShell();
    const overlayWindow = isOverlayWindowMode(overlayMode);
    const {uiWorld} = useTheme();

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
        setSettingsPane(false);
    }, []);

    const handleSettingsOpenChange = useCallback((open: boolean) => {
        publishSettingsDialogState(open);
        setSettingsOpen(open);
    }, []);

    useEffect(() => {
        if (!overlayWindow) return;
        document.documentElement.dataset.overlayMode = "true";
        document.body.dataset.overlayMode = "true";
        return () => {
            delete document.documentElement.dataset.overlayMode;
            delete document.body.dataset.overlayMode;
        };
    }, [overlayWindow]);

    useEffect(() => {
        setSettingsPane(false);
        handleSettingsOpenChange(false);
    }, [handleSettingsOpenChange, uiWorld]);

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

    if (overlayMode === "special-ops-calibration") {
        return <ToolPageSuspense fallback={null}><SpecialOpsCalibrationOverlay/></ToolPageSuspense>;
    }

    if (overlayMode === "special-ops-operation") {
        return <ToolPageSuspense fallback={null}><SpecialOpsOperationOverlay/></ToolPageSuspense>;
    }

    const toolPage = (
        <>
            {isNativeShell ? null : <div className="mb-2"><PagePreviewBanner/></div>}
            <GlobalEnabledConsumer activeTool={activeTool}/>
            <ToolPageSuspense>{renderToolPage(activeTool, highlightCardId, handleFavoritesNavigate)}</ToolPageSuspense>
        </>
    );

    if (uiWorld === "blackmark") {
        return (
            <BlackmarkShell
                activePane={settingsPane ? "settings" : activeTool}
                onPaneChange={(pane) => {
                    if (isToolId(pane)) {
                        setSettingsPane(false);
                        setActiveTool(pane);
                        return;
                    }
                    setSettingsPane(true);
                }}
            >
                {settingsPane ? <BlackmarkSettingsPage/> : toolPage}
            </BlackmarkShell>
        );
    }

    return (
        <ConsoleShell
            activeTool={activeTool}
            onSettingsOpenChange={handleSettingsOpenChange}
            onToolClick={setActiveTool}
            settingsOpen={settingsOpen}
        >
            {toolPage}
        </ConsoleShell>
    );
}

export default App;

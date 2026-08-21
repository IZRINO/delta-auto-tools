import {type ReactNode, Suspense, lazy} from "react";
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
    RiEyeOffLine,
    RiShieldLine,
} from "@remixicon/react";

import {useFavorites} from "@/hooks/use-favorites";
import {useGlobalEnabled} from "@/hooks/use-global-enabled";
import {useProfile} from "@/hooks/use-profile";
import {ProfileSwitcher} from "@/components/app/profile-switcher";
import type {ToolId} from "@/components/app/tool-nav";
import {Switch} from "@/components/ui/switch";
import {cn} from "@/lib/utils";

const SettingsDialog = lazy(() =>
    import("@/components/app/settings-page").then((module) => ({default: module.SettingsDialog})),
);

const tools = [
    {id: "timer" as const, icon: RiTimerLine, label: "计时器", short: "Timer"},
    {id: "counter" as const, icon: RiSpeedUpLine, label: "计数器", short: "Counter"},
    {id: "rapidfire" as const, icon: RiGamepadLine, label: "连发器", short: "Rapidfire"},
    {id: "strategy" as const, icon: RiCompassDiscoverLine, label: "攻略网站", short: "Strategy"},
    {id: "recognition" as const, icon: RiVolumeUpLine, label: "识别触发", short: "Recognition"},
    {id: "privacyScreen" as const, icon: RiEyeOffLine, label: "息屏", short: "Privacy"},
];

const deltaTools = [
    {id: "specialOps" as const, icon: RiShieldLine, label: "特勤处", short: "Special Ops"},
    {id: "morse" as const, icon: RiRadarLine, label: "摩斯密码解析", short: "Morse"},
];

type ConsoleShellProps = {
    activeTool: ToolId;
    children: ReactNode;
    onSettingsOpenChange: (open: boolean) => void;
    onToolClick: (id: ToolId) => void;
    settingsOpen: boolean;
};

export function ConsoleShell({
    activeTool,
    children,
    onSettingsOpenChange,
    onToolClick,
    settingsOpen,
}: ConsoleShellProps) {
    const favorites = useFavorites();
    const {reloadNonce} = useProfile();

    return (
        <div className="grid h-dvh min-h-0 grid-rows-[48px_1fr] overflow-hidden bg-base-100 max-lg:grid-rows-[48px_auto_1fr]">
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
                    <p className="truncate text-sm font-semibold">三角洲行动工具</p>
                </div>
                <div className="navbar-end gap-2">
                    <ProfileSwitcher/>
                    <GlobalSwitch/>
                </div>
            </header>

            <div className="lg:hidden">
                <TopTabBar activeTool={activeTool} onToolClick={onToolClick}/>
            </div>

            <div className="grid min-h-0 grid-cols-1 overflow-hidden lg:grid-cols-[240px_minmax(0,1fr)]">
                <aside aria-label="工具导航" className="hidden min-h-0 flex-col border-r border-base-300 bg-base-200 lg:flex">
                    <div
                        className="menu min-h-0 flex-1 flex-nowrap overflow-y-auto p-2 [-ms-overflow-style:none] [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
                        <FavoritesIndexRailItem
                            active={activeTool === "favorites"}
                            count={favorites.items.length}
                            onClick={() => onToolClick("favorites")}
                        />

                        <IndexRailSection title="通用工具">
                            {tools.map((tool) => (
                                <IndexRailItem
                                    active={activeTool === tool.id}
                                    icon={tool.icon}
                                    key={tool.id}
                                    label={tool.label}
                                    onClick={() => onToolClick(tool.id)}
                                />
                            ))}
                        </IndexRailSection>

                        <IndexRailSection title="三角洲工具">
                            {deltaTools.map((tool) => (
                                <IndexRailItem
                                    active={activeTool === tool.id}
                                    icon={tool.icon}
                                    key={tool.id}
                                    label={tool.label}
                                    onClick={() => onToolClick(tool.id)}
                                />
                            ))}
                        </IndexRailSection>
                    </div>

                    <div className="border-t border-base-300 bg-base-200 p-2">
                        <button
                            className="btn btn-ghost h-10 min-h-10 w-full justify-start rounded-box px-3"
                            onClick={() => onSettingsOpenChange(true)}
                            type="button"
                        >
                            <RiSettings3Line className="size-4 shrink-0" aria-hidden="true"/>
                            <span className="truncate text-sm font-semibold">设置</span>
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
                        className={cn("mx-auto min-h-full w-full px-2 py-2 xl:px-3 xl:py-3", activeTool === "strategy" ? "max-w-none" : "max-w-7xl")}
                    >
                        {children}
                    </div>
                </main>
            </div>

            <Suspense fallback={null}>
                <SettingsDialog open={settingsOpen} onOpenChange={onSettingsOpenChange}/>
            </Suspense>
        </div>
    );
}

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
            aria-current={active ? "page" : undefined}
            onClick={onClick}
            type="button"
        >
            <Icon className="size-4" aria-hidden="true"/>
            <span className="truncate">{label}</span>
        </button>
    );
}

function TopTabBar({activeTool, onToolClick}: {activeTool: ToolId; onToolClick: (id: ToolId) => void}) {
    return (
        <nav aria-label="工具导航" className="flex items-center overflow-x-auto border-b border-base-300 bg-base-100 [-ms-overflow-style:none] [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
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

function IndexRailItem({
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
                "btn btn-ghost h-10 min-h-10 w-full justify-start rounded-box px-3",
                active && "btn-active",
            )}
            aria-current={active ? "page" : undefined}
            data-active={active}
            onClick={onClick}
            type="button"
        >
            <Icon className="size-4 shrink-0" aria-hidden="true"/>
            <span className="truncate text-sm font-semibold">{label}</span>
        </button>
    );
}

function IndexRailSection({children, title}: {children: ReactNode; title: string}) {
    return (
        <section>
            <div className="menu-title px-3 pt-3 pb-1 text-xs text-base-content/60">{title}</div>
            <div>{children}</div>
        </section>
    );
}

function FavoritesIndexRailItem({active, count, onClick}: {active: boolean; count: number; onClick: () => void}) {
    return (
        <IndexRailItem
            active={active}
            icon={RiStarFill}
            label={count > 0 ? `收藏夹 ${count}` : "收藏夹"}
            onClick={onClick}
        />
    );
}

function GlobalSwitch() {
    const {globalEnabled, setGlobalEnabled} = useGlobalEnabled();
    return (
        <div
            className={cn(
                "join join-horizontal items-center rounded-field border border-base-300 bg-base-100 px-2 py-1 text-xs",
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

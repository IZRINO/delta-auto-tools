import {Fragment, type ReactNode} from "react";
import {RiShutDownLine} from "@remixicon/react";

import {BlackmarkGlyph} from "@/components/app/blackmark-glyphs";
import {ProfileSwitcher} from "@/components/app/profile-switcher";
import {BLACKMARK_DOCK_GROUPS, type BlackmarkPaneId, type ToolId} from "@/components/app/tool-nav";
import {Tooltip, TooltipContent, TooltipTrigger} from "@/components/ui/tooltip";
import {useGlobalEnabled} from "@/hooks/use-global-enabled";
import {useTheme} from "@/hooks/use-theme";

import "@/blackmark-shell.css";

type BlackmarkShellProps = {
    activePane: BlackmarkPaneId;
    children: ReactNode;
    onPaneChange: (pane: BlackmarkPaneId) => void;
};

export function BlackmarkShell({activePane, children, onPaneChange}: BlackmarkShellProps) {
    const {globalEnabled, setGlobalEnabled} = useGlobalEnabled();
    const {uiScheme} = useTheme();

    return (
        <div className="bm-stage flex h-dvh flex-col overflow-hidden" data-scheme={uiScheme}>
            <a
                className="sr-only fixed left-4 top-4 z-50 bg-[var(--bm-surface)] px-3 py-2 text-sm text-[var(--bm-ink)] focus:not-sr-only focus:outline-2 focus:outline-[var(--bm-ink)]"
                href="#app-content"
            >
                跳到主内容
            </a>
            <div className="bm-content flex min-h-0 flex-1 flex-col">
                <header className="navbar bm-header h-16 min-h-16 px-5">
                    <div className="navbar-start min-w-0 gap-3">
                        <span className="bm-stripe-v h-8 w-1 shrink-0" aria-hidden="true"/>
                        <p className="truncate text-sm font-bold uppercase tracking-[0.18em]">
                            三角洲行动工具
                        </p>
                    </div>
                    <div className="navbar-end gap-4">
                        <ProfileSwitcher/>
                        <label className="flex items-center gap-2 text-xs font-bold tracking-[0.12em] uppercase">
                            <RiShutDownLine className="size-3.5" aria-hidden="true"/>
                            {globalEnabled ? "全局开" : "全局关"}
                            <input
                                aria-label="全局总开关"
                                checked={globalEnabled}
                                className="toggle toggle-sm"
                                onChange={(event) => setGlobalEnabled(event.target.checked)}
                                type="checkbox"
                            />
                        </label>
                    </div>
                </header>
                <div className="bm-stripe h-1 w-full" aria-hidden="true"/>

                <main
                    className="min-h-0 flex-1 overflow-y-auto pb-28 focus:outline-none"
                    id="app-content"
                    tabIndex={-1}
                >
                    {children}
                </main>

                <nav aria-label="工具" className="dock bm-dock">
                    {BLACKMARK_DOCK_GROUPS.map((group, index) => (
                        <Fragment key={group[0].id}>
                            {index > 0 ? <span className="bm-dock-rule" aria-hidden="true"/> : null}
                            {group.map((item) => (
                                <DockItem
                                    active={item.id === activePane}
                                    id={item.id}
                                    key={item.id}
                                    label={item.label}
                                    onSelect={() => onPaneChange(item.id)}
                                />
                            ))}
                        </Fragment>
                    ))}
                    <span className="bm-dock-rule" aria-hidden="true"/>
                    <DockItem
                        active={activePane === "settings"}
                        id="settings"
                        label="设置"
                        onSelect={() => onPaneChange("settings")}
                    />
                </nav>
            </div>
        </div>
    );
}

function DockItem({
    active,
    id,
    label,
    onSelect,
}: {
    active: boolean;
    id: BlackmarkPaneId;
    label: string;
    onSelect: () => void;
}) {
    return (
        <Tooltip delayDuration={180} disableHoverableContent open={active ? false : undefined}>
            <TooltipTrigger asChild>
                <button
                    aria-current={active ? "page" : undefined}
                    aria-label={label}
                    className={active ? "dock-active" : undefined}
                    onClick={onSelect}
                    type="button"
                >
                    <BlackmarkGlyph id={id}/>
                    <span className="dock-label">{label}</span>
                </button>
            </TooltipTrigger>
            <TooltipContent className="bm-dock-tip" side="top" sideOffset={10}>
                {label}
            </TooltipContent>
        </Tooltip>
    );
}

export function isToolId(pane: BlackmarkPaneId): pane is ToolId {
    return pane !== "settings";
}

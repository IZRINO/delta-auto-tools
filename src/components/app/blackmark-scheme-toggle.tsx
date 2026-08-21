import {Tooltip, TooltipContent, TooltipTrigger} from "@/components/ui/tooltip";
import {useTheme} from "@/hooks/use-theme";

export function BlackmarkSchemeToggle() {
    const {uiScheme, setUiScheme} = useTheme();
    const maskId = "bm-scheme-moon-cut";
    const next = uiScheme === "night" ? "day" : "night";
    const label = next === "day" ? "切换日间" : "切换夜航";

    return (
        <Tooltip delayDuration={180} disableHoverableContent>
            <TooltipTrigger asChild>
                <button
                    aria-label={label}
                    className="bm-scheme-toggle"
                    data-scheme={uiScheme}
                    onClick={() => setUiScheme(next)}
                    type="button"
                >
                    <svg aria-hidden="true" fill="none" viewBox="0 0 24 24">
                        <defs>
                            <mask id={maskId}>
                                <rect fill="white" height="24" width="24"/>
                                <circle className="bm-scheme-cut" cx="18" cy="8" fill="black" r="6.5"/>
                            </mask>
                        </defs>
                        <g className="bm-scheme-rays">
                            <path d="M12 2.6v2.3"/>
                            <path d="M12 19.1v2.3"/>
                            <path d="M4.9 4.9l1.6 1.6"/>
                            <path d="M17.5 17.5l1.6 1.6"/>
                            <path d="M2.6 12h2.3"/>
                            <path d="M19.1 12h2.3"/>
                            <path d="M4.9 19.1l1.6-1.6"/>
                            <path d="M17.5 6.5l1.6-1.6"/>
                        </g>
                        <circle
                            className="bm-scheme-core"
                            cx="12"
                            cy="12"
                            fill="currentColor"
                            mask={`url(#${maskId})`}
                            r="5"
                        />
                    </svg>
                </button>
            </TooltipTrigger>
            <TooltipContent className="bm-dock-tip" side="bottom" sideOffset={8}>
                {next === "day" ? "日间" : "夜航"}
            </TooltipContent>
        </Tooltip>
    );
}

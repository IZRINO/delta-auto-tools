import type {ToolId} from "@/components/app/tool-nav";

export const ACTIVE_TOOL_STORAGE_KEY = "delta-auto-tools:active-tool:v1";

export const ACTIVE_TOOL_IDS = [
    "timer",
    "counter",
    "rapidfire",
    "strategy",
    "recognition",
    "privacyScreen",
    "specialOps",
    "morse",
    "favorites",
] as const satisfies readonly ToolId[];

export function parseActiveTool(raw: string | null): ToolId | null {
    if (raw && (ACTIVE_TOOL_IDS as readonly string[]).includes(raw)) {
        return raw as ToolId;
    }
    return null;
}

export function readStoredActiveTool(): ToolId | null {
    if (typeof window === "undefined") return null;
    try {
        return parseActiveTool(window.localStorage.getItem(ACTIVE_TOOL_STORAGE_KEY));
    } catch {
        return null;
    }
}

export function writeStoredActiveTool(id: ToolId): void {
    if (typeof window === "undefined") return;
    try {
        window.localStorage.setItem(ACTIVE_TOOL_STORAGE_KEY, id);
    } catch {
        // ponytail: quota/privacy — 只留内存态
    }
}

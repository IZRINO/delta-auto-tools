export const BLACKMARK_DOCK_TOOLS = [
    {id: "favorites", label: "收藏"},
    {id: "timer", label: "计时器"},
    {id: "counter", label: "计数器"},
    {id: "rapidfire", label: "连发器"},
    {id: "strategy", label: "攻略"},
    {id: "recognition", label: "识别"},
    {id: "privacyScreen", label: "息屏"},
    {id: "specialOps", label: "特勤处"},
    {id: "morse", label: "摩斯"},
] as const;

export type ToolId = (typeof BLACKMARK_DOCK_TOOLS)[number]["id"];

export type BlackmarkPaneId = ToolId | "settings";

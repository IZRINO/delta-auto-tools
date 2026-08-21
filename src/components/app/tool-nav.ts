/** 底栏分组：收藏 | 局内辅助 | 工作台 | 三角洲。设置单独竖线隔开。 */
export const BLACKMARK_DOCK_GROUPS = [
    [{id: "favorites", label: "收藏"}],
    [
        {id: "timer", label: "计时器"},
        {id: "counter", label: "计数器"},
        {id: "rapidfire", label: "连发器"},
    ],
    [
        {id: "strategy", label: "攻略"},
        {id: "recognition", label: "识别"},
        {id: "privacyScreen", label: "息屏"},
    ],
    [
        {id: "specialOps", label: "特勤处"},
        {id: "morse", label: "摩斯"},
    ],
] as const;

export const BLACKMARK_DOCK_TOOLS = BLACKMARK_DOCK_GROUPS.flat();

export type ToolId = (typeof BLACKMARK_DOCK_TOOLS)[number]["id"];

export type BlackmarkPaneId = ToolId | "settings";

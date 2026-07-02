import {describe, expect, it} from "vitest";
import rapidfirePageSource from "./rapidfire-page.tsx?raw";

/**
 * 验证 rapidfire-page ChannelTabs tab 切换逻辑（VAL-DF-006）。
 *
 * onTabChange 必须是非空函数，切换 tab 时更新当前激活 tab 状态并条件渲染对应内容。
 */
describe("rapidfire ChannelTabs tab 切换", () => {
    it("onTabChange 不是空箭头函数", () => {
        // 不应包含 onTabChange={() => {}} 或 onTabChange={() =>{}}
        expect(rapidfirePageSource).not.toMatch(/onTabChange=\{\(\)\s*=>\s*\{\s*\}\}/);
        expect(rapidfirePageSource).not.toMatch(/onTabChange=\{\(\)\s*=>\s*\}/);
    });

    it("存在 useState 管理 active tab 状态", () => {
        expect(rapidfirePageSource).toMatch(/useState<["']cards["']\s*\|\s*["']global["']\s*\|\s*["']display["']>/);
    });

    it("onTabChange 调用 setActiveTab", () => {
        expect(rapidfirePageSource).toMatch(/onTabChange.*setActiveTab/);
    });

    it("tabs active 字段动态计算（使用 activeTab ===）", () => {
        expect(rapidfirePageSource).toMatch(/active:\s*activeTab\s*===/);
    });

    it("根据 activeTab 条件渲染 cards 内容", () => {
        expect(rapidfirePageSource).toMatch(/activeTab\s*===\s*["']cards["']/);
    });

    it("根据 activeTab 条件渲染 global 内容", () => {
        expect(rapidfirePageSource).toMatch(/activeTab\s*===\s*["']global["']/);
    });

    it("根据 activeTab 条件渲染 display 内容", () => {
        expect(rapidfirePageSource).toMatch(/activeTab\s*===\s*["']display["']/);
    });
});

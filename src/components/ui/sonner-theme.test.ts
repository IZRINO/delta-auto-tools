import {describe, it, expect } from "vitest";
import sonnerSource from "./sonner.tsx?raw";

/**
 * 验证 sonner Toaster 主题不为 light（VAL-CF-010）。
 *
 * 本项目暗色唯一模式，sonner 必须硬编码 theme="dark"，
 * 不能依赖 next-themes 的 useTheme（已移除）。
 */
describe("sonner Toaster theme", () => {
    it("不含 next-themes import", () => {
        expect(sonnerSource).not.toContain("next-themes");
        expect(sonnerSource).not.toContain('from "next-themes"');
    });

    it("Toaster theme 硬编码为 dark，非 light", () => {
        // 确认 theme 属性值为 "dark"
        expect(sonnerSource).toMatch(/theme="dark"/);
        // 确认不含 theme="light" 或 theme="system"
        expect(sonnerSource).not.toMatch(/theme="light"/);
        expect(sonnerSource).not.toMatch(/theme="system"/);
    });

    it("不使用 useTheme hook 控制 theme 属性", () => {
        // 源码中不应有从 next-themes 导入的 useTheme
        expect(sonnerSource).not.toContain("useTheme");
    });

    it("{...props} JSX spread 只出现一次，防止底部 spread 覆盖 theme='dark'", () => {
        // 统计源码中 JSX {...props} spread 出现次数
        // 排除函数参数解构 ({...props}: ToasterProps)，只匹配 JSX 属性 spread
        const jsxSpreadMatches = sonnerSource.match(/<(?:Sonner|\w)[^>]*\{...\w+\}/gs);
        expect(jsxSpreadMatches).not.toBeNull();
        expect(jsxSpreadMatches!.length).toBe(1);
    });
});

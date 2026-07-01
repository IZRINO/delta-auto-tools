import {describe, it, expect } from "vitest";
import mainSource from "../../main.tsx?raw";

/**
 * 验证 main.tsx 不含 next-themes ThemeProvider 包裹（VAL-CF-009）。
 */
describe("main.tsx next-themes 移除", () => {
    it("不含 next-themes import", () => {
        expect(mainSource).not.toContain("next-themes");
    });

    it("不含 ThemeProvider JSX", () => {
        expect(mainSource).not.toContain("<ThemeProvider");
        expect(mainSource).not.toContain("</ThemeProvider>");
    });
});

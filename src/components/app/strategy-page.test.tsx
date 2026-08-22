import {describe, expect, it} from "vitest";

import pageSource from "./strategy-page.tsx?raw";
import shellSource from "./blackmark-shell.tsx?raw";

describe("StrategyPage 窗口铺满", () => {
    it("黑标不套英雄标题，主区铺满且外层不与网页双滚动", () => {
        expect(pageSource).not.toContain("内嵌攻略站");
        expect(pageSource).not.toContain("BlackmarkPage");
        expect(pageSource).not.toContain("min-h-[calc(100dvh-18rem)]");
        expect(pageSource).toContain("min-h-[calc(100dvh-4rem)]");
        expect(pageSource).toContain("grid-rows-[auto_minmax(0,1fr)]");
        expect(shellSource).toContain('activePane === "strategy"');
        expect(shellSource).toContain("pb-[5.75rem]");
        expect(shellSource).toContain("overflow-y-auto pb-36 scroll-pb-36");
    });

    it("站点索引与内嵌容器仍在同一页", () => {
        expect(pageSource).toContain("contentHostRef");
        expect(pageSource).toContain("strategy-content");
        expect(pageSource).toContain("新增");
        expect(pageSource).toContain("<h1 className=\"text-sm font-semibold\">攻略</h1>");
    });
});

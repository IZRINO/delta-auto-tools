import {describe, expect, it} from "vitest";

import profitFilterSource from "./special-ops-profit-filter.tsx?raw";

describe("SpecialOpsProfitFilter", () => {
    it("默认折叠利润规则编辑表", () => {
        expect(profitFilterSource).toContain('<summary className="cursor-pointer px-4 py-3 font-medium">利润规则</summary>');
    });

    it("默认折叠业务目标绑定表", () => {
        expect(profitFilterSource).toContain('<details className="rounded-box border border-base-300">');
        expect(profitFilterSource).toContain('<summary className="cursor-pointer px-4 py-3 font-medium">业务目标</summary>');
    });

    it("联网利润查询只保留 Moligod", () => {
        expect(profitFilterSource).toContain("Moligod 精确名称");
        expect(profitFilterSource).not.toContain("KKRB 精确名称");
        expect(profitFilterSource).not.toContain("刷新 KKRB 名称");
        expect(profitFilterSource).not.toContain("special_ops_fetch_profit_catalog");
    });
});

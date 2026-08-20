import {describe, expect, it} from "vitest";

import {formatProfitCatalogError} from "./special-ops-profit-filter";
import profitFilterSource from "./special-ops-profit-filter.tsx?raw";

describe("SpecialOpsProfitFilter", () => {
    it("默认折叠利润规则编辑表", () => {
        expect(profitFilterSource).toContain('<summary className="cursor-pointer px-4 py-3 font-medium">利润规则</summary>');
    });

    it("默认折叠业务目标绑定表", () => {
        expect(profitFilterSource).toContain('<details className="rounded-box border border-base-300">');
        expect(profitFilterSource).toContain('<summary className="cursor-pointer px-4 py-3 font-medium">业务目标</summary>');
    });

    it("KKRB -101 提示可手工填写精确名称", () => {
        expect(formatProfitCatalogError("KKRB 返回失败（code -101）：系统繁忙，请稍后再试")).toBe(
            "KKRB 暂时繁忙，名称列表未更新。可直接手工填写并保存“KKRB 精确名称”。",
        );
    });
});

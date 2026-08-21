import {describe, expect, it} from "vitest";

import {
    ACTIVE_TOOL_IDS,
    parseActiveTool,
} from "@/components/app/active-tool";

describe("parseActiveTool", () => {
    it("九个合法 id 原样返回", () => {
        expect(ACTIVE_TOOL_IDS).toEqual([
            "timer",
            "counter",
            "rapidfire",
            "strategy",
            "recognition",
            "privacyScreen",
            "specialOps",
            "morse",
            "favorites",
        ]);
        for (const id of ACTIVE_TOOL_IDS) {
            expect(parseActiveTool(id)).toBe(id);
        }
    });

    it("null、空串、未知值返回空，交给壳层默认", () => {
        expect(parseActiveTool(null)).toBeNull();
        expect(parseActiveTool("")).toBeNull();
        expect(parseActiveTool("nope")).toBeNull();
        expect(parseActiveTool("Timer")).toBeNull();
    });
});

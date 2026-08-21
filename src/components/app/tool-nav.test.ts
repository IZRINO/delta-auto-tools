import {describe, expect, it} from "vitest";

import {BLACKMARK_DOCK_GROUPS, BLACKMARK_DOCK_TOOLS} from "@/components/app/tool-nav";

describe("blackmark dock groups", () => {
    it("四组加设置竖线：收藏、局内、工作台、三角洲", () => {
        expect(BLACKMARK_DOCK_GROUPS.map((group) => group.map((item) => item.id))).toEqual([
            ["favorites"],
            ["timer", "counter", "rapidfire"],
            ["strategy", "recognition", "privacyScreen"],
            ["specialOps", "morse"],
        ]);
        expect(BLACKMARK_DOCK_TOOLS.map((item) => item.id)).toEqual([
            "favorites",
            "timer",
            "counter",
            "rapidfire",
            "strategy",
            "recognition",
            "privacyScreen",
            "specialOps",
            "morse",
        ]);
    });
});

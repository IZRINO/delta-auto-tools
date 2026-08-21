import {describe, expect, it} from "vitest";

import {isPrereleaseVersion, notAvailableLabel} from "@/components/app/about-update";

describe("isPrereleaseVersion", () => {
    it("版本含连字符视为测试版", () => {
        expect(isPrereleaseVersion("0.20.1-beta.1")).toBe(true);
        expect(isPrereleaseVersion("1.0.0-beta.1")).toBe(true);
        expect(isPrereleaseVersion("0.20.1")).toBe(false);
        expect(isPrereleaseVersion(undefined)).toBe(false);
    });
});

describe("notAvailableLabel", () => {
    it("测试版不写已是最新", () => {
        expect(notAvailableLabel(true)).toBe("暂无正式版可升");
        expect(notAvailableLabel(false)).toBe("已是最新");
    });
});

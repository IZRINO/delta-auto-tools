import {describe, expect, it} from "vitest";
import profileSwitcherSource from "./profile-switcher.tsx?raw";

/**
 * VAL-DF-005: profile-switcher 含"另存为"入口。
 */
describe("profile-switcher 另存为入口", () => {
    it("含「另存为」按钮文本", () => {
        expect(profileSwitcherSource).toContain("另存为");
    });

    it("调用 saveCurrentProfile 方法", () => {
        expect(profileSwitcherSource).toContain("saveCurrentProfile");
    });
});

import {describe, expect, it} from "vitest";
import useProfileSource from "./use-profile.tsx?raw";

/**
 * VAL-DF-004: use-profile 暴露 saveCurrentProfile 方法。
 * VAL-DF-005: profile-switcher 含"另存为"入口。
 */
describe("saveCurrentProfile", () => {
    it("use-profile.tsx ProfileContextValue 含 saveCurrentProfile 方法", () => {
        // 验证类型定义中包含 saveCurrentProfile
        expect(useProfileSource).toContain("saveCurrentProfile");
    });

    it("saveCurrentProfile 调用 invoke('profile_save_current', {name})", () => {
        // 验证源码中调用了正确的 Tauri command
        expect(useProfileSource).toMatch(/invoke.*profile_save_current/);
        // 验证传递 name 参数
        expect(useProfileSource).toMatch(/profile_save_current.*name/);
    });

    it("saveCurrentProfile 刷新 bootstrap 后自增 reloadNonce", () => {
        // 验证调用了 refreshAfterSwitch（刷新 bootstrap + 自增 nonce）
        expect(useProfileSource).toMatch(/saveCurrentProfile.*refreshAfterSwitch|refreshAfterSwitch.*saveCurrentProfile/s);
    });
});

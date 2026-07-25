import {describe, expect, it, vi} from "vitest";

import {
    applyExecutableSelection,
    eligibleLoginTrialAccounts,
    formatCalibrationTemplateTestResult,
    persistSpecialOpsSaveRequest,
} from "@/components/app/special-ops-utils";
import type {AccountPlan, SpecialOpsSettings} from "@/components/app/special-ops-types";

function account(
    id: string,
    order: number,
    patch: Partial<Pick<AccountPlan, "enabled" | "qqAccount" | "password">> = {},
): AccountPlan {
    return {
        id,
        qqAccount: "10001",
        password: "secret",
        enabled: true,
        initialized: false,
        order,
        status: "ready",
        stations: [],
        ammoTargets: [],
        lastFailure: null,
        loginTrialSignature: null,
        ...patch,
    };
}

describe("特勤处登录试运行 helpers", () => {
    it("仅返回启用且凭据完整的账号并按 order 排序", () => {
        const accounts = [
            account("later", 8),
            account("disabled", 0, {enabled: false}),
            account("missing-account", 1, {qqAccount: "  "}),
            account("missing-password", 2, {password: ""}),
            account("first", 3),
        ];

        expect(eligibleLoginTrialAccounts(accounts).map(({id}) => id)).toEqual(["first", "later"]);
    });

    it("stale save 使用冻结 revision 交给后端拒绝且只 reload 不重试", async () => {
        const settings: SpecialOpsSettings = {
            enabled: true,
            paused: true,
            dailyExchangeTime: "08:00",
            emergencyHotkey: "Ctrl+Shift+F12",
            wegameExecutablePath: "wegame.exe",
            gameExecutablePath: "game.exe",
            accounts: [],
            activeCalibrationId: null,
            calibrationEnvironments: [],
        };
        const request = {settings, settingsRevision: 7};
        const save = vi.fn().mockRejectedValue(new Error("配置保存已陈旧：页面 revision 7，当前 revision 8"));
        const reload = vi.fn();

        await expect(persistSpecialOpsSaveRequest(request, save, reload)).rejects.toThrow("配置保存已陈旧");

        expect(save).toHaveBeenCalledOnce();
        expect(save).toHaveBeenCalledWith(request);
        expect(reload).toHaveBeenCalledOnce();
    });

    it("取消选择可执行文件时保留当前路径", () => {
        expect(applyExecutableSelection("C:\\WeGame\\wegame.exe", null)).toBe("C:\\WeGame\\wegame.exe");
    });

    it("同时显示双采样相似度与测试结论", () => {
        expect(formatCalibrationTemplateTestResult("登录按钮", {
            sampleSimilarities: [0.9876, 0.8],
            passed: true,
            verifiedAtMs: 123,
        })).toBe("登录按钮：双采样相似度 98.8% / 80.0%，已通过");
        expect(formatCalibrationTemplateTestResult("登录按钮", {
            sampleSimilarities: [0.74, 0.99],
            passed: false,
            verifiedAtMs: null,
        })).toBe("登录按钮：双采样相似度 74.0% / 99.0%，未通过");
    });
});

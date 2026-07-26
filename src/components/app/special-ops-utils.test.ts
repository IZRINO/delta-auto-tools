import {describe, expect, it, vi} from "vitest";

import {
    applySpecialOpsBootstrapUpdate,
    applyExecutableSelection,
    eligibleLoginTrialAccounts,
    formatCalibrationTemplateTestResult,
    mergeSpecialOpsBootstrap,
    specialOpsErrorAfterUpdate,
    persistSpecialOpsSaveRequest,
} from "@/components/app/special-ops-utils";
import type {
    AccountPlan,
    LoginRunSnapshot,
    SpecialOpsBootstrap,
    SpecialOpsSettings,
} from "@/components/app/special-ops-types";

function account(
    id: string,
    order: number,
    patch: Partial<Pick<AccountPlan, "enabled" | "qqAccount">> = {},
): AccountPlan {
    return {
        id,
        qqAccount: "10001",
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

function runSnapshot(updatedAtMs: number): LoginRunSnapshot {
    return {
        runId: 3,
        accountId: "account-1",
        status: "waiting",
        currentStep: "waitLoginChoice",
        message: `更新于 ${updatedAtMs}`,
        countdownSeconds: null,
        startedAtMs: 1,
        updatedAtMs,
    };
}

function bootstrap(
    settingsRevision: number,
    overrides: Partial<SpecialOpsBootstrap> = {},
): SpecialOpsBootstrap {
    return {
        settings: {
            enabled: true,
            paused: true,
            dailyExchangeTime: "08:00",
            emergencyHotkey: "Ctrl+Shift+F12",
            wegameExecutablePath: "wegame.exe",
            gameExecutablePath: "game.exe",
            accounts: [account("account-1", 0)],
            activeCalibrationId: null,
            calibrationEnvironments: [],
        },
        schedule: {dueAccounts: [], nextWakeAtMs: null},
        settingsRevision,
        nowMs: settingsRevision,
        runSnapshot: null,
        ...overrides,
    };
}

describe("特勤处登录试运行 helpers", () => {
    it("同 revision 的旧 response 不覆盖较新的 runChanged snapshot", () => {
        const current = bootstrap(7, {runSnapshot: runSnapshot(50)});
        const incoming = bootstrap(7, {runSnapshot: runSnapshot(40)});

        expect(mergeSpecialOpsBootstrap(current, incoming).runSnapshot).toBe(current.runSnapshot);
    });

    it("低 revision response 整包不得回退 runtime-managed account fields", () => {
        const runtimeAccount = account("account-1", 0, {});
        runtimeAccount.status = "loginFailed";
        runtimeAccount.lastFailure = {step: "submitLogin", message: "密码错误", atMs: 80};
        runtimeAccount.loginTrialSignature = "runtime-8";
        const current = bootstrap(8, {settings: {...bootstrap(8).settings, accounts: [runtimeAccount]}});
        const incoming = bootstrap(7);

        const merged = mergeSpecialOpsBootstrap(current, incoming);

        expect(merged).toBe(current);
        expect(merged.settings.accounts[0]).toMatchObject({
            status: "loginFailed",
            lastFailure: {step: "submitLogin", message: "密码错误", atMs: 80},
            loginTrialSignature: "runtime-8",
        });
    });

    it("高 revision 应用新 settings 但不回退同一 run 的 snapshot", () => {
        const current = bootstrap(8, {runSnapshot: runSnapshot(50)});
        const incomingBase = bootstrap(9, {runSnapshot: runSnapshot(40)});
        const incoming = {
            ...incomingBase,
            settings: {...incomingBase.settings, dailyExchangeTime: "09:30"},
        };

        const merged = mergeSpecialOpsBootstrap(current, incoming);

        expect(merged.settingsRevision).toBe(9);
        expect(merged.settings.dailyExchangeTime).toBe("09:30");
        expect(merged.runSnapshot).toBe(current.runSnapshot);
    });

    it("runChanged 后的 reload 与 save 旧回包共用排序策略", () => {
        const runtimeAccount = account("account-1", 0);
        runtimeAccount.status = "loginFailed";
        runtimeAccount.lastFailure = {step: "submitLogin", message: "密码错误", atMs: 80};
        runtimeAccount.loginTrialSignature = "runtime-8";
        const current = bootstrap(8, {settings: {...bootstrap(8).settings, accounts: [runtimeAccount]}});
        const afterRunChanged = applySpecialOpsBootstrapUpdate({bootstrap: current, responseSeq: 3}, {
            type: "runChanged",
            snapshot: runSnapshot(50),
        });
        const afterReload = applySpecialOpsBootstrapUpdate(afterRunChanged, {
            type: "bootstrapResponse",
            bootstrap: bootstrap(8, {runSnapshot: runSnapshot(30)}),
            requestSeq: 1,
        });
        const afterSave = applySpecialOpsBootstrapUpdate(afterReload, {
            type: "bootstrapResponse",
            bootstrap: bootstrap(8, {runSnapshot: runSnapshot(40)}),
            requestSeq: 2,
        });

        expect(afterSave.responseSeq).toBe(3);
        expect(afterSave.bootstrap.settingsRevision).toBe(8);
        expect(afterSave.bootstrap.runSnapshot).toBe(afterRunChanged.bootstrap.runSnapshot);
        expect(afterSave.bootstrap.settings.accounts[0]).toMatchObject({
            status: "loginFailed",
            lastFailure: {step: "submitLogin", message: "密码错误", atMs: 80},
            loginTrialSignature: "runtime-8",
        });
    });

    it("运行事件和陈旧回包不清除现有错误", () => {
        const currentError = "配置保存失败";

        expect(specialOpsErrorAfterUpdate(currentError, {
            updateType: "runChanged",
            responseAccepted: false,
            completedCurrentDraft: false,
            dirtyBefore: true,
            revisionChanged: false,
        })).toBe(currentError);
        expect(specialOpsErrorAfterUpdate(currentError, {
            updateType: "bootstrapResponse",
            responseAccepted: false,
            completedCurrentDraft: false,
            dirtyBefore: true,
            revisionChanged: false,
        })).toBe(currentError);
    });

    it("runtime revision 抢占未保存编辑时显示明确警告", () => {
        expect(specialOpsErrorAfterUpdate(null, {
            updateType: "bootstrapResponse",
            responseAccepted: true,
            completedCurrentDraft: false,
            dirtyBefore: true,
            revisionChanged: true,
        })).toBe("运行状态已更新，未保存的编辑已被放弃，请重新检查");
    });

    it("当前草稿保存成功后清除旧错误", () => {
        expect(specialOpsErrorAfterUpdate("上次保存失败", {
            updateType: "bootstrapResponse",
            responseAccepted: true,
            completedCurrentDraft: true,
            dirtyBefore: true,
            revisionChanged: false,
        })).toBeNull();
    });

    it("仅返回启用且 QQ 为纯数字的账号并按 order 排序", () => {
        const accounts = [
            account("later", 8),
            account("disabled", 0, {enabled: false}),
            account("missing-account", 1, {qqAccount: "  "}),
            account("remembered-account", 2),
            account("letters", 4, {qqAccount: "abc123"}),
            account("first", 3),
        ];

        expect(eligibleLoginTrialAccounts(accounts).map(({id}) => id)).toEqual([
            "remembered-account",
            "first",
            "later",
        ]);
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

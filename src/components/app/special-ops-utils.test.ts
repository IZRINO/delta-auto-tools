import {describe, expect, it, vi} from "vitest";

import {
    applySpecialOpsBootstrapUpdate,
    applyExecutableSelection,
    buildInlineStationCorrection,
    buildTimelineHourSlots,
    changeAmmoTargetSeasonal,
    createInlineStationCorrectionDraft,
    eligibleLoginTrialAccounts,
    formatCalibrationTemplateTestResult,
    groupTimelineTasks,
    hasActiveSpecialOpsRun,
    mergeSpecialOpsBootstrap,
    moveAmmoTargetWithinGroup,
    parseNavigationDelayMs,
    insertNormalAmmoTarget,
    specialOpsErrorAfterUpdate,
    persistSpecialOpsSaveRequest,
    timelineDelayMinutes,
} from "@/components/app/special-ops-utils";
import type {
    AccountPlan,
    AmmoBusinessTarget,
    LoginRunSnapshot,
    SpecialOpsBootstrap,
    SpecialOpsSettings,
} from "@/components/app/special-ops-types";

describe("createInlineStationCorrectionDraft", () => {
    it("未来完成时间按分钟向上取整", () => {
        expect(createInlineStationCorrectionDraft({finishesAtMs: 160_001}, 100_000)).toEqual({
            state: "crafting",
            hours: "0",
            minutes: "2",
        });
    });

    it("无未来完成时间时保留空输入", () => {
        expect(createInlineStationCorrectionDraft({finishesAtMs: 100_000}, 100_000)).toEqual({
            state: "crafting",
            hours: "",
            minutes: "",
        });
    });
});

describe("buildInlineStationCorrection", () => {
    it.each([
        ["", "", null],
        ["0", "0", null],
        ["-1", "0", null],
        ["1.5", "0", null],
        ["0", "60", null],
        ["0", "1", 1],
        ["168", "0", 10_080],
        ["168", "1", null],
    ])("校验 %s 小时 %s 分钟", (hours, minutes, expected) => {
        expect(buildInlineStationCorrection("crafting", hours, minutes)?.remainingMinutes ?? null)
            .toBe(expected);
    });

    it.each(["immediateDue", "idle"] as const)("%s 不携带剩余时间", (state) => {
        expect(buildInlineStationCorrection(state, "", "")).toEqual({
            state,
            remainingMinutes: null,
        });
    });
});

function ammoBusinessTarget(id: string, seasonal: boolean, order: number): AmmoBusinessTarget {
    return {
        id,
        note: id,
        enabled: true,
        seasonal,
        clickPoint: null,
        scrollDirection: "down",
        scrollSteps: 0,
        order,
        profitRuleId: null,
    };
}

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
        independentSettingsEnabled: false,
        independentBusinessConfig: null,
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
        runKind: "login",
        status: "waiting",
        currentStep: "waitLoginChoice",
        message: `更新于 ${updatedAtMs}`,
        countdownSeconds: null,
        roundProgress: null,
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
            navigationBeaconDelayMs: 3000,
            navigationSpaceDelayMs: 3000,
            navigationTabDelayMs: 3000,
            navigationSpecialOpsDelayMs: 3000,
            ammoSupplyDelayMs: 3000,
            ammoTacticalDelayMs: 3000,
            craftSpaceDelayMs: 3000,
            craftReopenDelayMs: 3000,
            craftConfirmPinnedDelayMs: 3000,
            wegameExecutablePath: "wegame.exe",
            gameExecutablePath: "game.exe",
            defaultBusinessConfig: {stations: [], recipePoints: [], ammoTargets: []},
            profitFilter: {enabled: false, cutoffTime: "17:00", rules: [], audits: []},
            accounts: [account("account-1", 0)],
            activeCalibrationId: null,
            calibrationEnvironments: [],
        },
        schedule: {dueAccounts: [], nextWakeAtMs: null, timelineStartMs: 0, timelineEndMs: 24 * 60 * 60_000, timelineTasks: []},
        settingsRevision,
        nowMs: settingsRevision,
        runSnapshot: null,
        profitRuntime: {
            phase: "disabled",
            nextQueryAtMs: null,
            queryAttempt: null,
            qualifiedRuleIds: [],
            currentSessionRuleIds: [],
            activeRoundTargets: [],
            lastSummary: null,
            configurationError: null,
        },
        ...overrides,
    };
}

describe("特勤处登录试运行 helpers", () => {
    it("OCR 校准测试显示两次识别到的账号文本", () => {
        const result = {
            method: "ocr" as const,
            firstTexts: ["3079643589"],
            secondTexts: ["3079643589"],
            passed: true,
        };

        expect(formatCalibrationTemplateTestResult("已记住账号列表", result)).toBe(
            "已记住账号列表：OCR 双采样 3079643589 / 3079643589，已通过",
        );
    });

    it("勾选赛季限定后移动到赛季组末尾并重排 order", () => {
        const targets = [
            ammoBusinessTarget("normal-a", false, 0),
            ammoBusinessTarget("normal-b", false, 1),
            ammoBusinessTarget("seasonal-a", true, 2),
        ];

        const changed = changeAmmoTargetSeasonal(targets, "normal-a", true);

        expect(changed.map((target) => target.id)).toEqual(["normal-b", "seasonal-a", "normal-a"]);
        expect(changed.map((target) => target.order)).toEqual([0, 1, 2]);
    });

    it("取消赛季限定后移动到普通组末尾", () => {
        const targets = [
            ammoBusinessTarget("normal-a", false, 0),
            ammoBusinessTarget("seasonal-a", true, 1),
            ammoBusinessTarget("seasonal-b", true, 2),
        ];

        const changed = changeAmmoTargetSeasonal(targets, "seasonal-b", false);

        expect(changed.map((target) => target.id)).toEqual(["normal-a", "seasonal-b", "seasonal-a"]);
        expect(changed.map((target) => target.order)).toEqual([0, 1, 2]);
    });

    it("上下移动只交换同组相邻目标", () => {
        const targets = [
            ammoBusinessTarget("normal-a", false, 0),
            ammoBusinessTarget("normal-b", false, 1),
            ammoBusinessTarget("seasonal-a", true, 2),
            ammoBusinessTarget("seasonal-b", true, 3),
        ];

        expect(moveAmmoTargetWithinGroup(targets, "seasonal-b", -1).map((target) => target.id))
            .toEqual(["normal-a", "normal-b", "seasonal-b", "seasonal-a"]);
        expect(moveAmmoTargetWithinGroup(targets, "seasonal-a", -1)).toEqual(targets);
    });

    it("新增普通子弹插入所有赛季子弹之前", () => {
        const targets = [
            ammoBusinessTarget("normal-a", false, 0),
            ammoBusinessTarget("seasonal-a", true, 1),
        ];

        const changed = insertNormalAmmoTarget(targets, ammoBusinessTarget("normal-b", false, 9));

        expect(changed.map((target) => target.id)).toEqual(["normal-a", "normal-b", "seasonal-a"]);
        expect(changed.map((target) => target.order)).toEqual([0, 1, 2]);
    });

    it("导航动作等待时间只接受 0 到 60000 的整数毫秒", () => {
        expect(parseNavigationDelayMs("0")).toBe(0);
        expect(parseNavigationDelayMs("3000")).toBe(3000);
        expect(parseNavigationDelayMs("60000")).toBe(60000);
        expect(parseNavigationDelayMs("-1")).toBeNull();
        expect(parseNavigationDelayMs("60001")).toBeNull();
        expect(parseNavigationDelayMs("1.5")).toBeNull();
        expect(parseNavigationDelayMs("")).toBeNull();
        expect(parseNavigationDelayMs(" 3000 ")).toBeNull();
    });

    it("stopping 与 final snapshot 在 bootstrap 清空前均保持 active", () => {
        expect(hasActiveSpecialOpsRun({...runSnapshot(10), status: "stopping"})).toBe(true);
        expect(hasActiveSpecialOpsRun({...runSnapshot(11), status: "stopped"})).toBe(true);
        expect(hasActiveSpecialOpsRun(null)).toBe(false);
    });

    it("已结束 run 可被同 revision authoritative bootstrap null 清空", () => {
        const current = bootstrap(8, {
            runSnapshot: {...runSnapshot(50), status: "stopped"},
        });
        const incoming = bootstrap(8, {runSnapshot: null});

        expect(mergeSpecialOpsBootstrap(current, incoming).runSnapshot).toBeNull();
    });

    it("运行中的 run 不被旧 null response 清空", () => {
        const current = bootstrap(8, {runSnapshot: runSnapshot(50)});
        const incoming = bootstrap(8, {runSnapshot: null});

        expect(mergeSpecialOpsBootstrap(current, incoming).runSnapshot).toBe(current.runSnapshot);
    });

    it("同 revision 的旧 response 不覆盖较新的 runChanged snapshot", () => {
        const current = bootstrap(7, {runSnapshot: runSnapshot(50)});
        const incoming = bootstrap(7, {runSnapshot: runSnapshot(40)});

        expect(mergeSpecialOpsBootstrap(current, incoming).runSnapshot).toBe(current.runSnapshot);
    });

    it("低 revision response 整包不得回退 runtime-managed account fields", () => {
        const runtimeAccount = account("account-1", 0, {});
        runtimeAccount.status = "loginFailed";
        runtimeAccount.lastFailure = {
            step: "submitLogin",
            message: "密码错误",
            atMs: 80,
            stationKind: null,
            ammoTargetId: null,
        };
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
        runtimeAccount.lastFailure = {
            step: "submitLogin",
            message: "密码错误",
            atMs: 80,
            stationKind: null,
            ammoTargetId: null,
        };
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
            navigationBeaconDelayMs: 3000,
            navigationSpaceDelayMs: 3000,
            navigationTabDelayMs: 3000,
            navigationSpecialOpsDelayMs: 3000,
            ammoSupplyDelayMs: 3000,
            ammoTacticalDelayMs: 3000,
            craftSpaceDelayMs: 3000,
            craftReopenDelayMs: 3000,
            craftConfirmPinnedDelayMs: 3000,
            wegameExecutablePath: "wegame.exe",
            gameExecutablePath: "game.exe",
            defaultBusinessConfig: {stations: [], recipePoints: [], ammoTargets: []},
            profitFilter: {enabled: false, cutoffTime: "17:00", rules: [], audits: []},
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
            method: "template",
            sampleSimilarities: [0.9876, 0.8],
            passed: true,
            verifiedAtMs: 123,
        })).toBe("登录按钮：双采样相似度 98.8% / 80.0%，已通过");
        expect(formatCalibrationTemplateTestResult("登录按钮", {
            method: "template",
            sampleSimilarities: [0.74, 0.99],
            passed: false,
            verifiedAtMs: null,
        })).toBe("登录按钮：双采样相似度 74.0% / 99.0%，未通过");
    });
});

describe("特勤处 24 小时任务时间轴", () => {
    const taskAt = (id: string, scheduledAtMs: number) => ({
        id,
        accountId: "account-1",
        qqAccount: "10001",
        kind: "craft" as const,
        stationKind: "technicalCenter" as const,
        ammoTargetId: null,
        note: id,
        scheduledAtMs,
        overdue: false,
        accountStatus: "ready" as const,
        manualFailure: null,
    });

    it("以第一项为锚合并十分钟内任务且不链式扩展", () => {
        const minute = 60_000;
        const groups = groupTimelineTasks([
            taskAt("c", 36 * minute),
            taskAt("a", 20 * minute),
            taskAt("b", 28 * minute),
        ], 10 * minute);

        expect(groups.map((group) => group.tasks.map((task) => task.id))).toEqual([
            ["a", "b"],
            ["c"],
        ]);
    });

    it("恰好相差十分钟不合并", () => {
        const groups = groupTimelineTasks([taskAt("a", 0), taskAt("b", 600_000)]);
        expect(groups).toHaveLength(2);
    });

    it("逾期显示零分钟后且不修改原计划时间", () => {
        const task = taskAt("overdue", 1_000);
        expect(timelineDelayMinutes(task, 121_000)).toBe(0);
        expect(task.scheduledAtMs).toBe(1_000);
    });

    it("生成滚动未来二十四小时槽", () => {
        const slots = buildTimelineHourSlots(new Date("2026-07-30T10:23:00+08:00").getTime());
        expect(slots).toHaveLength(24);
        expect(slots[1] - slots[0]).toBe(60 * 60_000);
        expect(slots[0]).toBe(new Date("2026-07-30T10:00:00+08:00").getTime());
        expect(slots[23]).toBe(new Date("2026-07-31T09:00:00+08:00").getTime());
    });
});

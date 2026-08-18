import {describe, expect, it, vi} from "vitest";

import {
    accountRestorable,
    applySpecialOpsBootstrapUpdate,
    applyExecutableSelection,
    buildInlineStationCorrection,
    buildTimelineHourSlots,
    changeAmmoTargetSeasonal,
    createInlineStationCorrectionDraft,
    createStationRemainingTimeDraft,
    eligibleLoginTrialAccounts,
    formatCalibrationTemplateTestResult,
    groupTimelineTasks,
    hasActiveSpecialOpsRun,
    mergeSpecialOpsBootstrap,
    moveAmmoTargetWithinGroup,
    parseNavigationDelayMs,
    insertNormalAmmoTarget,
    formatLimitedMatchedColors,
    limitedColorToHex,
    parseLimitedColorHex,
    shanghaiDay,
    specialOpsErrorAfterUpdate,
    persistSpecialOpsSaveRequest,
    timelineDelayMinutes,
    timelineTaskAllowsInlineCorrection,
    timelineTaskLabel,
} from "@/components/app/special-ops-utils";
import type {
    AccountFailure,
    AccountPlan,
    AmmoBusinessTarget,
    AmmoTarget,
    LoginRunSnapshot,
    SpecialOpsBootstrap,
    SpecialOpsSettings,
    StationPlan,
    TimelineTask,
} from "@/components/app/special-ops-types";

describe("限时商品颜色转换", () => {
    it("把 RGB 转成六位小写 hex", () => {
        expect(limitedColorToHex([0, 15, 255])).toBe("#000fff");
        expect(limitedColorToHex([255, 255, 255])).toBe("#ffffff");
    });

    it("只接受六位 hex 并返回 RGB", () => {
        expect(parseLimitedColorHex("#00Ff80")).toEqual([0, 255, 128]);
        expect(parseLimitedColorHex("00ff80")).toEqual([0, 255, 128]);
        expect(parseLimitedColorHex("#fff")).toBeNull();
        expect(parseLimitedColorHex("#gg0000")).toBeNull();
    });

    it("把命中颜色编号格式化成 1 / 2 / 都有", () => {
        expect(formatLimitedMatchedColors([])).toBe("");
        expect(formatLimitedMatchedColors([1])).toBe("命中颜色 1");
        expect(formatLimitedMatchedColors([2])).toBe("命中颜色 2");
        expect(formatLimitedMatchedColors([2, 1, 1])).toBe("命中颜色 1 和 2");
        expect(formatLimitedMatchedColors([3, 0])).toBe("");
    });
});

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

describe("createStationRemainingTimeDraft", () => {
    it("按完成时间生成剩余小时和分钟并向上取整", () => {
        const nowMs = 1_000_000;
        expect(createStationRemainingTimeDraft({finishesAtMs: nowMs + 61_001}, nowMs))
            .toEqual({hours: "0", minutes: "2"});
        expect(createStationRemainingTimeDraft({finishesAtMs: nowMs}, nowMs))
            .toEqual({hours: "", minutes: ""});
        expect(createStationRemainingTimeDraft(
            {finishesAtMs: nowMs + 10_081 * 60_000},
            nowMs,
        )).toEqual({hours: "", minutes: ""});
    });
});

describe("buildInlineStationCorrection", () => {
    // 留空/0 与非法输入必须区分：前者提交 remainingMinutes: null 让后端继承存量计时，
    // 后者返回 null 对象锁住提交按钮。只比较 remainingMinutes 会让两者都过。
    it.each([
        ["", ""],
        ["0", "0"],
        ["", "0"],
        ["0", ""],
        [" ", " "],
    ])("留空或 0（%s 小时 %s 分钟）提交继承标记", (hours, minutes) => {
        expect(buildInlineStationCorrection("crafting", hours, minutes)).toEqual({
            state: "crafting",
            remainingMinutes: null,
        });
    });

    it.each([
        ["-1", "0"],
        ["1.5", "0"],
        ["0", "60"],
        ["168", "1"],
        ["abc", "0"],
    ])("非法输入 %s 小时 %s 分钟返回 null", (hours, minutes) => {
        expect(buildInlineStationCorrection("crafting", hours, minutes)).toBeNull();
    });

    it.each([
        ["0", "1", 1],
        ["1", "30", 90],
        ["168", "0", 10_080],
    ])("%s 小时 %s 分钟折算成 %i 分钟", (hours, minutes, expected) => {
        expect(buildInlineStationCorrection("crafting", hours, minutes)).toEqual({
            state: "crafting",
            remainingMinutes: expected,
        });
    });

    it.each(["immediateDue", "idle"] as const)("%s 不携带剩余时间", (state) => {
        expect(buildInlineStationCorrection(state, "12", "30")).toEqual({
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
    patch: Partial<AccountPlan> = {},
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
            ammoExchangeEntryDelayMs: 3000,
            craftSpaceDelayMs: 3000,
            craftReopenDelayMs: 3000,
            craftConfirmPinnedDelayMs: 3000,
            wegameExecutablePath: "wegame.exe",
            gameExecutablePath: "game.exe",
            defaultBusinessConfig: {stations: [], recipePoints: [], ammoTargets: []},
            profitFilter: {enabled: false, cutoffTime: "17:00", rules: [], audits: [], cutoffState: null},
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
            ammoExchangeEntryDelayMs: 3000,
            craftSpaceDelayMs: 3000,
            craftReopenDelayMs: 3000,
            craftConfirmPinnedDelayMs: 3000,
            wegameExecutablePath: "wegame.exe",
            gameExecutablePath: "game.exe",
            defaultBusinessConfig: {stations: [], recipePoints: [], ammoTargets: []},
            profitFilter: {enabled: false, cutoffTime: "17:00", rules: [], audits: [], cutoffState: null},
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

    it("识别新增限时商品与交易行任务标签", () => {
        expect(timelineTaskLabel({...taskAt("limited", 0), kind: "limitedSupplyCheck", stationKind: null})).toBe("限时商品检查");
        expect(timelineTaskLabel({...taskAt("market", 0), kind: "marketPurchase", stationKind: null})).toBe("交易行购买");
    });
});

function station(patch: Partial<StationPlan> = {}): StationPlan {
    return {
        kind: "technicalCenter",
        enabled: true,
        itemName: "配方",
        durationMinutes: 60,
        startedAtMs: null,
        finishesAtMs: null,
        status: "idle",
        ...patch,
    };
}

function ammoTarget(patch: Partial<AmmoTarget> = {}): AmmoTarget {
    return {
        id: "target-1",
        name: "子弹",
        enabled: true,
        seasonal: false,
        scrollSteps: 0,
        order: 0,
        lastSuccessDay: null,
        retryDay: null,
        retryCount: 0,
        lastFailure: null,
        ...patch,
    };
}

function failure(patch: Partial<AccountFailure> = {}): AccountFailure {
    return {
        step: "ammo.confirm",
        message: "确认失败",
        atMs: 1_000,
        stationKind: null,
        ammoTargetId: null,
        ...patch,
    };
}

describe("accountRestorable", () => {
    const currentDay = "2026-08-10";

    it("干净账号不可恢复", () => {
        expect(accountRestorable(account("account-1", 0, {
            stations: [station({status: "crafting"})],
            ammoTargets: [ammoTarget({lastSuccessDay: "2026-08-09"})],
        }), currentDay)).toBe(false);
    });

    it.each([
        ["账号状态异常", {status: "manualCheckRequired"} as Partial<AccountPlan>],
        ["账号带失败记录", {lastFailure: failure()}],
        ["制作台 Uncertain", {stations: [station({status: "uncertain"})]}],
        ["子弹目标带失败", {ammoTargets: [ammoTarget({lastFailure: failure()})]}],
        ["子弹目标已重试", {ammoTargets: [ammoTarget({retryCount: 1})]}],
        ["子弹目标当天已兑换", {ammoTargets: [ammoTarget({lastSuccessDay: "2026-08-10"})]}],
        ["限时商品失败", {
            limitedSupply: {
                cycleId: "2026-08-09T08:00",
                outcome: "failed" as const,
                checkedAtMs: 1_000,
                matchedRegion: null,
                matchedColor: null,
                acknowledged: false,
                lastError: "识别超时",
            },
        }],
        // 交易行被封锁同样算可恢复：后端会放回 pending，否则点「继续」不会再跑交易行。
        ["交易行窗口已关闭", {market: {day: "2026-08-10", completedCount: 1, status: "windowClosed" as const, lastError: null}}],
        ["交易行价格识别失败", {market: {day: "2026-08-10", completedCount: 0, status: "priceRecognitionFailed" as const, lastError: "OCR 失败"}}],
        ["交易行残留运行中", {market: {day: "2026-08-10", completedCount: 0, status: "running" as const, lastError: null}}],
    ])("%s 时可恢复", (_label, patch) => {
        expect(accountRestorable(account("account-1", 0, patch), currentDay)).toBe(true);
    });

    it.each([
        // 购买次数已经花掉，后端不动 Completed -> 前端也不能亮按钮。
        ["交易行当天已完成", {market: {day: "2026-08-10", completedCount: 3, status: "completed" as const, lastError: null}}],
        // 昨天的封锁状态与今天无关，后端只放回当天的。
        ["交易行封锁属于昨天", {market: {day: "2026-08-09", completedCount: 1, status: "windowClosed" as const, lastError: null}}],
    ])("%s 时不可恢复", (_label, patch) => {
        expect(accountRestorable(account("account-1", 0, {
            stations: [station({status: "crafting"})],
            ammoTargets: [ammoTarget({lastSuccessDay: "2026-08-09"})],
            ...patch,
        }), currentDay)).toBe(false);
    });
});

describe("shanghaiDay", () => {
    it.each([
        ["2026-08-09T23:59:00+08:00", "2026-08-09"],
        ["2026-08-10T00:00:00+08:00", "2026-08-10"],
        // UTC 仍是 8 月 9 日，但 Asia/Shanghai 已跨天 -> 必须按固定 UTC+8 取日
        ["2026-08-09T16:30:00Z", "2026-08-10"],
    ])("%s -> %s", (iso, expected) => {
        expect(shanghaiDay(new Date(iso).getTime())).toBe(expected);
    });
});

describe("timelineTaskAllowsInlineCorrection", () => {
    const task = (patch: Partial<TimelineTask> = {}): Pick<TimelineTask, "kind" | "accountStatus" | "manualFailure"> => ({
        kind: "craft",
        accountStatus: "ready",
        manualFailure: null,
        ...patch,
    });

    it("带定位失败的制作与子弹任务给出入口", () => {
        expect(timelineTaskAllowsInlineCorrection(
            task({manualFailure: failure({stationKind: "workbench"})}),
            station({status: "uncertain"}),
        )).toBe(true);
        expect(timelineTaskAllowsInlineCorrection(
            task({kind: "ammo", manualFailure: failure({ammoTargetId: "target-1"})}),
            null,
        )).toBe(true);
    });

    it("无定位失败但制作台 Uncertain 仍给出入口", () => {
        // NavigationTimedOut 只落 ManualCheckRequired，manualFailure 为空，
        // 旧逻辑在这里直接返回 null → 任务栏没有任何人工判定选项。
        expect(timelineTaskAllowsInlineCorrection(
            task({accountStatus: "manualCheckRequired"}),
            station({status: "uncertain"}),
        )).toBe(true);
        expect(timelineTaskAllowsInlineCorrection(
            task({kind: "ammo", accountStatus: "manualCheckRequired"}),
            null,
        )).toBe(true);
    });

    it("制作台正常时不给出入口", () => {
        expect(timelineTaskAllowsInlineCorrection(
            task({accountStatus: "manualCheckRequired"}),
            station({status: "crafting"}),
        )).toBe(false);
        expect(timelineTaskAllowsInlineCorrection(task(), null)).toBe(false);
    });

    it.each(["needsManualLogin", "loginFailed"] as const)("%s 只能在账号页处理", (accountStatus) => {
        expect(timelineTaskAllowsInlineCorrection(
            task({accountStatus, manualFailure: failure({stationKind: "workbench"})}),
            station({status: "uncertain"}),
        )).toBe(false);
    });

    it.each(["limitedSupplyCheck", "marketPurchase"] as const)("%s 不支持单项判定", (kind) => {
        expect(timelineTaskAllowsInlineCorrection(
            task({kind, manualFailure: failure()}),
            null,
        )).toBe(false);
    });
});


import {describe, expect, it} from "vitest";

import {
    buildProfitConfigurationDraft,
    deleteProfitRuleFromDraft,
    listProfitBindings,
    parseMinimumProfit,
    profitConfigurationFingerprint,
    ruleReferenceCounts,
} from "@/components/app/special-ops-profit-utils";
import type {SpecialOpsSettings} from "@/components/app/special-ops-types";

function settings(): SpecialOpsSettings {
    return {
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
        wegameExecutablePath: "",
        gameExecutablePath: "",
        defaultBusinessConfig: {
            stations: [],
            recipePoints: [],
            ammoTargets: [{id: "default-a", note: "默认 A", enabled: true, seasonal: false, clickPoint: {x: 1, y: 2, width: 1, height: 1}, scrollDirection: "down", scrollSteps: 0, order: 0, profitRuleId: "rule-a"}],
        },
        profitFilter: {
            enabled: true,
            cutoffTime: "17:00",
            rules: [
                {id: "rule-a", displayName: "规则 A", kkrbMatchName: "KKRB A", moligodMatchName: null, minimumProfit: 100},
                {id: "rule-b", displayName: "规则 B", kkrbMatchName: "KKRB B", moligodMatchName: null, minimumProfit: 200},
            ],
            audits: [],
        },
        accounts: [
            {
                id: "inherited",
                qqAccount: "10001",
                enabled: true,
                initialized: true,
                order: 0,
                status: "ready",
                independentSettingsEnabled: false,
                independentBusinessConfig: null,
                stations: [],
                ammoTargets: [],
                lastFailure: null,
                loginTrialSignature: null,
            },
            {
                id: "independent",
                qqAccount: "10002",
                enabled: true,
                initialized: true,
                order: 1,
                status: "ready",
                independentSettingsEnabled: true,
                independentBusinessConfig: {
                    stations: [],
                    recipePoints: [],
                    ammoTargets: [{id: "independent-a", note: "独立 A", enabled: true, seasonal: false, clickPoint: {x: 3, y: 4, width: 1, height: 1}, scrollDirection: "down", scrollSteps: 0, order: 0, profitRuleId: "rule-a"}],
                },
                stations: [],
                ammoTargets: [],
                lastFailure: null,
                loginTrialSignature: null,
            },
        ],
        activeCalibrationId: null,
        calibrationEnvironments: [],
    };
}

describe("特勤处利润配置 helpers", () => {
    it("默认目标仅列一次，独立账号目标额外列出", () => {
        const bindings = listProfitBindings(settings());

        expect(bindings.map((binding) => [binding.accountId, binding.targetId]))
            .toEqual([[null, "default-a"], ["independent", "independent-a"]]);
    });

    it("规则引用计数包含默认与独立业务目标", () => {
        expect([...ruleReferenceCounts(buildProfitConfigurationDraft(settings())).entries()])
            .toEqual([["rule-a", 2], ["rule-b", 0]]);
    });

    it("删除规则同时清空全部绑定", () => {
        const next = deleteProfitRuleFromDraft(buildProfitConfigurationDraft(settings()), "rule-a");

        expect(next.rules.map((rule) => rule.id)).toEqual(["rule-b"]);
        expect(next.bindings.every((binding) => binding.profitRuleId === null)).toBe(true);
    });

    it("最低利润只接受非负安全整数", () => {
        expect(parseMinimumProfit("0")).toBe(0);
        expect(parseMinimumProfit("270458")).toBe(270458);
        expect(parseMinimumProfit("-1")).toBeNull();
        expect(parseMinimumProfit("1.5")).toBeNull();
        expect(parseMinimumProfit("9007199254740992")).toBeNull();
    });

    it("配置 fingerprint 忽略审计变化但保留规则与绑定变化", () => {
        const original = settings();
        const audited = structuredClone(original);
        audited.profitFilter.audits.push({ruleId: "rule-a", day: "2026-08-02", queriedAtMs: 1, source: "kkrb", attemptedSources: ["kkrb"], sourceDataAt: null, sourceVersion: null, profit: 10, threshold: 100, outcome: "belowThreshold", detail: "未达标", nextQueryAtMs: 2});
        const rebound = structuredClone(original);
        rebound.defaultBusinessConfig.ammoTargets[0].profitRuleId = "rule-b";

        expect(profitConfigurationFingerprint(audited)).toBe(profitConfigurationFingerprint(original));
        expect(profitConfigurationFingerprint(rebound)).not.toBe(profitConfigurationFingerprint(original));
    });
});

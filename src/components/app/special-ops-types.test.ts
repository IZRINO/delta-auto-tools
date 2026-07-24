import {beforeEach, describe, expect, it, vi} from "vitest";

import {
    formatCalibrationTemplateTestResult,
    testSpecialOpsCalibrationTarget,
} from "@/components/app/special-ops-types";

const {invokeLogged} = vi.hoisted(() => ({invokeLogged: vi.fn()}));

vi.mock("@/lib/logging", () => ({invokeLogged}));

describe("特勤处校准测试结果", () => {
    beforeEach(() => invokeLogged.mockReset());

    it("按百分比显示两次模板相似度与通过状态", () => {
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

    it("模板测试调用携带完整 revision 合约", async () => {
        const result = {
            sampleSimilarities: [0.8, 0.9] as [number, number],
            passed: true,
            verifiedAtMs: 123,
        };
        invokeLogged.mockResolvedValue(result);

        await expect(testSpecialOpsCalibrationTarget({
            environmentId: "default",
            targetKey: "wegame.login",
            settingsRevision: 42,
        })).resolves.toEqual(result);
        expect(invokeLogged).toHaveBeenCalledWith("special_ops_test_calibration_target", {
            environmentId: "default",
            targetKey: "wegame.login",
            settingsRevision: 42,
        });
    });
});

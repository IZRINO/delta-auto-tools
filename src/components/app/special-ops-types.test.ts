import {describe, expect, it} from "vitest";

import {formatCalibrationTemplateTestResult} from "@/components/app/special-ops-types";

describe("特勤处校准测试结果", () => {
    it("按百分比显示两次模板相似度", () => {
        expect(formatCalibrationTemplateTestResult("登录按钮", {sampleSimilarities: [0.9876, 0.8]}))
            .toBe("登录按钮：双采样相似度 98.8% / 80.0%");
    });
});

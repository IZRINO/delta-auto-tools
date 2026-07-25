import {describe, expect, it} from "vitest";

import {operationOverlayText} from "@/components/app/special-ops-operation-overlay";
import type {LoginRunSnapshot} from "@/components/app/special-ops-types";

function snapshot(overrides: Partial<LoginRunSnapshot> = {}): LoginRunSnapshot {
    return {
        runId: 1,
        accountId: "account-1",
        status: "waiting",
        currentStep: "waitLoginChoice",
        message: "正在识别登录入口",
        countdownSeconds: null,
        startedAtMs: 1,
        updatedAtMs: 1,
        ...overrides,
    };
}

describe("operationOverlayText", () => {
    it("倒计时时提示即将占用键盘鼠标", () => {
        expect(operationOverlayText(snapshot({countdownSeconds: 2}))).toEqual({
            title: "即将占用键盘鼠标",
            detail: "2 秒后执行当前步骤",
            hotkey: "Ctrl+Shift+F12",
        });
    });

    it("无倒计时时提示特勤处操作中", () => {
        expect(operationOverlayText(snapshot({countdownSeconds: null})).title).toBe("特勤处操作中");
    });
});

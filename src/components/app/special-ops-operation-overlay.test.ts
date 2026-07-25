import {createElement} from "react";
import {renderToStaticMarkup} from "react-dom/server";
import {describe, expect, it, vi} from "vitest";

import {
    SpecialOpsOperationOverlay,
    operationOverlayText,
} from "@/components/app/special-ops-operation-overlay";
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

describe("SpecialOpsOperationOverlay", () => {
    it("事件尚未到达时首帧仍显示准备状态和自定义紧急热键", () => {
        vi.stubGlobal("window", {
            location: {search: "?emergencyHotkey=Ctrl%2BAlt%2BX"},
        });

        const html = renderToStaticMarkup(createElement(SpecialOpsOperationOverlay));

        expect(html).toContain("特勤处操作中");
        expect(html).toContain("正在准备登录流程");
        expect(html).toContain("紧急停止：Ctrl+Alt+X");
    });
});

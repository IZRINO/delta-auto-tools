import {createElement} from "react";
import {renderToStaticMarkup} from "react-dom/server";
import {describe, expect, it, vi} from "vitest";

import {
    loadOperationRunSnapshot,
    OperationHud,
    SpecialOpsOperationOverlay,
    operationOverlayText,
} from "@/components/app/special-ops-operation-overlay";
import type {LoginRunSnapshot} from "@/components/app/special-ops-types";

function snapshot(overrides: Partial<LoginRunSnapshot> = {}): LoginRunSnapshot {
    return {
        runId: 1,
        accountId: "account-1",
        runKind: "login",
        status: "waiting",
        currentStep: "waitLoginChoice",
        message: "正在识别登录入口",
        countdownSeconds: null,
        roundProgress: null,
        startedAtMs: 1,
        updatedAtMs: 1,
        ...overrides,
    };
}

describe("operationOverlayText", () => {
    it("倒计时时提示即将占用键盘鼠标", () => {
        expect(operationOverlayText(snapshot({countdownSeconds: 2}))).toEqual({
            title: "即将占用键盘鼠标",
            detail: "正在识别登录入口",
            countdownSeconds: 2,
            hotkey: "Ctrl+Shift+F12",
        });
    });

    it("切号倒计时显示秒数文案", () => {
        expect(operationOverlayText(snapshot({
            runKind: "round",
            status: "countdown",
            message: "15 秒后切换下一账号",
            countdownSeconds: 15,
        }))).toEqual({
            title: "即将占用键盘鼠标",
            detail: "15 秒后切换下一账号",
            countdownSeconds: 15,
            hotkey: "Ctrl+Shift+F12",
        });
    });

    it("无倒计时时提示特勤处操作中", () => {
        expect(operationOverlayText(snapshot({countdownSeconds: null})).title).toBe("特勤处操作中");
    });

    it("按 run 类型显示实际试运行流程", () => {
        expect(operationOverlayText(snapshot({runKind: "navigation"})).title).toBe("游戏内导航试运行中");
        expect(operationOverlayText(snapshot({runKind: "craft"})).title).toBe("制作试运行中");
        expect(operationOverlayText(snapshot({runKind: "ammo"})).title).toBe("子弹兑换操作中");
        expect(operationOverlayText(snapshot({runKind: "limitedSupply"})).title).toBe("限时商品检查中");
        expect(operationOverlayText(snapshot({runKind: "market"})).title).toBe("交易行购买中");
        expect(operationOverlayText(snapshot({runKind: "round" as LoginRunSnapshot["runKind"]})).title).toBe("多账号制作轮次中");
        expect(operationOverlayText(snapshot({runKind: "stationWalkthrough"})).title).toBe("多账号制作台更改中");
    });

    it("多账号轮次显示账号与制作台进度", () => {
        expect(operationOverlayText(snapshot({
            runKind: "round",
            roundProgress: {
                accountIndex: 2,
                accountTotal: 4,
                qqAccount: "12345",
                stationKind: "workbench",
                stationIndex: 1,
                stationTotal: 3,
            },
        })).detail).toBe("账号 2/4 · QQ 12345 · 工作台 1/3");
    });

    it("多账号轮次等待时显示会话保持或切号状态", () => {
        expect(operationOverlayText(snapshot({
            runKind: "round",
            status: "waiting",
            message: "保持当前账号在线，等待同账号下一任务",
            roundProgress: {
                accountIndex: 1,
                accountTotal: 2,
                qqAccount: "12345",
                stationKind: null,
                stationIndex: 0,
                stationTotal: 0,
            },
        })).detail).toBe("账号 1/2 · QQ 12345 · 保持当前账号在线，等待同账号下一任务");
    });
});

describe("SpecialOpsOperationOverlay", () => {
    it("挂载后读取 bootstrap 中的运行快照", async () => {
        const expected = snapshot({runKind: "round"});

        await expect(loadOperationRunSnapshot(async () => ({runSnapshot: expected}))).resolves.toEqual(expected);
    });

    it("事件尚未到达时首帧仍显示准备状态和自定义紧急热键", () => {
        vi.stubGlobal("window", {
            location: {search: "?emergencyHotkey=Ctrl%2BAlt%2BX&runKind=craft"},
        });

        const html = renderToStaticMarkup(createElement(SpecialOpsOperationOverlay));

        expect(html).toContain("制作试运行中");
        expect(html).toContain("正在准备制作试运行");
        expect(html).toContain("紧急停止：Ctrl+Alt+X");
        expect(html).not.toContain("card-title");
        expect(html).not.toContain("shadow-lg");
    });

    it("倒计时 HUD 放大秒数并保留紧急停止", () => {
        const html = renderToStaticMarkup(createElement(OperationHud, {
            title: "即将占用键盘鼠标",
            detail: "3 秒后执行当前步骤",
            countdownSeconds: 3,
            hotkey: "Ctrl+Shift+F12",
        }));
        expect(html).toContain("即将占用键盘鼠标");
        expect(html).toContain(">3<");
        expect(html).toContain("3 秒后执行当前步骤");
        expect(html).not.toContain("border-primary");
        expect(html).toContain("紧急停止：Ctrl+Shift+F12");
        expect(html).toContain("ops-digit");
        expect(html).toContain("ops-fuse");
        expect(html).toContain("aria-live=\"polite\"");
    });
});

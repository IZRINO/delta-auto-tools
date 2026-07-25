import {beforeEach, describe, expect, it, vi} from "vitest";

import {
    formatCalibrationTemplateTestResult,
    reloadSpecialOpsAfterStateChanged,
    runLatestSpecialOpsBootstrapRequest,
    testSpecialOpsCalibrationTarget,
} from "@/components/app/special-ops-types";

const {invokeLogged} = vi.hoisted(() => ({invokeLogged: vi.fn()}));

vi.mock("@/lib/logging", () => ({invokeLogged}));

function deferred<T>() {
    let resolve!: (value: T) => void;
    let reject!: (error: unknown) => void;
    const promise = new Promise<T>((nextResolve, nextReject) => {
        resolve = nextResolve;
        reject = nextReject;
    });
    return {promise, resolve, reject};
}

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

    it("窄状态事件只触发 bootstrap reload", () => {
        const reload = vi.fn();

        reloadSpecialOpsAfterStateChanged({settingsRevision: 17, nowMs: 23}, reload);

        expect(reload).toHaveBeenCalledOnce();
    });
});

describe("特勤处 bootstrap 请求竞态", () => {
    it("后发 save 成功后忽略旧 reload 结果", async () => {
        const token = {current: 0};
        const reload = deferred<string>();
        const save = deferred<string>();
        const applied: string[] = [];

        const reloadTask = runLatestSpecialOpsBootstrapRequest(token, () => reload.promise, (value) => applied.push(value), vi.fn());
        const saveTask = runLatestSpecialOpsBootstrapRequest(token, () => save.promise, (value) => applied.push(value), vi.fn());
        save.resolve("save");
        await saveTask;
        reload.resolve("reload");
        await reloadTask;

        expect(applied).toEqual(["save"]);
    });

    it("事件 reload 成功后忽略旧 save 结果", async () => {
        const token = {current: 0};
        const save = deferred<string>();
        const reload = deferred<string>();
        const applied: string[] = [];

        const saveTask = runLatestSpecialOpsBootstrapRequest(token, () => save.promise, (value) => applied.push(value), vi.fn());
        const reloadTask = runLatestSpecialOpsBootstrapRequest(token, () => reload.promise, (value) => applied.push(value), vi.fn());
        reload.resolve("reload");
        await reloadTask;
        save.resolve("save");
        await saveTask;

        expect(applied).toEqual(["reload"]);
    });

    it("新请求成功后忽略旧请求错误", async () => {
        const token = {current: 0};
        const oldRequest = deferred<string>();
        const latestRequest = deferred<string>();
        const onError = vi.fn();

        const oldTask = runLatestSpecialOpsBootstrapRequest(token, () => oldRequest.promise, vi.fn(), onError);
        const latestTask = runLatestSpecialOpsBootstrapRequest(token, () => latestRequest.promise, vi.fn(), onError);
        latestRequest.resolve("latest");
        await latestTask;
        oldRequest.reject(new Error("stale"));
        await oldTask;

        expect(onError).not.toHaveBeenCalled();
    });
});

import type {EventCallback, UnlistenFn} from "@tauri-apps/api/event";
import {afterEach, describe, expect, it, vi} from "vitest";

const mockListen = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/event", () => ({
    listen: mockListen,
}));

import {subscribeTauriEvent} from "@/lib/tauri-listener";

function deferred<T>() {
    let resolve!: (value: T) => void;
    let reject!: (error: unknown) => void;
    const promise = new Promise<T>((promiseResolve, promiseReject) => {
        resolve = promiseResolve;
        reject = promiseReject;
    });
    return {promise, reject, resolve};
}

describe("subscribeTauriEvent", () => {
    afterEach(() => {
        mockListen.mockReset();
    });

    it("listen resolve 后 cleanup 调用 unlisten 且重复 cleanup 幂等", async () => {
        const pending = deferred<UnlistenFn>();
        const unlisten = vi.fn();
        mockListen.mockReturnValueOnce(pending.promise);

        const cleanup = subscribeTauriEvent("test://event", vi.fn());
        pending.resolve(unlisten);
        await pending.promise;

        cleanup();
        cleanup();

        expect(unlisten).toHaveBeenCalledTimes(1);
    });

    it("cleanup 先于 listen resolve 时，resolve 后立即调用 unlisten 且恰好一次", async () => {
        const pending = deferred<UnlistenFn>();
        const unlisten = vi.fn();
        mockListen.mockReturnValueOnce(pending.promise);

        const cleanup = subscribeTauriEvent("test://event", vi.fn());
        cleanup();
        cleanup();
        pending.resolve(unlisten);
        await pending.promise;

        expect(unlisten).toHaveBeenCalledTimes(1);
    });

    it("cleanup 后不再调用业务 handler", async () => {
        const pending = deferred<UnlistenFn>();
        const handler = vi.fn();
        mockListen.mockReturnValueOnce(pending.promise);

        const cleanup = subscribeTauriEvent<string>("test://event", handler);
        const callback = mockListen.mock.calls[0][1] as EventCallback<string>;
        cleanup();
        callback({event: "test://event", id: 1, payload: "已释放"});
        pending.resolve(vi.fn());
        await pending.promise;

        expect(handler).not.toHaveBeenCalled();
    });

    it("disposed 前按 Tauri Event callback contract 转发事件", () => {
        const handler = vi.fn();
        mockListen.mockReturnValueOnce(new Promise<UnlistenFn>(() => undefined));

        subscribeTauriEvent<string>("test://event", handler);
        const callback = mockListen.mock.calls[0][1] as EventCallback<string>;
        const event = {event: "test://event", id: 1, payload: "有效"};
        callback(event);

        expect(handler).toHaveBeenCalledOnce();
        expect(handler).toHaveBeenCalledWith(event);
    });

    it("listen reject 时调用 onError 一次，disposed 后仍报告", async () => {
        const pending = deferred<UnlistenFn>();
        const error = new Error("监听失败");
        const onError = vi.fn();
        mockListen.mockReturnValueOnce(pending.promise);

        const cleanup = subscribeTauriEvent("test://event", vi.fn(), onError);
        cleanup();
        pending.reject(error);
        await pending.promise.catch(() => undefined);

        expect(onError).toHaveBeenCalledOnce();
        expect(onError).toHaveBeenCalledWith(error);
    });

    it("未提供 onError 时消费 listen reject", async () => {
        mockListen.mockRejectedValueOnce(new Error("监听失败"));

        const cleanup = subscribeTauriEvent("test://event", vi.fn());
        await new Promise((resolve) => setTimeout(resolve, 0));

        expect(cleanup).toBeTypeOf("function");
    });

    it("listen 注册完成后调用 onReady", async () => {
        const pending = deferred<UnlistenFn>();
        const onReady = vi.fn();
        mockListen.mockReturnValueOnce(pending.promise);

        subscribeTauriEvent("test://event", vi.fn(), undefined, onReady);
        expect(onReady).not.toHaveBeenCalled();

        pending.resolve(vi.fn());
        await pending.promise;

        expect(onReady).toHaveBeenCalledOnce();
    });
});

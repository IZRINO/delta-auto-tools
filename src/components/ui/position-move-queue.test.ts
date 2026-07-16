import {describe, expect, it, vi} from "vitest";

import {PositionMoveQueue} from "@/components/ui/position-move-queue";

function createFrameHarness() {
    let nextId = 1;
    const callbacks = new Map<number, () => void>();

    return {
        requestFrame(callback: () => void) {
            const id = nextId++;
            callbacks.set(id, callback);
            return id;
        },
        cancelFrame(id: number) {
            callbacks.delete(id);
        },
        runNext() {
            const next = callbacks.entries().next().value as [number, () => void] | undefined;
            if (!next) throw new Error("没有待执行帧");
            callbacks.delete(next[0]);
            next[1]();
        },
        pendingCount() {
            return callbacks.size;
        },
    };
}

describe("PositionMoveQueue", () => {
    it("合并 500 次拖动且始终只保留一个 in-flight，flush 发送最终坐标", async () => {
        const frames = createFrameHarness();
        const resolvers: Array<() => void> = [];
        let active = 0;
        let maxActive = 0;
        const invoke = vi.fn((_point: {x: number; y: number}) => {
            active += 1;
            maxActive = Math.max(maxActive, active);
            return new Promise<void>((resolve) => {
                resolvers.push(() => {
                    active -= 1;
                    resolve();
                });
            });
        });
        const queue = new PositionMoveQueue({
            invoke,
            requestFrame: frames.requestFrame,
            cancelFrame: frames.cancelFrame,
        });

        for (let index = 0; index < 500; index += 1) {
            queue.move({x: index, y: index * 2});
        }

        expect(frames.pendingCount()).toBe(1);
        frames.runNext();
        expect(invoke).toHaveBeenCalledTimes(1);
        expect(invoke).toHaveBeenLastCalledWith({x: 499, y: 998});

        for (let index = 500; index < 1000; index += 1) {
            queue.move({x: index, y: index * 2});
        }
        expect(frames.pendingCount()).toBe(0);

        const flushed = queue.flush();
        resolvers.shift()?.();
        await Promise.resolve();
        await Promise.resolve();

        expect(invoke).toHaveBeenCalledTimes(2);
        expect(invoke).toHaveBeenLastCalledWith({x: 999, y: 1998});
        expect(maxActive).toBe(1);

        resolvers.shift()?.();
        await flushed;
        queue.dispose();
    });

    it("最后一次移动失败时 flush 拒绝提交", async () => {
        const frames = createFrameHarness();
        const error = new Error("移动失败");
        const queue = new PositionMoveQueue({
            invoke: () => Promise.reject(error),
            requestFrame: frames.requestFrame,
            cancelFrame: frames.cancelFrame,
        });

        queue.move({x: 10, y: 20});

        await expect(queue.flush()).rejects.toBe(error);
        queue.dispose();
    });
});

import {describe, expect, it} from "vitest";

import {LatestSaveQueue} from "@/hooks/autosave-queue";

function deferred() {
    let resolve!: () => void;
    let reject!: (error: Error) => void;
    const promise = new Promise<void>((next, fail) => {
        resolve = next;
        reject = fail;
    });
    return {promise, resolve, reject};
}

describe("LatestSaveQueue", () => {
    it("同一时刻只运行一个 save，并只保留等待中的最新快照", async () => {
        const first = deferred();
        const calls: number[] = [];
        let active = 0;
        let maxActive = 0;
        const queue = new LatestSaveQueue<number>();
        const save = async (value: number) => {
            calls.push(value);
            active += 1;
            maxActive = Math.max(maxActive, active);
            if (value === 1) await first.promise;
            active -= 1;
        };

        const firstDrain = queue.enqueue(1, save);
        const secondDrain = queue.enqueue(2, save);
        const latestDrain = queue.enqueue(3, save);

        expect(calls).toEqual([1]);
        first.resolve();
        await Promise.all([firstDrain, secondDrain, latestDrain]);

        expect(calls).toEqual([1, 3]);
        expect(maxActive).toBe(1);
    });

    it("当前 save 失败后继续保存等待中的最新快照", async () => {
        const first = deferred();
        const calls: number[] = [];
        const queue = new LatestSaveQueue<number>();
        const save = async (value: number) => {
            calls.push(value);
            if (value === 1) await first.promise;
        };

        const firstSave = queue.enqueue(1, save);
        const supersededSave = queue.enqueue(2, save);
        const latestSave = queue.enqueue(3, save);
        first.reject(new Error("首次保存失败"));

        await expect(firstSave).rejects.toThrow("首次保存失败");
        await expect(supersededSave).resolves.toBeUndefined();
        await expect(latestSave).resolves.toBeUndefined();
        expect(calls).toEqual([1, 3]);
    });
});

import {describe, expect, it} from "vitest";

import {
    addFavorite,
    DEFAULT_FAVORITES_VIEW,
    type FavoriteItem,
    favoriteKey,
    type FavoritesState,
    isFavorite,
    moveFavorite,
    parseFavoriteKey,
    pruneFavorites,
    pruneFavoritesWithSources,
    readStoredFavorites,
    removeFavorite,
    renumberFavorites,
    sortFavorites,
    settleFavoriteBootstraps,
    toggleFavorite,
    updateFavoritesView,
    writeStoredFavorites,
} from "@/components/app/favorites-utils";

function deferred<T>() {
    let resolve!: (value: T) => void;
    let reject!: (error: unknown) => void;
    const promise = new Promise<T>((promiseResolve, promiseReject) => {
        resolve = promiseResolve;
        reject = promiseReject;
    });
    return {promise, reject, resolve};
}

function makeStub() {
    const data = new Map<string, string>();
    return {
        data,
        getItem: (k: string) => (data.has(k) ? data.get(k)! : null),
        setItem: (k: string, v: string) => {
            data.set(k, v);
        },
    };
}

function makeState(items: FavoriteItem[] = []): FavoritesState {
    return {items, view: {...DEFAULT_FAVORITES_VIEW}};
}

describe("favorites-utils", () => {
    describe("favoriteKey / parseFavoriteKey", () => {
        it("builds and parses keys for all card kinds", () => {
            expect(favoriteKey("timer", "t-1")).toBe("timer:t-1");
            expect(favoriteKey("counter", "c-2")).toBe("counter:c-2");
            expect(favoriteKey("rapidfire", "r-3")).toBe("rapidfire:r-3");
            expect(parseFavoriteKey("timer:t-1")).toEqual({kind: "timer", cardId: "t-1"});
            expect(parseFavoriteKey("rapidfire:r-3")).toEqual({kind: "rapidfire", cardId: "r-3"});
        });

        it("rejects malformed keys", () => {
            expect(parseFavoriteKey("nope")).toBeNull();
            expect(parseFavoriteKey("timer:")).toBeNull();
            expect(parseFavoriteKey(":t-1")).toBeNull();
            expect(parseFavoriteKey("invalid:t-1")).toBeNull();
        });
    });

    describe("addFavorite / removeFavorite / toggleFavorite / isFavorite", () => {
        it("adds new items with monotonically growing sortKey", () => {
            let s = makeState();
            s = addFavorite(s, "timer", "t-1");
            s = addFavorite(s, "counter", "c-1");
            s = addFavorite(s, "rapidfire", "r-1");
            expect(s.items).toHaveLength(3);
            const sortKeys = s.items.map((item) => item.sortKey);
            expect(sortKeys[1]).toBeGreaterThan(sortKeys[0]!);
            expect(sortKeys[2]).toBeGreaterThan(sortKeys[1]!);
        });

        it("isFavorite returns true after addFavorite", () => {
            let s = makeState();
            s = addFavorite(s, "timer", "t-1");
            expect(isFavorite(s, "timer", "t-1")).toBe(true);
            expect(isFavorite(s, "counter", "t-1")).toBe(false);
        });

        it("addFavorite is idempotent", () => {
            const initial = addFavorite(makeState(), "timer", "t-1");
            const second = addFavorite(initial, "timer", "t-1");
            expect(second).toBe(initial);
        });

        it("removeFavorite filters items", () => {
            const initial = makeState([
                {kind: "timer", cardId: "t-1", sortKey: 1},
                {kind: "counter", cardId: "c-1", sortKey: 2},
            ]);
            const next = removeFavorite(initial, "timer", "t-1");
            expect(next.items).toEqual([{kind: "counter", cardId: "c-1", sortKey: 2}]);
            expect(removeFavorite(initial, "rapidfire", "r-1")).toBe(initial);
        });

        it("toggleFavorite flips state", () => {
            let s = makeState();
            s = toggleFavorite(s, "timer", "t-1");
            expect(isFavorite(s, "timer", "t-1")).toBe(true);
            s = toggleFavorite(s, "timer", "t-1");
            expect(isFavorite(s, "timer", "t-1")).toBe(false);
        });
    });

    describe("sortFavorites / renumberFavorites", () => {
        it("sorts ascending by sortKey", () => {
            const items: FavoriteItem[] = [
                {kind: "timer", cardId: "a", sortKey: 5},
                {kind: "timer", cardId: "b", sortKey: 1},
                {kind: "timer", cardId: "c", sortKey: 3},
            ];
            expect(sortFavorites(items).map((item) => item.cardId)).toEqual(["b", "c", "a"]);
        });

        it("renumbers in input order", () => {
            const items: FavoriteItem[] = [
                {kind: "timer", cardId: "c", sortKey: 99},
                {kind: "timer", cardId: "a", sortKey: 7},
                {kind: "timer", cardId: "b", sortKey: 13},
            ];
            const renumbered = renumberFavorites(items);
            expect(renumbered.map((item) => item.cardId)).toEqual(["c", "a", "b"]);
            expect(renumbered.map((item) => item.sortKey)).toEqual([1024, 2048, 3072]);
        });
    });

    describe("moveFavorite (fractional indexing)", () => {
        it("moves to start", () => {
            const s = makeState([
                {kind: "timer", cardId: "a", sortKey: 1},
                {kind: "timer", cardId: "b", sortKey: 2},
                {kind: "timer", cardId: "c", sortKey: 3},
            ]);
            const moved = moveFavorite(s, "timer", "c", "start");
            expect(sortFavorites(moved.items).map((item) => item.cardId)).toEqual(["c", "a", "b"]);
        });

        it("moves to end", () => {
            const s = makeState([
                {kind: "timer", cardId: "a", sortKey: 1},
                {kind: "timer", cardId: "b", sortKey: 2},
                {kind: "timer", cardId: "c", sortKey: 3},
            ]);
            const moved = moveFavorite(s, "timer", "a", "end");
            expect(sortFavorites(moved.items).map((item) => item.cardId)).toEqual(["b", "c", "a"]);
        });

        it("moves to a position between two items using midpoint", () => {
            const s = makeState([
                {kind: "timer", cardId: "a", sortKey: 100},
                {kind: "timer", cardId: "b", sortKey: 200},
                {kind: "timer", cardId: "c", sortKey: 300},
            ]);
            const moved = moveFavorite(s, "timer", "a", {after: "b"});
            const ordered = sortFavorites(moved.items);
            expect(ordered.map((item) => item.cardId)).toEqual(["b", "a", "c"]);
            const a = ordered.find((item) => item.cardId === "a")!;
            expect(a.sortKey).toBe(250);
        });

        it("moves before a specific item", () => {
            const s = makeState([
                {kind: "timer", cardId: "a", sortKey: 100},
                {kind: "timer", cardId: "b", sortKey: 200},
                {kind: "timer", cardId: "c", sortKey: 300},
            ]);
            const moved = moveFavorite(s, "timer", "c", {before: "a"});
            expect(sortFavorites(moved.items).map((item) => item.cardId)).toEqual(["c", "a", "b"]);
        });

        it("renumbers when midpoint precision is exhausted", () => {
            const tinyGap: FavoritesState = {
                items: [
                    {kind: "timer", cardId: "a", sortKey: 1},
                    {kind: "timer", cardId: "b", sortKey: 1 + 1e-9},
                ],
                view: {...DEFAULT_FAVORITES_VIEW},
            };
            const moved = moveFavorite(tinyGap, "timer", "a", {after: "b"});
            const sortKeys = sortFavorites(moved.items).map((item) => item.sortKey);
            // After renumber, sort keys should be 1024 and 2048 (well-spaced).
            expect(sortKeys[1]! - sortKeys[0]!).toBeGreaterThan(1);
        });

        it("returns the same state when the item does not exist", () => {
            const s = makeState([{kind: "timer", cardId: "a", sortKey: 1}]);
            expect(moveFavorite(s, "timer", "missing", "start")).toBe(s);
        });
    });

    describe("updateFavoritesView", () => {
        it("patches the view subset", () => {
            const initial = makeState();
            const next = updateFavoritesView(initial, {compactMode: true, showHotkey: false});
            expect(next.view.compactMode).toBe(true);
            expect(next.view.showHotkey).toBe(false);
            expect(next.view.showProgress).toBe(DEFAULT_FAVORITES_VIEW.showProgress);
            expect(next.view.showCounter).toBe(DEFAULT_FAVORITES_VIEW.showCounter);
        });
    });

    describe("pruneFavorites", () => {
        it("keeps only items whose key is in the valid set", () => {
            const s = makeState([
                {kind: "timer", cardId: "a", sortKey: 1},
                {kind: "timer", cardId: "b", sortKey: 2},
                {kind: "counter", cardId: "c", sortKey: 3},
            ]);
            const validKeys = new Set([
                favoriteKey("timer", "a"),
                favoriteKey("counter", "c"),
            ]);
            const pruned = pruneFavorites(s, validKeys);
            expect(pruned.items.map((item) => item.cardId).sort()).toEqual(["a", "c"]);
        });

        it("returns the same state when nothing changes", () => {
            const s = makeState([{kind: "timer", cardId: "a", sortKey: 1}]);
            const validKeys = new Set([favoriteKey("timer", "a")]);
            expect(pruneFavorites(s, validKeys)).toBe(s);
        });

        it("Counter 尚未加载或加载失败时保留全部收藏", () => {
            const state = makeState([
                {kind: "timer", cardId: "timer-1", sortKey: 1},
                {kind: "counter", cardId: "counter-1", sortKey: 2},
                {kind: "rapidfire", cardId: "rapidfire-1", sortKey: 3},
            ]);

            expect(pruneFavoritesWithSources(state, {
                timer: new Set(["timer-1"]),
                rapidfire: new Set(["rapidfire-1"]),
            })).toBe(state);
        });

        it("三类来源全部 ready 后才删除孤儿收藏", () => {
            const state = makeState([
                {kind: "timer", cardId: "timer-1", sortKey: 1},
                {kind: "counter", cardId: "missing-counter", sortKey: 2},
                {kind: "rapidfire", cardId: "rapidfire-1", sortKey: 3},
            ]);

            const next = pruneFavoritesWithSources(state, {
                timer: new Set(["timer-1"]),
                counter: new Set(["counter-1"]),
                rapidfire: new Set(["rapidfire-1"]),
            });

            expect(next.items.map((item) => item.cardId)).toEqual(["timer-1", "rapidfire-1"]);
        });
    });

    describe("settleFavoriteBootstraps", () => {
        it("等待 delayed Counter 后再结束三类加载", async () => {
            const counter = deferred<string>();
            let settled = false;
            const result = settleFavoriteBootstraps(
                Promise.resolve("timer"),
                counter.promise,
                Promise.resolve("rapidfire"),
            ).finally(() => {
                settled = true;
            });

            await Promise.resolve();
            expect(settled).toBe(false);

            counter.resolve("counter");
            await expect(result).resolves.toEqual({
                timer: "timer",
                counter: "counter",
                rapidfire: "rapidfire",
            });
            expect(settled).toBe(true);
        });

        it("Counter reject 时保留其失败状态且消费 rejection", async () => {
            const counter = deferred<string>();
            const result = settleFavoriteBootstraps(
                Promise.resolve("timer"),
                counter.promise,
                Promise.resolve("rapidfire"),
            );

            counter.reject(new Error("Counter 加载失败"));

            await expect(result).resolves.toEqual({
                timer: "timer",
                counter: null,
                rapidfire: "rapidfire",
            });
        });
    });

    describe("storage round-trip", () => {
        it("returns default state when storage is null", () => {
            const state = readStoredFavorites(null);
            expect(state.items).toEqual([]);
            expect(state.view).toEqual(DEFAULT_FAVORITES_VIEW);
        });

        it("persists and reads back state", () => {
            const stub = makeStub();
            const state: FavoritesState = {
                items: [
                    {kind: "timer", cardId: "a", sortKey: 1024},
                    {kind: "counter", cardId: "b", sortKey: 2048},
                ],
                view: {showHotkey: false, showProgress: true, showCounter: false, compactMode: true},
            };
            writeStoredFavorites(state, stub);
            const restored = readStoredFavorites(stub);
            expect(restored).toEqual(state);
        });

        it("returns default state on corrupted JSON", () => {
            const stub = makeStub();
            stub.setItem("delta-auto-tools:favorites:v1", "not json");
            const state = readStoredFavorites(stub);
            expect(state.items).toEqual([]);
            expect(state.view).toEqual(DEFAULT_FAVORITES_VIEW);
        });

        it("returns default state on wrong shape", () => {
            const stub = makeStub();
            stub.setItem("delta-auto-tools:favorites:v1", JSON.stringify([1, 2, 3]));
            const state = readStoredFavorites(stub);
            expect(state.items).toEqual([]);
        });

        it("filters malformed items but keeps the well-formed ones", () => {
            const stub = makeStub();
            stub.setItem(
                "delta-auto-tools:favorites:v1",
                JSON.stringify({
                    items: [
                        {kind: "timer", cardId: "a", sortKey: 1},
                        {kind: "invalid", cardId: "b", sortKey: 2},
                        {kind: "timer", cardId: "c", sortKey: Number.NaN},
                        {kind: "timer", cardId: 42, sortKey: 3},
                        {kind: "timer", cardId: "d", sortKey: 4},
                    ],
                    view: {showHotkey: "yes"},
                }),
            );
            const state = readStoredFavorites(stub);
            expect(state.items.map((item) => item.cardId)).toEqual(["a", "d"]);
            expect(state.view.showHotkey).toBe(DEFAULT_FAVORITES_VIEW.showHotkey);
        });

        it("view with partial fields keeps defaults for missing ones", () => {
            const stub = makeStub();
            stub.setItem(
                "delta-auto-tools:favorites:v1",
                JSON.stringify({items: [], view: {compactMode: true}}),
            );
            const state = readStoredFavorites(stub);
            expect(state.view).toEqual({...DEFAULT_FAVORITES_VIEW, compactMode: true});
        });
    });
});

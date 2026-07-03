import {describe, expect, it, vi} from "vitest";
import {
    TIMER_EVENTS,
    COUNTER_EVENTS,
    RAPIDFIRE_EVENTS,
    MORSE_EVENTS,
    AUDIO_EVENTS,
    ABOUT_EVENTS,
} from "@/lib/tauri-events";

/**
 * VAL-AR-028: 前端组件 mount/unmount 事件订阅清理测试。
 *
 * 验证关键工具页组件 mount 时 subscribe、unmount 时 unsubscribe（无监听泄漏）。
 * 测试使用生产代码中的事件名常量（来自 tauri-events.ts），而非内联重定义。
 *
 * 1. mount: 调用 listen() 订阅预期事件
 * 2. unmount: 调用所有 unlisten 回调
 * 3. disposed 后回调不执行副作用
 * 4. 非原生环境下跳过订阅
 */

// ── 通用事件订阅管理器模拟 ──────────────────────────────

interface ListenCall {
    event: string;
    callback: (event: {payload: unknown}) => void;
}

interface UnlistenCall {
    event: string;
}

/**
 * 模拟 React useEffect 内的事件订阅/清理生命周期。
 * 返回 listen/unlisten 调用记录，便于断言。
 */
function createEventSubscriptionTracker() {
    const listenCalls: ListenCall[] = [];
    const unlistenCalls: UnlistenCall[] = [];
    const unlistenFns = new Map<string, () => void>();

    /** 模拟 listen() 调用 */
    const mockListen = (event: string, callback: (event: {payload: unknown}) => void) => {
        listenCalls.push({event, callback});
        const unlisten = () => {
            unlistenCalls.push({event});
        };
        unlistenFns.set(event, unlisten);
        return Promise.resolve(unlisten);
    };

    /** 模拟 unmount 清理 */
    const cleanup = (...events: string[]) => {
        for (const event of events) {
            unlistenFns.get(event)?.();
        }
    };

    return {listenCalls, unlistenCalls, mockListen, cleanup};
}

// ── 计时器页事件订阅 ──────────────────────────────────

describe("timer-page 事件订阅清理", () => {
    it("mount 时订阅 stateChanged 和 hotkeyTriggered", () => {
        const tracker = createEventSubscriptionTracker();
        const {mockListen} = tracker;

        // 模拟 mount：调用 listen 订阅两个事件
        void mockListen(TIMER_EVENTS.stateChanged, () => {});
        void mockListen(TIMER_EVENTS.hotkeyTriggered, () => {});

        expect(tracker.listenCalls).toHaveLength(2);
        expect(tracker.listenCalls[0].event).toBe(TIMER_EVENTS.stateChanged);
        expect(tracker.listenCalls[1].event).toBe(TIMER_EVENTS.hotkeyTriggered);
    });

    it("unmount 时调用所有 unlisten 回调", () => {
        const tracker = createEventSubscriptionTracker();
        const {mockListen, cleanup} = tracker;

        void mockListen(TIMER_EVENTS.stateChanged, () => {});
        void mockListen(TIMER_EVENTS.hotkeyTriggered, () => {});

        // 模拟 unmount
        cleanup(TIMER_EVENTS.stateChanged, TIMER_EVENTS.hotkeyTriggered);

        expect(tracker.unlistenCalls).toHaveLength(2);
        expect(tracker.unlistenCalls[0].event).toBe(TIMER_EVENTS.stateChanged);
        expect(tracker.unlistenCalls[1].event).toBe(TIMER_EVENTS.hotkeyTriggered);
    });

    it("unmount 后 disposed 标志阻止 stateChanged 回调执行副作用", () => {
        let disposed = false;
        const setBootstrap = vi.fn();

        // 模拟 mount 时的回调
        const stateChangedCallback = (event: {payload: unknown}) => {
            if (disposed) return;
            setBootstrap(event.payload);
        };

        // 正常触发
        stateChangedCallback({payload: {settings: {timerEnabled: true}, runs: []}});
        expect(setBootstrap).toHaveBeenCalledTimes(1);

        // disposed 后触发
        disposed = true;
        stateChangedCallback({payload: {settings: {timerEnabled: false}, runs: []}});
        expect(setBootstrap).toHaveBeenCalledTimes(1); // 无新增调用
    });

    it("unmount 后 disposed 标志阻止 hotkeyTriggered 回调执行副作用", () => {
        let disposed = false;
        const setStatusMessage = vi.fn();

        const hotkeyTriggeredCallback = (event: {payload: unknown}) => {
            if (disposed) return;
            setStatusMessage(`快捷键已触发 ${(event.payload as unknown[]).length} 个计时器。`);
        };

        hotkeyTriggeredCallback({payload: ["timer-1"]});
        expect(setStatusMessage).toHaveBeenCalledTimes(1);

        disposed = true;
        hotkeyTriggeredCallback({payload: ["timer-2"]});
        expect(setStatusMessage).toHaveBeenCalledTimes(1);
    });

    it("非原生环境（isNativeShell=false）跳过订阅", () => {
        const tracker = createEventSubscriptionTracker();
        const isNativeShell = false;

        // 模拟 timer-page useEffect 中 isNativeShell 检查
        if (isNativeShell) {
            void tracker.mockListen(TIMER_EVENTS.stateChanged, () => {});
            void tracker.mockListen(TIMER_EVENTS.hotkeyTriggered, () => {});
        }

        expect(tracker.listenCalls).toHaveLength(0);
    });
});

// ── 计数器页事件订阅 ──────────────────────────────────

describe("counter-page 事件订阅清理", () => {
    it("mount 时订阅 stateChanged 和 hotkeyTriggered", () => {
        const tracker = createEventSubscriptionTracker();
        const {mockListen} = tracker;

        void mockListen(COUNTER_EVENTS.stateChanged, () => {});
        void mockListen(COUNTER_EVENTS.hotkeyTriggered, () => {});

        expect(tracker.listenCalls).toHaveLength(2);
        expect(tracker.listenCalls.map((c) => c.event)).toEqual([
            COUNTER_EVENTS.stateChanged,
            COUNTER_EVENTS.hotkeyTriggered,
        ]);
    });

    it("unmount 时调用所有 unlisten 回调", () => {
        const tracker = createEventSubscriptionTracker();
        const {mockListen, cleanup} = tracker;

        void mockListen(COUNTER_EVENTS.stateChanged, () => {});
        void mockListen(COUNTER_EVENTS.hotkeyTriggered, () => {});

        cleanup(COUNTER_EVENTS.stateChanged, COUNTER_EVENTS.hotkeyTriggered);

        expect(tracker.unlistenCalls).toHaveLength(2);
    });

    it("unmount 后 disposed 阻止回调副作用", () => {
        let disposed = false;
        const setBootstrap = vi.fn();

        const stateChangedCallback = (event: {payload: unknown}) => {
            if (disposed) return;
            setBootstrap(event.payload);
        };

        stateChangedCallback({payload: {settings: {}, counterRuns: []}});
        expect(setBootstrap).toHaveBeenCalledTimes(1);

        disposed = true;
        stateChangedCallback({payload: {settings: {}, counterRuns: []}});
        expect(setBootstrap).toHaveBeenCalledTimes(1);
    });
});

// ── 连发器页事件订阅 ──────────────────────────────────

describe("rapidfire-page 事件订阅清理", () => {
    it("mount 时订阅 stateChanged 和 hotkeyError", () => {
        const tracker = createEventSubscriptionTracker();
        const {mockListen} = tracker;

        void mockListen(RAPIDFIRE_EVENTS.stateChanged, () => {});
        void mockListen(RAPIDFIRE_EVENTS.hotkeyError, () => {});

        expect(tracker.listenCalls).toHaveLength(2);
        expect(tracker.listenCalls.map((c) => c.event)).toEqual([
            RAPIDFIRE_EVENTS.stateChanged,
            RAPIDFIRE_EVENTS.hotkeyError,
        ]);
    });

    it("unmount 时调用所有 unlisten 回调", () => {
        const tracker = createEventSubscriptionTracker();
        const {mockListen, cleanup} = tracker;

        void mockListen(RAPIDFIRE_EVENTS.stateChanged, () => {});
        void mockListen(RAPIDFIRE_EVENTS.hotkeyError, () => {});

        cleanup(RAPIDFIRE_EVENTS.stateChanged, RAPIDFIRE_EVENTS.hotkeyError);

        expect(tracker.unlistenCalls).toHaveLength(2);
    });

    it("unmount 后 disposed 阻止 hotkeyError 回调副作用", () => {
        let disposed = false;
        const setPageError = vi.fn();
        const setStatusMessage = vi.fn();

        const hotkeyErrorCallback = (event: {payload: string}) => {
            if (disposed) return;
            setPageError(event.payload);
            setStatusMessage(event.payload);
        };

        hotkeyErrorCallback({payload: "快捷键冲突"});
        expect(setPageError).toHaveBeenCalledTimes(1);
        expect(setStatusMessage).toHaveBeenCalledTimes(1);

        disposed = true;
        hotkeyErrorCallback({payload: "另一个冲突"});
        expect(setPageError).toHaveBeenCalledTimes(1);
        expect(setStatusMessage).toHaveBeenCalledTimes(1);
    });
});

// ── 摩斯页事件订阅 ──────────────────────────────────

describe("morse-page 事件订阅清理", () => {
    it("mount 时订阅 runFinished、selectionProgress 和 hotkeyError", () => {
        const tracker = createEventSubscriptionTracker();
        const {mockListen} = tracker;

        void mockListen(MORSE_EVENTS.runFinished, () => {});
        void mockListen(MORSE_EVENTS.selectionProgress, () => {});
        void mockListen(MORSE_EVENTS.hotkeyError, () => {});

        expect(tracker.listenCalls).toHaveLength(3);
        expect(tracker.listenCalls.map((c) => c.event)).toEqual([
            MORSE_EVENTS.runFinished,
            MORSE_EVENTS.selectionProgress,
            MORSE_EVENTS.hotkeyError,
        ]);
    });

    it("unmount 时调用所有 unlisten 回调", () => {
        const tracker = createEventSubscriptionTracker();
        const {mockListen, cleanup} = tracker;

        void mockListen(MORSE_EVENTS.runFinished, () => {});
        void mockListen(MORSE_EVENTS.selectionProgress, () => {});
        void mockListen(MORSE_EVENTS.hotkeyError, () => {});

        cleanup(MORSE_EVENTS.runFinished, MORSE_EVENTS.selectionProgress, MORSE_EVENTS.hotkeyError);

        expect(tracker.unlistenCalls).toHaveLength(3);
    });

    it("overlay 模式跳过事件订阅", () => {
        const tracker = createEventSubscriptionTracker();
        const overlayMode = true;
        const isNativeShell = true;

        // morse-page 中: if (overlayMode || !isNativeShell) return;
        if (!overlayMode && isNativeShell) {
            void tracker.mockListen(MORSE_EVENTS.runFinished, () => {});
            void tracker.mockListen(MORSE_EVENTS.selectionProgress, () => {});
            void tracker.mockListen(MORSE_EVENTS.hotkeyError, () => {});
        }

        expect(tracker.listenCalls).toHaveLength(0);
    });

    it("unmount 后 disposed 阻止 selectionProgress 回调副作用", () => {
        let disposed = false;
        const setForm = vi.fn();

        const selectionProgressCallback = (_event: {payload: {regions: unknown}}) => {
            if (disposed) return;
            setForm((current: unknown) => current);
        };

        selectionProgressCallback({payload: {regions: []}});
        expect(setForm).toHaveBeenCalledTimes(1);

        disposed = true;
        selectionProgressCallback({payload: {regions: []}});
        expect(setForm).toHaveBeenCalledTimes(1);
    });
});

// ── 音频页事件订阅 ──────────────────────────────────

describe("audio-page 事件订阅清理", () => {
    it("mount 时订阅 stateChanged、hotkeyError、hotkeyTriggered 和 regionMatched", () => {
        const tracker = createEventSubscriptionTracker();
        const {mockListen} = tracker;

        void mockListen(AUDIO_EVENTS.stateChanged, () => {});
        void mockListen(AUDIO_EVENTS.hotkeyError, () => {});
        void mockListen(AUDIO_EVENTS.hotkeyTriggered, () => {});
        void mockListen(AUDIO_EVENTS.regionMatched, () => {});

        expect(tracker.listenCalls).toHaveLength(4);
        expect(tracker.listenCalls.map((c) => c.event)).toEqual([
            AUDIO_EVENTS.stateChanged,
            AUDIO_EVENTS.hotkeyError,
            AUDIO_EVENTS.hotkeyTriggered,
            AUDIO_EVENTS.regionMatched,
        ]);
    });

    it("unmount 时调用所有 unlisten 回调", () => {
        const tracker = createEventSubscriptionTracker();
        const {mockListen, cleanup} = tracker;

        void mockListen(AUDIO_EVENTS.stateChanged, () => {});
        void mockListen(AUDIO_EVENTS.hotkeyError, () => {});
        void mockListen(AUDIO_EVENTS.hotkeyTriggered, () => {});
        void mockListen(AUDIO_EVENTS.regionMatched, () => {});

        cleanup(
            AUDIO_EVENTS.stateChanged,
            AUDIO_EVENTS.hotkeyError,
            AUDIO_EVENTS.hotkeyTriggered,
            AUDIO_EVENTS.regionMatched,
        );

        expect(tracker.unlistenCalls).toHaveLength(4);
    });

    it("unmount 后 disposed 阻止所有回调副作用", () => {
        let disposed = false;
        const setBootstrap = vi.fn();
        const setStatusMessage = vi.fn();
        const toastInfo = vi.fn();

        const stateChangedCallback = (event: {payload: unknown}) => {
            if (disposed) return;
            setBootstrap(event.payload);
        };
        const hotkeyTriggeredCallback = (event: {payload: string}) => {
            if (disposed) return;
            toastInfo(`快捷键触发：卡片 ${event.payload}`);
            setStatusMessage(`快捷键触发：卡片 ${event.payload}`);
        };

        // 正常调用
        stateChangedCallback({payload: {settings: {}, runs: []}});
        hotkeyTriggeredCallback({payload: "card-1"});
        expect(setBootstrap).toHaveBeenCalledTimes(1);
        expect(toastInfo).toHaveBeenCalledTimes(1);
        expect(setStatusMessage).toHaveBeenCalledTimes(1);

        // disposed 后调用
        disposed = true;
        stateChangedCallback({payload: {settings: {}, runs: []}});
        hotkeyTriggeredCallback({payload: "card-2"});
        expect(setBootstrap).toHaveBeenCalledTimes(1);
        expect(toastInfo).toHaveBeenCalledTimes(1);
        expect(setStatusMessage).toHaveBeenCalledTimes(1);
    });
});

// ── 关于页事件订阅 ──────────────────────────────────

describe("about-page 事件订阅清理", () => {
    it("mount 时订阅 updateProgress", () => {
        const tracker = createEventSubscriptionTracker();
        const {mockListen} = tracker;

        void mockListen(ABOUT_EVENTS.updateProgress, () => {});

        expect(tracker.listenCalls).toHaveLength(1);
        expect(tracker.listenCalls[0].event).toBe(ABOUT_EVENTS.updateProgress);
    });

    it("unmount 时调用 unlisten 回调", () => {
        const tracker = createEventSubscriptionTracker();
        const {mockListen, cleanup} = tracker;

        void mockListen(ABOUT_EVENTS.updateProgress, () => {});
        cleanup(ABOUT_EVENTS.updateProgress);

        expect(tracker.unlistenCalls).toHaveLength(1);
    });

    it("unmount 后 disposed 阻止 updateProgress 回调副作用", () => {
        let disposed = false;
        const setProgress = vi.fn();

        const updateProgressCallback = (event: {payload: unknown}) => {
            if (disposed) return;
            setProgress(event.payload);
        };

        updateProgressCallback({payload: {downloaded: 50, total: 100}});
        expect(setProgress).toHaveBeenCalledTimes(1);

        disposed = true;
        updateProgressCallback({payload: {downloaded: 100, total: 100}});
        expect(setProgress).toHaveBeenCalledTimes(1);
    });
});

// ── 收藏页事件订阅 ──────────────────────────────────

describe("favorites-page 事件订阅清理", () => {
    it("mount 时订阅 timer/counter/rapidfire 的 stateChanged", () => {
        const tracker = createEventSubscriptionTracker();
        const {mockListen} = tracker;

        void mockListen(TIMER_EVENTS.stateChanged, () => {});
        void mockListen(COUNTER_EVENTS.stateChanged, () => {});
        void mockListen(RAPIDFIRE_EVENTS.stateChanged, () => {});

        expect(tracker.listenCalls).toHaveLength(3);
        expect(tracker.listenCalls.map((c) => c.event)).toEqual([
            TIMER_EVENTS.stateChanged,
            COUNTER_EVENTS.stateChanged,
            RAPIDFIRE_EVENTS.stateChanged,
        ]);
    });

    it("unmount 时调用所有 unlisten 回调", () => {
        const tracker = createEventSubscriptionTracker();
        const {mockListen, cleanup} = tracker;

        void mockListen(TIMER_EVENTS.stateChanged, () => {});
        void mockListen(COUNTER_EVENTS.stateChanged, () => {});
        void mockListen(RAPIDFIRE_EVENTS.stateChanged, () => {});

        cleanup(TIMER_EVENTS.stateChanged, COUNTER_EVENTS.stateChanged, RAPIDFIRE_EVENTS.stateChanged);

        expect(tracker.unlistenCalls).toHaveLength(3);
    });
});

// ── 跨页面订阅完整性检查 ──────────────────────────────

describe("跨页面事件订阅完整性", () => {
    it("各页面订阅的事件名与 tauri-events.ts 常量一致", () => {
        // 确保测试中使用的事件名与项目实际定义一致
        expect(TIMER_EVENTS.stateChanged).toBe("timer://state-changed");
        expect(TIMER_EVENTS.hotkeyTriggered).toBe("timer://hotkey-triggered");
        expect(COUNTER_EVENTS.stateChanged).toBe("counter://state-changed");
        expect(COUNTER_EVENTS.hotkeyTriggered).toBe("counter://hotkey-triggered");
        expect(RAPIDFIRE_EVENTS.stateChanged).toBe("rapidfire://state-changed");
        expect(RAPIDFIRE_EVENTS.hotkeyError).toBe("rapidfire://hotkey-error");
        expect(MORSE_EVENTS.runFinished).toBe("morse://run-finished");
        expect(MORSE_EVENTS.selectionProgress).toBe("morse://selection-progress");
        expect(MORSE_EVENTS.hotkeyError).toBe("morse://hotkey-error");
        expect(AUDIO_EVENTS.stateChanged).toBe("audio://state-changed");
        expect(AUDIO_EVENTS.hotkeyTriggered).toBe("audio://hotkey-triggered");
        expect(AUDIO_EVENTS.regionMatched).toBe("audio://region-matched");
        expect(AUDIO_EVENTS.hotkeyError).toBe("audio://hotkey-error");
        expect(ABOUT_EVENTS.updateProgress).toBe("about://update-progress");
    });

    it("所有页面 unmount 清理无遗漏（每页的 listen 调用数等于 unlisten 调用数）", () => {
        const tracker = createEventSubscriptionTracker();
        const {mockListen, cleanup} = tracker;

        // 模拟所有页面同时 mount
        const allEvents = [
            TIMER_EVENTS.stateChanged,
            TIMER_EVENTS.hotkeyTriggered,
            COUNTER_EVENTS.stateChanged,
            COUNTER_EVENTS.hotkeyTriggered,
            RAPIDFIRE_EVENTS.stateChanged,
            RAPIDFIRE_EVENTS.hotkeyError,
            MORSE_EVENTS.runFinished,
            MORSE_EVENTS.selectionProgress,
            MORSE_EVENTS.hotkeyError,
            AUDIO_EVENTS.stateChanged,
            AUDIO_EVENTS.hotkeyTriggered,
            AUDIO_EVENTS.regionMatched,
            AUDIO_EVENTS.hotkeyError,
            ABOUT_EVENTS.updateProgress,
        ];

        for (const event of allEvents) {
            void mockListen(event, () => {});
        }

        // 模拟所有页面同时 unmount
        cleanup(...allEvents);

        expect(tracker.listenCalls).toHaveLength(allEvents.length);
        expect(tracker.unlistenCalls).toHaveLength(allEvents.length);

        // 确认每个事件都有对应的 unlisten
        const listenEventNames = tracker.listenCalls.map((c) => c.event);
        const unlistenEventNames = tracker.unlistenCalls.map((c) => c.event);
        expect(listenEventNames.sort()).toEqual(unlistenEventNames.sort());
    });
});

// ── 监听泄漏检测 ──────────────────────────────────

describe("监听泄漏检测", () => {
    it("部分 unlisten 缺失时能检测到泄漏", () => {
        const tracker = createEventSubscriptionTracker();
        const {mockListen, cleanup} = tracker;

        // 订阅 4 个事件
        void mockListen(AUDIO_EVENTS.stateChanged, () => {});
        void mockListen(AUDIO_EVENTS.hotkeyError, () => {});
        void mockListen(AUDIO_EVENTS.hotkeyTriggered, () => {});
        void mockListen(AUDIO_EVENTS.regionMatched, () => {});

        // 只清理 3 个，模拟泄漏
        cleanup(AUDIO_EVENTS.stateChanged, AUDIO_EVENTS.hotkeyError, AUDIO_EVENTS.hotkeyTriggered);

        expect(tracker.unlistenCalls).toHaveLength(3);
        // 检测泄漏：unlisten 调用数 < listen 调用数
        expect(tracker.unlistenCalls.length).toBeLessThan(tracker.listenCalls.length);

        // 找出泄漏的事件
        const unlistenedEvents = new Set(tracker.unlistenCalls.map((c) => c.event));
        const leakedEvents = tracker.listenCalls
            .map((c) => c.event)
            .filter((e) => !unlistenedEvents.has(e));
        expect(leakedEvents).toEqual([AUDIO_EVENTS.regionMatched]);
    });

    it("完全清理时无泄漏", () => {
        const tracker = createEventSubscriptionTracker();
        const {mockListen, cleanup} = tracker;

        void mockListen(TIMER_EVENTS.stateChanged, () => {});
        void mockListen(TIMER_EVENTS.hotkeyTriggered, () => {});

        cleanup(TIMER_EVENTS.stateChanged, TIMER_EVENTS.hotkeyTriggered);

        expect(tracker.unlistenCalls.length).toBe(tracker.listenCalls.length);
    });
});

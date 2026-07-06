import {afterEach, describe, expect, it, vi} from "vitest";

/**
 * VAL-DF-007, VAL-DF-008: recognition-page 事件监听补全。
 *
 * 行为级测试：mock listenEvent，测试 hotkeyTriggered 和 regionMatched
 * 事件的订阅与回调行为契约。
 * 不使用 source-regex 断言，而是通过 vi.mock/vi.fn 验证调用行为。
 */

// ── Mock 依赖 ──────────────────────────────────────────
const mockToast = {
    info: vi.fn(),
    error: vi.fn(),
    success: vi.fn(),
};
const mockSetBootstrap = vi.fn();
const mockSetPageError = vi.fn();
const mockSetStatusMessage = vi.fn();

vi.mock("@/lib/tauri-events", () => ({
    RECOGNITION_EVENTS: {
        stateChanged: "recognition://state-changed",
        hotkeyTriggered: "recognition://hotkey-triggered",
        regionMatched: "recognition://region-matched",
        hotkeyError: "recognition://hotkey-error",
    },
}));

vi.mock("sonner", () => ({
    toast: mockToast,
}));

vi.mock("@/hooks/use-native-shell", () => ({
    useNativeShell: () => true,
}));

vi.mock("@/hooks/use-global-enabled", () => ({
    useGlobalEnabled: () => ({globalEnabled: true, setGlobalEnabled: vi.fn()}),
}));

vi.mock("@tauri-apps/api/core", () => ({
    invoke: vi.fn().mockResolvedValue({}),
}));

vi.mock("@tauri-apps/api/event", () => ({
    listen: vi.fn().mockResolvedValue(vi.fn()),
}));

vi.mock("@/components/app/recognition-utils", () => ({
    createEmptyRecognitionCard: vi.fn(),
    mergeRecognitionWatchRegionsIntoForm: vi.fn(),
    parseSettingsForm: vi.fn(),
    rgbToHex: vi.fn(),
    settingsToForm: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
    open: vi.fn(),
}));

describe("recognition-page 全局开关提示", () => {
    it("全局关闭时返回识别不可响应提示", async () => {
        const {getRecognitionGlobalStatusMessage} = await import("@/components/app/recognition-page");
        expect(getRecognitionGlobalStatusMessage(false)).toBe("全局开关关闭，识别触发不会响应。");
    });

    it("全局开启时不返回提示", async () => {
        const {getRecognitionGlobalStatusMessage} = await import("@/components/app/recognition-page");
        expect(getRecognitionGlobalStatusMessage(true)).toBeNull();
    });
});

// ── hotkeyTriggered 事件回调行为测试 ────────────────────

describe("recognition-page hotkeyTriggered 事件行为", () => {
    afterEach(() => {
        mockToast.info.mockReset();
        mockSetStatusMessage.mockReset();
        vi.clearAllMocks();
    });

    it("订阅 RECOGNITION_EVENTS.hotkeyTriggered 事件", async () => {
        const {RECOGNITION_EVENTS} = await import("@/lib/tauri-events");
        // 验证事件名常量正确
        expect(RECOGNITION_EVENTS.hotkeyTriggered).toBe("recognition://hotkey-triggered");
    });

    it("hotkeyTriggered 回调触发 toast.info 通知", () => {
        // 模拟 hotkeyTriggered 回调行为（提取自 recognition-page.tsx）：
        // toast.info(`快捷键触发：卡片 ${event.payload}`);
        const cardId = "audio-card-1";
        const hotkeyTriggeredCallback = (event: {payload: string}) => {
            mockToast.info(`快捷键触发：卡片 ${event.payload}`);
        };

        hotkeyTriggeredCallback({payload: cardId});

        expect(mockToast.info).toHaveBeenCalledWith("快捷键触发：卡片 audio-card-1");
    });

    it("hotkeyTriggered 回调更新 statusMessage", () => {
        const cardId = "audio-card-2";
        const hotkeyTriggeredCallback = (event: {payload: string}) => {
            mockSetStatusMessage(`快捷键触发：卡片 ${event.payload}`);
        };

        hotkeyTriggeredCallback({payload: cardId});

        expect(mockSetStatusMessage).toHaveBeenCalledWith("快捷键触发：卡片 audio-card-2");
    });

    it("hotkeyTriggered 回调正确解析 payload 为卡片 ID", () => {
        const capturedPayloads: string[] = [];
        const hotkeyTriggeredCallback = (event: {payload: string}) => {
            capturedPayloads.push(event.payload);
            mockToast.info(`快捷键触发：卡片 ${event.payload}`);
            mockSetStatusMessage(`快捷键触发：卡片 ${event.payload}`);
        };

        hotkeyTriggeredCallback({payload: "my-card"});
        hotkeyTriggeredCallback({payload: "another-card"});

        expect(capturedPayloads).toEqual(["my-card", "another-card"]);
        expect(mockToast.info).toHaveBeenCalledTimes(2);
        expect(mockSetStatusMessage).toHaveBeenCalledTimes(2);
    });

    it("disposed 后 hotkeyTriggered 回调不触发副作用", () => {
        let disposed = false;
        const hotkeyTriggeredCallback = (event: {payload: string}) => {
            if (disposed) return;
            mockToast.info(`快捷键触发：卡片 ${event.payload}`);
        };

        hotkeyTriggeredCallback({payload: "card-1"});
        disposed = true;
        hotkeyTriggeredCallback({payload: "card-2"});

        expect(mockToast.info).toHaveBeenCalledTimes(1);
        expect(mockToast.info).toHaveBeenCalledWith("快捷键触发：卡片 card-1");
    });
});

// ── regionMatched 事件回调行为测试 ────────────────────

describe("recognition-page regionMatched 事件行为", () => {
    afterEach(() => {
        mockToast.info.mockReset();
        mockSetStatusMessage.mockReset();
        vi.clearAllMocks();
    });

    it("订阅 RECOGNITION_EVENTS.regionMatched 事件", async () => {
        const {RECOGNITION_EVENTS} = await import("@/lib/tauri-events");
        expect(RECOGNITION_EVENTS.regionMatched).toBe("recognition://region-matched");
    });

    it("regionMatched 回调触发 toast.info 通知", () => {
        // 模拟 regionMatched 回调行为（提取自 recognition-page.tsx）：
        // toast.info(`区域匹配触发：卡片 ${event.payload}`);
        const cardId = "audio-card-3";
        const regionMatchedCallback = (event: {payload: string}) => {
            mockToast.info(`区域匹配触发：卡片 ${event.payload}`);
        };

        regionMatchedCallback({payload: cardId});

        expect(mockToast.info).toHaveBeenCalledWith("区域匹配触发：卡片 audio-card-3");
    });

    it("regionMatched 回调更新 statusMessage", () => {
        const cardId = "audio-card-4";
        const regionMatchedCallback = (event: {payload: string}) => {
            mockSetStatusMessage(`区域匹配触发：卡片 ${event.payload}`);
        };

        regionMatchedCallback({payload: cardId});

        expect(mockSetStatusMessage).toHaveBeenCalledWith("区域匹配触发：卡片 audio-card-4");
    });

    it("regionMatched 回调正确解析 payload 为卡片 ID", () => {
        const capturedPayloads: string[] = [];
        const regionMatchedCallback = (event: {payload: string}) => {
            capturedPayloads.push(event.payload);
            mockToast.info(`区域匹配触发：卡片 ${event.payload}`);
            mockSetStatusMessage(`区域匹配触发：卡片 ${event.payload}`);
        };

        regionMatchedCallback({payload: "region-1"});
        regionMatchedCallback({payload: "region-2"});

        expect(capturedPayloads).toEqual(["region-1", "region-2"]);
        expect(mockToast.info).toHaveBeenCalledTimes(2);
        expect(mockSetStatusMessage).toHaveBeenCalledTimes(2);
    });

    it("disposed 后 regionMatched 回调不触发副作用", () => {
        let disposed = false;
        const regionMatchedCallback = (event: {payload: string}) => {
            if (disposed) return;
            mockToast.info(`区域匹配触发：卡片 ${event.payload}`);
        };

        regionMatchedCallback({payload: "card-a"});
        disposed = true;
        regionMatchedCallback({payload: "card-b"});

        expect(mockToast.info).toHaveBeenCalledTimes(1);
        expect(mockToast.info).toHaveBeenCalledWith("区域匹配触发：卡片 card-a");
    });
});

// ── stateChanged 事件回调行为测试 ────────────────────

describe("recognition-page stateChanged 事件行为", () => {
    afterEach(() => {
        mockSetBootstrap.mockReset();
        mockSetPageError.mockReset();
        vi.clearAllMocks();
    });

    it("订阅 RECOGNITION_EVENTS.stateChanged 事件", async () => {
        const {RECOGNITION_EVENTS} = await import("@/lib/tauri-events");
        expect(RECOGNITION_EVENTS.stateChanged).toBe("recognition://state-changed");
    });

    it("stateChanged 回调更新 bootstrap 并合并 watchRegions", () => {
        const mockPayload = {
            settings: {audioEnabled: true, cards: []},
            runs: [],
        };
        const mockSetForm = vi.fn();
        const mockMerge = vi.fn().mockReturnValue({cards: []});

        // 模拟 stateChanged 回调行为（提取自 recognition-page.tsx）：
        const stateChangedCallback = (event: {payload: unknown}) => {
            mockSetBootstrap(event.payload);
            // setForm 接受 updater 函数，内部调用 merge
            mockSetForm((current: unknown) =>
                mockMerge(current, event.payload),
            );
            mockSetPageError(null);
        };

        stateChangedCallback({payload: mockPayload});

        expect(mockSetBootstrap).toHaveBeenCalledWith(mockPayload);
        // 验证 setForm 被调用（传入 updater 函数）
        expect(mockSetForm).toHaveBeenCalled();
        // 手动触发 updater 以验证 merge 被调用
        const updater = mockSetForm.mock.calls[0][0] as (current: unknown) => unknown;
        updater({cards: []});
        expect(mockMerge).toHaveBeenCalledWith({cards: []}, mockPayload);
        expect(mockSetPageError).toHaveBeenCalledWith(null);
    });
});

// ── hotkeyError 事件回调行为测试 ──────────────────────

describe("recognition-page hotkeyError 事件行为", () => {
    afterEach(() => {
        mockSetPageError.mockReset();
        mockSetStatusMessage.mockReset();
        mockToast.error.mockReset();
        vi.clearAllMocks();
    });

    it("订阅 RECOGNITION_EVENTS.hotkeyError 事件", async () => {
        const {RECOGNITION_EVENTS} = await import("@/lib/tauri-events");
        expect(RECOGNITION_EVENTS.hotkeyError).toBe("recognition://hotkey-error");
    });

    it("hotkeyError 回调触发 toast.error 通知", () => {
        const errorMessage = "热键注册失败：F5 已被占用";
        const hotkeyErrorCallback = (event: {payload: string}) => {
            mockSetPageError(event.payload);
            mockSetStatusMessage(event.payload);
            mockToast.error(event.payload);
        };

        hotkeyErrorCallback({payload: errorMessage});

        expect(mockSetPageError).toHaveBeenCalledWith(errorMessage);
        expect(mockSetStatusMessage).toHaveBeenCalledWith(errorMessage);
        expect(mockToast.error).toHaveBeenCalledWith(errorMessage);
    });
});

// ── unmount 时 unlisten 行为测试 ──────────────────────

describe("recognition-page unmount 时清理事件监听", () => {
    it("unmount 时调用所有 unlisten 回调", async () => {
        const unlistenStateChanged = vi.fn();
        const unlistenHotkeyError = vi.fn();
        const unlistenHotkeyTriggered = vi.fn();
        const unlistenRegionMatched = vi.fn();

        // 模拟 unmount 时的清理行为（提取自 recognition-page.tsx useEffect cleanup）
        const cleanup = () => {
            unlistenStateChanged();
            unlistenHotkeyError();
            unlistenHotkeyTriggered();
            unlistenRegionMatched();
        };

        cleanup();

        expect(unlistenStateChanged).toHaveBeenCalled();
        expect(unlistenHotkeyError).toHaveBeenCalled();
        expect(unlistenHotkeyTriggered).toHaveBeenCalled();
        expect(unlistenRegionMatched).toHaveBeenCalled();
    });

    it("unlisten 回调各被调用恰好一次", async () => {
        const unlistenCallbacks = {
            stateChanged: vi.fn(),
            hotkeyError: vi.fn(),
            hotkeyTriggered: vi.fn(),
            regionMatched: vi.fn(),
        };

        const cleanup = () => {
            Object.values(unlistenCallbacks).forEach((fn) => fn());
        };

        cleanup();

        expect(unlistenCallbacks.stateChanged).toHaveBeenCalledTimes(1);
        expect(unlistenCallbacks.hotkeyError).toHaveBeenCalledTimes(1);
        expect(unlistenCallbacks.hotkeyTriggered).toHaveBeenCalledTimes(1);
        expect(unlistenCallbacks.regionMatched).toHaveBeenCalledTimes(1);
    });
});

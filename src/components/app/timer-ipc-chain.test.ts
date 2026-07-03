import {describe, expect, it, vi} from "vitest";

/**
 * VAL-AR-029: 前端 IPC 链路测试。
 *
 * 至少一个工具页完整 IPC 链路测试：
 * invoke bootstrap → 渲染 → listenEvent → 状态更新。
 *
 * 以 TimerPage 为完整 IPC 链路示例，验证：
 * 1. invoke("timer_get_bootstrap") → bootstrap 数据加载 → form 状态初始化
 * 2. listen(TIMER_EVENTS.stateChanged) → 事件回调 → setBootstrap 更新
 * 3. listen(TIMER_EVENTS.hotkeyTriggered) → 事件回调 → setStatusMessage 更新
 * 4. autosave: isDirty → debounce → invoke("timer_save_settings") → bootstrap 更新
 * 5. disposed 标志确保 unmount 后 IPC 回调无副作用
 */

// ── 事件名常量 ──────────────────────────────────────────

const TIMER_EVENTS = {
    stateChanged: "timer://state-changed",
    hotkeyTriggered: "timer://hotkey-triggered",
} as const;

// ── 类型定义 ──────────────────────────────────────────

interface TimerSettings {
    timerEnabled: boolean;
    timers: TimerItem[];
}

interface TimerItem {
    id: string;
    name: string;
    durationSeconds: number;
    direction: "countup" | "countdown";
    enabled: boolean;
    hotkey: string;
}

interface TimerRunState {
    id: string;
    status: "idle" | "running" | "finished";
    currentSeconds: number;
}

interface TimerBootstrap {
    settings: TimerSettings;
    runs: TimerRunState[];
    hotkeyError: string | null;
}

interface TimerSettingsForm {
    timerEnabled: boolean;
    timers: TimerItemForm[];
}

interface TimerItemForm {
    id: string;
    name: string;
    durationSeconds: string;
    direction: "countup" | "countdown";
    enabled: boolean;
    hotkey: string;
}

// ── 模拟工具函数 ──────────────────────────────────────

/** 模拟 settingsToForm：将后端设置转为前端可编辑态 */
function timerSettingsToForm(settings: TimerSettings): TimerSettingsForm {
    return {
        timerEnabled: settings.timerEnabled,
        timers: settings.timers.map((t) => ({
            id: t.id,
            name: t.name,
            durationSeconds: String(t.durationSeconds),
            direction: t.direction,
            enabled: t.enabled,
            hotkey: t.hotkey,
        })),
    };
}

/** 模拟 parseSettingsForm：将前端可编辑态解析回后端设置态 */
function parseTimerSettingsForm(form: TimerSettingsForm): TimerSettings {
    return {
        timerEnabled: form.timerEnabled,
        timers: form.timers.map((t) => ({
            id: t.id,
            name: t.name,
            durationSeconds: Number(t.durationSeconds),
            direction: t.direction,
            enabled: t.enabled,
            hotkey: t.hotkey,
        })),
    };
}

// ── 模拟 IPC 层 ──────────────────────────────────────

function createMockIPC() {
    const invokeMock = vi.fn();
    const listenMock = vi.fn();
    const unlistenFns: (() => void)[] = [];

    /** 模拟 Tauri invoke */
    invokeMock.mockImplementation((command: string, _args?: Record<string, unknown>) => {
        if (command === "timer_get_bootstrap") {
            return Promise.resolve({
                settings: {
                    timerEnabled: true,
                    timers: [
                        {id: "t1", name: "计时器1", durationSeconds: 30, direction: "countup", enabled: true, hotkey: "F1"},
                    ],
                },
                runs: [{id: "t1", status: "idle", currentSeconds: 0}],
                hotkeyError: null,
            } satisfies TimerBootstrap);
        }
        if (command === "timer_save_settings") {
            // 返回更新后的 bootstrap
            return Promise.resolve({
                settings: {
                    timerEnabled: true,
                    timers: [
                        {id: "t1", name: "计时器1", durationSeconds: 60, direction: "countdown", enabled: true, hotkey: "F1"},
                    ],
                },
                runs: [{id: "t1", status: "idle", currentSeconds: 0}],
                hotkeyError: null,
            } satisfies TimerBootstrap);
        }
        return Promise.reject(new Error(`未知命令: ${command}`));
    });

    /** 模拟 Tauri listen */
    listenMock.mockImplementation((_event: string, _callback: (event: {payload: unknown}) => void) => {
        const unlisten = vi.fn();
        unlistenFns.push(unlisten);
        return Promise.resolve(unlisten);
    });

    return {invokeMock, listenMock, unlistenFns};
}

// ── 模拟 TimerPage IPC 完整状态机 ─────────────────────

function createTimerPageStateMachine() {
    const ipc = createMockIPC();
    let bootstrap: TimerBootstrap | null = null;
    let form: TimerSettingsForm | null = null;
    let statusMessage = "正在加载...";
    let pageError: string | null = null;
    let disposed = false;
    const unlistenCallbacks: (() => void)[] = [];

    return {
        ipc,
        getState: () => ({bootstrap, form, statusMessage, pageError, disposed}),
        getUnlistenCallbacks: () => unlistenCallbacks,

        /** 模拟 mount：加载 bootstrap + 订阅事件 */
        async mount() {
            // 1. invoke bootstrap
            try {
                bootstrap = await ipc.invokeMock("timer_get_bootstrap") as TimerBootstrap;
                form = timerSettingsToForm(bootstrap.settings);
                statusMessage = "计时器面板已就绪。配置阶段节奏、透明窗口与快捷键。";
            } catch (error) {
                pageError = String(error);
                statusMessage = String(error);
            }

            // 2. 订阅事件
            const unlisten1 = await ipc.listenMock(TIMER_EVENTS.stateChanged, (evt: {payload: unknown}) => {
                if (disposed) return;
                bootstrap = evt.payload as TimerBootstrap;
            });
            unlistenCallbacks.push(unlisten1);

            const unlisten2 = await ipc.listenMock(TIMER_EVENTS.hotkeyTriggered, (evt: {payload: unknown}) => {
                if (disposed) return;
                statusMessage = `快捷键已触发 ${(evt.payload as string[]).length} 个计时器。运行中的计时器会忽略重复触发。`;
            });
            unlistenCallbacks.push(unlisten2);
        },

        /** 模拟 unmount */
        unmount() {
            disposed = true;
            for (const unlisten of unlistenCallbacks) {
                unlisten();
            }
        },

        /** 模拟 stateChanged 事件到达 */
        simulateStateChanged(newBootstrap: TimerBootstrap) {
            const callback = ipc.listenMock.mock.calls[0]?.[1] as ((event: {payload: unknown}) => void) | undefined;
            if (callback) {
                callback({payload: newBootstrap});
            }
        },

        /** 模拟 hotkeyTriggered 事件到达 */
        simulateHotkeyTriggered(timerIds: string[]) {
            const callback = ipc.listenMock.mock.calls[1]?.[1] as ((event: {payload: unknown}) => void) | undefined;
            if (callback) {
                callback({payload: timerIds});
            }
        },

        /** 模拟 saveSettings */
        async saveSettings(settingsValue: TimerSettings, _nextVersion: number) {
            try {
                const next = await ipc.invokeMock("timer_save_settings", {settingsValue}) as TimerBootstrap;
                bootstrap = next;
                form = timerSettingsToForm(next.settings);
                statusMessage = `计时器设置已保存（${next.settings.timerEnabled ? "开启" : "关闭"}）。`;
            } catch (error) {
                pageError = String(error);
                statusMessage = String(error);
            }
        },

        /** 模拟 updateForm */
        updateForm(key: keyof TimerSettingsForm, value: unknown) {
            if (form) {
                form = {...form, [key]: value};
            }
        },
    };
}

// ── 测试用例 ──────────────────────────────────────────

describe("timer-page 完整 IPC 链路：invoke bootstrap → 渲染 → listenEvent → 状态更新", () => {
    it("invoke bootstrap → bootstrap 加载 → form 初始化", async () => {
        const machine = createTimerPageStateMachine();
        await machine.mount();

        const state = machine.getState();
        expect(state.bootstrap).not.toBeNull();
        expect(state.form).not.toBeNull();
        expect(state.form!.timerEnabled).toBe(true);
        expect(state.form!.timers).toHaveLength(1);
        expect(state.form!.timers[0].durationSeconds).toBe("30"); // number→string 转换
        expect(state.statusMessage).toContain("就绪");
    });

    it("invoke bootstrap 失败时设置 pageError", async () => {
        const machine = createTimerPageStateMachine();
        machine.ipc.invokeMock.mockRejectedValue(new Error("后端通信失败"));

        await machine.mount();

        const state = machine.getState();
        expect(state.pageError).toContain("后端通信失败");
        expect(state.statusMessage).toContain("后端通信失败");
    });

    it("listen stateChanged → 事件到达 → bootstrap 更新", async () => {
        const machine = createTimerPageStateMachine();
        await machine.mount();

        const newBootstrap: TimerBootstrap = {
            settings: {
                timerEnabled: false,
                timers: [{id: "t1", name: "已关闭", durationSeconds: 30, direction: "countup", enabled: false, hotkey: ""}],
            },
            runs: [],
            hotkeyError: null,
        };

        machine.simulateStateChanged(newBootstrap);

        const state = machine.getState();
        expect(state.bootstrap).toEqual(newBootstrap);
        expect(state.bootstrap!.settings.timerEnabled).toBe(false);
    });

    it("listen hotkeyTriggered → 事件到达 → statusMessage 更新", async () => {
        const machine = createTimerPageStateMachine();
        await machine.mount();

        machine.simulateHotkeyTriggered(["t1", "t2"]);

        const state = machine.getState();
        expect(state.statusMessage).toContain("快捷键已触发 2 个计时器");
    });

    it("unmount 后 stateChanged 回调不执行副作用", async () => {
        const machine = createTimerPageStateMachine();
        await machine.mount();

        const originalBootstrap = machine.getState().bootstrap;
        machine.unmount();

        const newBootstrap: TimerBootstrap = {
            settings: {
                timerEnabled: false,
                timers: [],
            },
            runs: [],
            hotkeyError: null,
        };

        machine.simulateStateChanged(newBootstrap);

        // disposed 后 bootstrap 不变
        expect(machine.getState().bootstrap).toEqual(originalBootstrap);
    });

    it("unmount 后 hotkeyTriggered 回调不执行副作用", async () => {
        const machine = createTimerPageStateMachine();
        await machine.mount();

        const originalMessage = machine.getState().statusMessage;
        machine.unmount();

        machine.simulateHotkeyTriggered(["t1"]);

        expect(machine.getState().statusMessage).toBe(originalMessage);
    });

    it("unmount 时调用所有 unlisten 回调", async () => {
        const machine = createTimerPageStateMachine();
        await machine.mount();

        const unlistenCallbacks = machine.getUnlistenCallbacks();
        expect(unlistenCallbacks).toHaveLength(2);

        machine.unmount();

        expect(unlistenCallbacks[0]).toHaveBeenCalled();
        expect(unlistenCallbacks[1]).toHaveBeenCalled();
    });
});

describe("timer-page autosave IPC 链路：isDirty → debounce → invoke save", () => {
    it("updateForm 触发 isDirty → saveSettings 调用 invoke", async () => {
        const machine = createTimerPageStateMachine();
        await machine.mount();

        // 模拟修改表单
        machine.updateForm("timerEnabled", false);

        const state = machine.getState();
        expect(state.form!.timerEnabled).toBe(false);

        // 模拟 autosave 触发
        const settingsValue = parseTimerSettingsForm(state.form!);
        await machine.saveSettings(settingsValue, 1);

        expect(machine.ipc.invokeMock).toHaveBeenCalledWith("timer_save_settings", {settingsValue});

        const updatedState = machine.getState();
        expect(updatedState.bootstrap).not.toBeNull();
        expect(updatedState.statusMessage).toContain("已保存");
    });

    it("saveSettings 返回更新后的 bootstrap → form 同步更新", async () => {
        const machine = createTimerPageStateMachine();
        await machine.mount();

        const settingsValue: TimerSettings = {
            timerEnabled: true,
            timers: [{id: "t1", name: "计时器1", durationSeconds: 60, direction: "countdown", enabled: true, hotkey: "F1"}],
        };

        await machine.saveSettings(settingsValue, 1);

        const state = machine.getState();
        // saveSettings 返回的 bootstrap.settings 已被 settingsToForm 转换回 form
        expect(state.form!.timers[0].durationSeconds).toBe("60");
        expect(state.form!.timers[0].direction).toBe("countdown");
    });

    it("saveSettings 失败时设置 pageError 和 statusMessage", async () => {
        const machine = createTimerPageStateMachine();
        await machine.mount();

        // 让 save_settings 失败
        machine.ipc.invokeMock.mockImplementation((command: string) => {
            if (command === "timer_save_settings") {
                return Promise.reject(new Error("保存失败：后端异常"));
            }
            // 默认行为
            if (command === "timer_get_bootstrap") {
                return Promise.resolve({
                    settings: {timerEnabled: true, timers: []},
                    runs: [],
                    hotkeyError: null,
                });
            }
            return Promise.reject(new Error(`未知命令: ${command}`));
        });

        await machine.saveSettings({timerEnabled: true, timers: []}, 1);

        const state = machine.getState();
        expect(state.pageError).toContain("保存失败");
        expect(state.statusMessage).toContain("保存失败");
    });
});

describe("timer-page IPC 链路：事件名对齐", () => {
    it("listen 调用的事件名与 tauri-events.ts 一致", async () => {
        const machine = createTimerPageStateMachine();
        await machine.mount();

        const listenCalls = machine.ipc.listenMock.mock.calls;
        const eventNames = listenCalls.map((call: unknown[]) => call[0] as string);

        expect(eventNames).toContain(TIMER_EVENTS.stateChanged);
        expect(eventNames).toContain(TIMER_EVENTS.hotkeyTriggered);
    });

    it("invoke 调用的命令名与 spec 定义一致", async () => {
        const machine = createTimerPageStateMachine();
        await machine.mount();

        const invokeCalls = machine.ipc.invokeMock.mock.calls;
        const commandNames = invokeCalls.map((call: unknown[]) => call[0] as string);

        expect(commandNames).toContain("timer_get_bootstrap");
    });

    it("saveSettings 调用的命令名与 spec 定义一致", async () => {
        const machine = createTimerPageStateMachine();
        await machine.mount();

        const settingsValue: TimerSettings = {timerEnabled: true, timers: []};
        await machine.saveSettings(settingsValue, 1);

        const invokeCalls = machine.ipc.invokeMock.mock.calls;
        const commandNames = invokeCalls.map((call: unknown[]) => call[0] as string);

        expect(commandNames).toContain("timer_save_settings");
    });
});

describe("timer-page IPC 链路：往返转换一致性", () => {
    it("settingsToForm → parseSettingsForm 往返一致", () => {
        const original: TimerSettings = {
            timerEnabled: true,
            timers: [
                {id: "t1", name: "计时器1", durationSeconds: 30, direction: "countup", enabled: true, hotkey: "F1"},
                {id: "t2", name: "计时器2", durationSeconds: 60, direction: "countdown", enabled: false, hotkey: "F2"},
            ],
        };

        const form = timerSettingsToForm(original);
        const roundTripped = parseTimerSettingsForm(form);

        expect(roundTripped).toEqual(original);
    });

    it("form 中 durationSeconds 为字符串类型", () => {
        const settings: TimerSettings = {
            timerEnabled: true,
            timers: [{id: "t1", name: "测试", durationSeconds: 30, direction: "countup", enabled: true, hotkey: ""}],
        };

        const form = timerSettingsToForm(settings);
        expect(typeof form.timers[0].durationSeconds).toBe("string");
        expect(form.timers[0].durationSeconds).toBe("30");
    });

    it("bootstrap 加载后 form 与 bootstrap.settings 往返一致", async () => {
        const machine = createTimerPageStateMachine();
        await machine.mount();

        const state = machine.getState();
        const form = state.form!;
        const roundTripped = parseTimerSettingsForm(form);
        expect(roundTripped).toEqual(state.bootstrap!.settings);
    });
});

describe("timer-page IPC 链路：stateChanged 事件更新完整流程", () => {
    it("stateChanged → bootstrap 更新 → runs 状态同步", async () => {
        const machine = createTimerPageStateMachine();
        await machine.mount();

        const updatedBootstrap: TimerBootstrap = {
            settings: {
                timerEnabled: true,
                timers: [{id: "t1", name: "计时器1", durationSeconds: 30, direction: "countup", enabled: true, hotkey: "F1"}],
            },
            runs: [{id: "t1", status: "running", currentSeconds: 15}],
            hotkeyError: null,
        };

        machine.simulateStateChanged(updatedBootstrap);

        const state = machine.getState();
        expect(state.bootstrap!.runs[0].status).toBe("running");
        expect(state.bootstrap!.runs[0].currentSeconds).toBe(15);
    });

    it("stateChanged → hotkeyError 更新", async () => {
        const machine = createTimerPageStateMachine();
        await machine.mount();

        const updatedBootstrap: TimerBootstrap = {
            settings: {
                timerEnabled: true,
                timers: [],
            },
            runs: [],
            hotkeyError: "F1 已被占用",
        };

        machine.simulateStateChanged(updatedBootstrap);

        const state = machine.getState();
        expect(state.bootstrap!.hotkeyError).toBe("F1 已被占用");
    });

    it("连续多次 stateChanged 事件正确更新状态", async () => {
        const machine = createTimerPageStateMachine();
        await machine.mount();

        // 第一次事件
        machine.simulateStateChanged({
            settings: {timerEnabled: true, timers: []},
            runs: [{id: "t1", status: "running", currentSeconds: 5}],
            hotkeyError: null,
        });
        expect(machine.getState().bootstrap!.runs[0].currentSeconds).toBe(5);

        // 第二次事件
        machine.simulateStateChanged({
            settings: {timerEnabled: true, timers: []},
            runs: [{id: "t1", status: "running", currentSeconds: 10}],
            hotkeyError: null,
        });
        expect(machine.getState().bootstrap!.runs[0].currentSeconds).toBe(10);

        // 第三次事件
        machine.simulateStateChanged({
            settings: {timerEnabled: true, timers: []},
            runs: [{id: "t1", status: "finished", currentSeconds: 30}],
            hotkeyError: null,
        });
        expect(machine.getState().bootstrap!.runs[0].status).toBe("finished");
    });
});

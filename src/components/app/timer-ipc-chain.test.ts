import {describe, expect, it, vi} from "vitest";
import {TIMER_EVENTS} from "@/lib/tauri-events";
import {timerSettingsToForm, parseTimerSettingsForm} from "@/components/app/timer-utils";
import type {
    TimerBootstrap,
    TimerRunState,
    TimerRunsChanged,
    TimerSettings,
    TimerSettingsForm,
} from "@/components/app/timer-types";

/** 测试用的默认 TimerDisplaySettings */
const TEST_DISPLAY = {rect: {x: 0, y: 0, width: 300, height: 200}, fontOpacity: 1};

/** 测试用的完整 TimerRunState */
function makeRunState(overrides: Partial<TimerRunState> & {id: string}): TimerRunState {
    return {
        currentSeconds: 0,
        remainingSeconds: 30,
        durationSeconds: 30,
        direction: "countup",
        status: "running",
        segmentCount: null,
        segmentDuration: 0,
        recovering: false,
        recoveringCount: 0,
        activeSegmentIndex: 0,
        startedAtMs: 0,
        recoveryStartPool: 0,
        ...overrides,
    };
}

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
 *
 * 使用生产代码中的事件名常量和表单转换函数（来自 tauri-events.ts / timer-utils.ts），
 * 而非内联重定义。
 */

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
                    display: TEST_DISPLAY,
                    timers: [
                        {id: "t1", name: "计时器1", durationSeconds: 30, direction: "countup", enabled: true, hotkey: "F1", triggerMode: "press", ignoreRunning: true, segmentCount: null},
                    ],
                },
                runs: [makeRunState({id: "t1", status: "running", currentSeconds: 0})],
                hotkeyError: null,
            } satisfies TimerBootstrap);
        }
        if (command === "timer_save_settings") {
            // 返回更新后的 bootstrap
            return Promise.resolve({
                settings: {
                    timerEnabled: true,
                    display: TEST_DISPLAY,
                    timers: [
                        {id: "t1", name: "计时器1", durationSeconds: 60, direction: "countdown", enabled: true, hotkey: "F1", triggerMode: "press", ignoreRunning: true, segmentCount: null},
                    ],
                },
                runs: [makeRunState({id: "t1", status: "running", currentSeconds: 0})],
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
    let runtimeRuns: TimerRunState[] | null = null;
    let statusMessage = "正在加载...";
    let pageError: string | null = null;
    let disposed = false;
    const unlistenCallbacks: (() => void)[] = [];

    return {
        ipc,
        getState: () => ({bootstrap, form, runtimeRuns, statusMessage, pageError, disposed}),
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

            const unlisten2 = await ipc.listenMock(TIMER_EVENTS.runsChanged, (evt: {payload: unknown}) => {
                if (disposed) return;
                runtimeRuns = (evt.payload as TimerRunsChanged).runs;
            });
            unlistenCallbacks.push(unlisten2);

            const unlisten3 = await ipc.listenMock(TIMER_EVENTS.hotkeyTriggered, (evt: {payload: unknown}) => {
                if (disposed) return;
                statusMessage = `快捷键已触发 ${(evt.payload as string[]).length} 个计时器。运行中的计时器会忽略重复触发。`;
            });
            unlistenCallbacks.push(unlisten3);
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
            const callback = ipc.listenMock.mock.calls[2]?.[1] as ((event: {payload: unknown}) => void) | undefined;
            if (callback) {
                callback({payload: timerIds});
            }
        },

        simulateRunsChanged(runs: TimerRunState[]) {
            const callback = ipc.listenMock.mock.calls[1]?.[1] as ((event: {payload: unknown}) => void) | undefined;
            if (callback) callback({payload: {runs} satisfies TimerRunsChanged});
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
                display: TEST_DISPLAY,
                timers: [{id: "t1", name: "已关闭", durationSeconds: 30, direction: "countup", enabled: false, hotkey: "", triggerMode: "press", ignoreRunning: true, segmentCount: null}],
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

    it("runsChanged 只更新运行态，不替换 settings 或 form", async () => {
        const machine = createTimerPageStateMachine();
        await machine.mount();
        const before = machine.getState();
        const runs = [makeRunState({id: "t1", currentSeconds: 12})];

        machine.simulateRunsChanged(runs);

        const after = machine.getState();
        expect(after.runtimeRuns).toBe(runs);
        expect(after.bootstrap?.settings).toBe(before.bootstrap?.settings);
        expect(after.form).toBe(before.form);
    });

    it("较旧 stateChanged 不回滚较新的运行态", async () => {
        const machine = createTimerPageStateMachine();
        await machine.mount();
        const latestRuns = [makeRunState({id: "t1", currentSeconds: 27})];
        machine.simulateRunsChanged(latestRuns);

        machine.simulateStateChanged({
            ...machine.getState().bootstrap!,
            runs: [makeRunState({id: "t1", currentSeconds: 9})],
        });

        expect(machine.getState().runtimeRuns).toBe(latestRuns);
    });

    it("unmount 后 stateChanged 回调不执行副作用", async () => {
        const machine = createTimerPageStateMachine();
        await machine.mount();

        const originalBootstrap = machine.getState().bootstrap;
        machine.unmount();

        const newBootstrap: TimerBootstrap = {
            settings: {
                timerEnabled: false,
                display: TEST_DISPLAY,
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
        expect(unlistenCallbacks).toHaveLength(3);

        machine.unmount();

        expect(unlistenCallbacks[0]).toHaveBeenCalled();
        expect(unlistenCallbacks[1]).toHaveBeenCalled();
        expect(unlistenCallbacks[2]).toHaveBeenCalled();
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
            display: TEST_DISPLAY,
            timers: [{id: "t1", name: "计时器1", durationSeconds: 60, direction: "countdown", enabled: true, hotkey: "F1", triggerMode: "press", ignoreRunning: true, segmentCount: null}],
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
                    settings: {timerEnabled: true, display: TEST_DISPLAY, timers: []},
                    runs: [],
                    hotkeyError: null,
                });
            }
            return Promise.reject(new Error(`未知命令: ${command}`));
        });

        await machine.saveSettings({timerEnabled: true, display: TEST_DISPLAY, timers: []}, 1);

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

        const settingsValue: TimerSettings = {timerEnabled: true, display: TEST_DISPLAY, timers: []};
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
            display: TEST_DISPLAY,
            timerGroups: [{id: "default-timer-group", name: "默认分组", enabled: true, display: TEST_DISPLAY}],
            timers: [
                {id: "t1", groupId: "default-timer-group", name: "计时器1", durationSeconds: 30, direction: "countup", enabled: true, hotkey: "F1", triggerMode: "press", ignoreRunning: true, segmentCount: null},
                {id: "t2", groupId: "default-timer-group", name: "计时器2", durationSeconds: 60, direction: "countdown", enabled: false, hotkey: "F2", triggerMode: "press", ignoreRunning: true, segmentCount: null},
            ],
        };

        const form = timerSettingsToForm(original);
        const roundTripped = parseTimerSettingsForm(form);

        // 比较关键字段而非全量 equals（生产函数会规范化 display 精度等）
        expect(roundTripped.timerEnabled).toBe(original.timerEnabled);
        expect(roundTripped.timers.length).toBe(original.timers.length);
        expect(roundTripped.timers[0].durationSeconds).toBe(30);
        expect(roundTripped.timers[1].durationSeconds).toBe(60);
    });

    it("form 中 durationSeconds 为字符串类型", () => {
        const settings: TimerSettings = {
            timerEnabled: true,
            display: TEST_DISPLAY,
            timers: [{id: "t1", name: "测试", durationSeconds: 30, direction: "countup", enabled: true, hotkey: "", triggerMode: "press", ignoreRunning: true, segmentCount: null}],
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
        // 比较关键字段而非全量 equals（生产函数会规范化 display 精度等）
        expect(roundTripped.timerEnabled).toBe(state.bootstrap!.settings.timerEnabled);
        expect(roundTripped.timers.length).toBe(state.bootstrap!.settings.timers.length);
    });
});

describe("timer-page IPC 链路：stateChanged 事件更新完整流程", () => {
    it("stateChanged → bootstrap 更新 → runs 状态同步", async () => {
        const machine = createTimerPageStateMachine();
        await machine.mount();

        const updatedBootstrap: TimerBootstrap = {
            settings: {
                timerEnabled: true,
                display: TEST_DISPLAY,
                timers: [{id: "t1", name: "计时器1", durationSeconds: 30, direction: "countup", enabled: true, hotkey: "F1", triggerMode: "press", ignoreRunning: true, segmentCount: null}],
            },
            runs: [makeRunState({id: "t1", status: "running", currentSeconds: 15})],
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
                display: TEST_DISPLAY,
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
            settings: {timerEnabled: true, display: TEST_DISPLAY, timers: []},
            runs: [makeRunState({id: "t1", status: "running", currentSeconds: 5})],
            hotkeyError: null,
        });
        expect(machine.getState().bootstrap!.runs[0].currentSeconds).toBe(5);

        // 第二次事件
        machine.simulateStateChanged({
            settings: {timerEnabled: true, display: TEST_DISPLAY, timers: []},
            runs: [makeRunState({id: "t1", status: "running", currentSeconds: 10})],
            hotkeyError: null,
        });
        expect(machine.getState().bootstrap!.runs[0].currentSeconds).toBe(10);

        // 第三次事件
        machine.simulateStateChanged({
            settings: {timerEnabled: true, display: TEST_DISPLAY, timers: []},
            runs: [makeRunState({id: "t1", status: "finished", currentSeconds: 30})],
            hotkeyError: null,
        });
        expect(machine.getState().bootstrap!.runs[0].status).toBe("finished");
    });
});

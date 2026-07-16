import { describe, it, expect, vi, beforeEach } from "vitest";

// Mock @tauri-apps/api/core
const mockInvoke = vi.fn().mockResolvedValue("abc123");
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

// 在 mock 建立后再 import
import {
  generateTraceId,
  setTraceId,
  clearTraceId,
  currentTraceId,
  logFrontend,
  getSessionId,
  initLogging,
  getLogSettings,
  setLogSettings,
  invokeLogged,
} from "./logging";

describe("logging", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    mockInvoke.mockResolvedValue("abc123");
    clearTraceId();
  });

  describe("generateTraceId", () => {
    it("生成 4 位 hex 字符串", () => {
      const id = generateTraceId();
      expect(id).toHaveLength(4);
      expect(id).toMatch(/^[0-9a-f]{4}$/);
    });

    it("多次调用生成不同 ID（极高概率）", () => {
      const ids = new Set(Array.from({ length: 100 }, () => generateTraceId()));
      // 100 个随机 4 位 hex 不应全相同
      expect(ids.size).toBeGreaterThan(1);
    });
  });

  describe("setTraceId / clearTraceId / currentTraceId", () => {
    it("设置后可获取", () => {
      setTraceId("abcd");
      expect(currentTraceId()).toBe("abcd");
    });

    it("清除后恢复为 --", () => {
      setTraceId("abcd");
      clearTraceId();
      expect(currentTraceId()).toBe("--");
    });

    it("初始状态为 --", () => {
      expect(currentTraceId()).toBe("--");
    });
  });

  describe("logFrontend", () => {
    it("在 native shell 下调用 invoke", async () => {
      // 模拟 __TAURI_INTERNALS__ 存在
      const origWindow = globalThis.window;
      Object.defineProperty(globalThis, "window", {
        value: { __TAURI_INTERNALS__: {} },
        writable: true,
        configurable: true,
      });

      mockInvoke.mockResolvedValue(undefined);
      await logFrontend("info", "test-source", "TestLoc:1", "测试消息", { key: "val" });

      expect(mockInvoke).toHaveBeenCalledWith("log_write_frontend", {
        request: {
          level: "info",
          source: "test-source",
          location: "TestLoc:1",
          traceId: "--",
          message: "测试消息",
          payload: { key: "val" },
        },
      });

      // 恢复
      Object.defineProperty(globalThis, "window", {
        value: origWindow,
        writable: true,
        configurable: true,
      });
    });

    it("在非 native shell 下不调用 invoke", async () => {
      // 移除 __TAURI_INTERNALS__
      const origWindow = globalThis.window;
      Object.defineProperty(globalThis, "window", {
        value: {},
        writable: true,
        configurable: true,
      });

      mockInvoke.mockClear();
      await logFrontend("error", "test", "T:1", "msg");
      expect(mockInvoke).not.toHaveBeenCalled();

      Object.defineProperty(globalThis, "window", {
        value: origWindow,
        writable: true,
        configurable: true,
      });
    });

    it("logFrontend 传入当前 traceId", async () => {
      const origWindow = globalThis.window;
      Object.defineProperty(globalThis, "window", {
        value: { __TAURI_INTERNALS__: {} },
        writable: true,
        configurable: true,
      });

      setTraceId("f0e1");
      mockInvoke.mockResolvedValue(undefined);
      await logFrontend("warn", "src", "Loc:1", "msg");

      expect(mockInvoke).toHaveBeenCalledWith("log_write_frontend", {
        request: expect.objectContaining({ traceId: "f0e1" }),
      });

      clearTraceId();
      Object.defineProperty(globalThis, "window", {
        value: origWindow,
        writable: true,
        configurable: true,
      });
    });
  });

  describe("invokeLogged", () => {
    it("记录变更类 command 的开始和完成", async () => {
      const origWindow = globalThis.window;
      Object.defineProperty(globalThis, "window", {
        value: { __TAURI_INTERNALS__: {} },
        writable: true,
        configurable: true,
      });

      mockInvoke.mockImplementation((command: string) => {
        if (command === "timer_save_settings") return Promise.resolve({ ok: true });
        if (command === "log_write_frontend") return Promise.resolve(undefined);
        return Promise.resolve("abc123");
      });

      const result = await invokeLogged("timer_save_settings", { settingsValue: { token: "secret" } });

      expect(result).toEqual({ ok: true });
      expect(mockInvoke).toHaveBeenCalledWith("timer_save_settings", {
        settingsValue: { token: "secret" },
      });
      const logCalls = mockInvoke.mock.calls.filter(([command]) => command === "log_write_frontend");
      expect(logCalls).toHaveLength(2);
      expect(logCalls[0][1].request.level).toBe("info");
      expect(logCalls[0][1].request.source).toBe("timer-command");
      expect(logCalls[0][1].request.payload.args.settingsValue.token).toBe("[masked]");
      expect(logCalls[1][1].request.payload.phase).toBe("success");

      Object.defineProperty(globalThis, "window", {
        value: origWindow,
        writable: true,
        configurable: true,
      });
    });

    it("logs commands in __TAURI__ fallback shell", async () => {
      const origWindow = globalThis.window;
      Object.defineProperty(globalThis, "window", {
        value: { __TAURI__: {} },
        writable: true,
        configurable: true,
      });

      mockInvoke.mockImplementation((command: string) => {
        if (command === "theme_save_settings") return Promise.resolve({ ok: true });
        if (command === "log_write_frontend") return Promise.resolve(undefined);
        return Promise.resolve("abc123");
      });

      await invokeLogged("theme_save_settings", { settingsValue: { activeThemeId: "valentine" } });

      const logCalls = mockInvoke.mock.calls.filter(([command]) => command === "log_write_frontend");
      expect(logCalls).toHaveLength(2);

      Object.defineProperty(globalThis, "window", {
        value: origWindow,
        writable: true,
        configurable: true,
      });
    });

    it("log=false 时执行 command 但不写开始和完成日志", async () => {
      const origWindow = globalThis.window;
      Object.defineProperty(globalThis, "window", {
        value: { __TAURI_INTERNALS__: {} },
        writable: true,
        configurable: true,
      });

      mockInvoke.mockImplementation((command: string) => {
        if (command === "timer_position_moved") return Promise.resolve(undefined);
        if (command === "log_write_frontend") return Promise.resolve(undefined);
        return Promise.resolve("abc123");
      });

      await invokeLogged("timer_position_moved", {x: 12, y: 34}, {log: false});

      expect(mockInvoke).toHaveBeenCalledWith("timer_position_moved", {x: 12, y: 34});
      expect(mockInvoke.mock.calls.filter(([command]) => command === "log_write_frontend")).toHaveLength(0);

      Object.defineProperty(globalThis, "window", {
        value: origWindow,
        writable: true,
        configurable: true,
      });
    });

    it("command 失败时记录 error 并透传原错误", async () => {
      const origWindow = globalThis.window;
      Object.defineProperty(globalThis, "window", {
        value: { __TAURI_INTERNALS__: {} },
        writable: true,
        configurable: true,
      });

      mockInvoke.mockImplementation((command: string) => {
        if (command === "counter_adjust") return Promise.reject("计数器不存在");
        if (command === "log_write_frontend") return Promise.resolve(undefined);
        return Promise.resolve("abc123");
      });

      await expect(invokeLogged("counter_adjust", { counterId: "missing", delta: 1 })).rejects.toBe(
        "计数器不存在",
      );
      const errorLog = mockInvoke.mock.calls.find(
        ([command, payload]) => command === "log_write_frontend" && payload.request.level === "error",
      );
      expect(errorLog?.[1].request.payload.error.message).toBe("计数器不存在");

      Object.defineProperty(globalThis, "window", {
        value: origWindow,
        writable: true,
        configurable: true,
      });
    });
  });

  describe("initLogging", () => {
    it("在 native shell 下获取 session_id", async () => {
      const origWindow = globalThis.window;
      Object.defineProperty(globalThis, "window", {
        value: { __TAURI_INTERNALS__: {} },
        writable: true,
        configurable: true,
      });

      mockInvoke.mockResolvedValue("sess99");
      await initLogging();
      expect(getSessionId()).toBe("sess99");

      Object.defineProperty(globalThis, "window", {
        value: origWindow,
        writable: true,
        configurable: true,
      });
    });
  });
  describe("getLogSettings", () => {
    it("在 native shell 下调用 invoke 并返回设置", async () => {
      const origWindow = globalThis.window;
      Object.defineProperty(globalThis, "window", {
        value: { __TAURI_INTERNALS__: {} },
        writable: true,
        configurable: true,
      });

      const settings = { globalLevel: "debug" as const, moduleLevels: {} };
      mockInvoke.mockResolvedValue(settings);
      const result = await getLogSettings();

      expect(mockInvoke).toHaveBeenCalledWith("log_get_level");
      expect(result).toEqual(settings);

      Object.defineProperty(globalThis, "window", {
        value: origWindow,
        writable: true,
        configurable: true,
      });
    });

    it("在非 native shell 下不调用 invoke 并返回 null", async () => {
      const origWindow = globalThis.window;
      Object.defineProperty(globalThis, "window", {
        value: {},
        writable: true,
        configurable: true,
      });

      mockInvoke.mockClear();
      const result = await getLogSettings();
      expect(mockInvoke).not.toHaveBeenCalled();
      expect(result).toBeNull();

      Object.defineProperty(globalThis, "window", {
        value: origWindow,
        writable: true,
        configurable: true,
      });
    });
  });

  describe("setLogSettings", () => {
    it("在 native shell 下调用 invoke 传入完整 settings", async () => {
      const origWindow = globalThis.window;
      Object.defineProperty(globalThis, "window", {
        value: { __TAURI_INTERNALS__: {} },
        writable: true,
        configurable: true,
      });

      const settings = { globalLevel: "warn" as const, moduleLevels: { foo: "debug" as const } };
      mockInvoke.mockResolvedValue(undefined);
      await setLogSettings(settings);

      expect(mockInvoke).toHaveBeenCalledWith("log_set_level", { settings });

      Object.defineProperty(globalThis, "window", {
        value: origWindow,
        writable: true,
        configurable: true,
      });
    });

    it("在非 native shell 下不调用 invoke", async () => {
      const origWindow = globalThis.window;
      Object.defineProperty(globalThis, "window", {
        value: {},
        writable: true,
        configurable: true,
      });

      mockInvoke.mockClear();
      await setLogSettings({ globalLevel: "info", moduleLevels: {} });
      expect(mockInvoke).not.toHaveBeenCalled();

      Object.defineProperty(globalThis, "window", {
        value: origWindow,
        writable: true,
        configurable: true,
      });
    });
  });
});

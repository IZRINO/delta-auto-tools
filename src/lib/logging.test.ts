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

/**
 * 前端日志接口
 *
 * 通过 Tauri Command 将前端日志写入后端统一日志文件。
 * 格式与 Rust 后端日志完全一致。
 */

import { invoke as rawInvoke } from "@tauri-apps/api/core";

export type FrontendLogLevel = "error" | "warn" | "info" | "debug" | "trace";

type InvokeArgs = Record<string, unknown>;

interface InvokeLogOptions {
  source?: string;
  location?: string;
  payload?: Record<string, unknown>;
}

let _sessionId: string | null = null;
let _traceId: string = "--";

/**
 * 初始化：从 Rust 获取 session_id
 */
export async function initLogging(): Promise<void> {
  if (!checkNativeShell()) return;
  try {
    _sessionId = await rawInvoke<string>("log_get_session_id");
  } catch {
    _sessionId = null;
  }
}

/**
 * 获取当前 session_id
 */
export function getSessionId(): string | null {
  return _sessionId;
}

/**
 * 生成 4 位 hex trace_id
 */
export function generateTraceId(): string {
  const arr = new Uint8Array(2);
  crypto.getRandomValues(arr);
  return Array.from(arr, (b) => b.toString(16).padStart(2, "0")).join("");
}

/**
 * 设置当前追踪 ID（在操作入口调用）
 */
export function setTraceId(id: string): void {
  _traceId = id;
}

/**
 * 清除当前追踪 ID
 */
export function clearTraceId(): void {
  _traceId = "--";
}

/**
 * 获取当前追踪 ID
 */
export function currentTraceId(): string {
  return _traceId;
}

/**
 * 写入日志到 Rust 后端
 */
export async function logFrontend(
  level: FrontendLogLevel,
  source: string,
  location: string,
  message: string,
  payload?: Record<string, unknown>,
): Promise<void> {
  if (!checkNativeShell()) return;

  try {
    await rawInvoke("log_write_frontend", {
      request: {
        level,
        source,
        location,
        traceId: _traceId,
        message,
        payload: payload ?? null,
      },
    });
  } catch {
    // 日志写入失败不应阻塞业务，静默忽略
  }
}

/**
 * 便捷日志对象
 */
export const log = {
  error: (source: string, location: string, message: string, payload?: Record<string, unknown>) =>
    logFrontend("error", source, location, message, payload),
  warn: (source: string, location: string, message: string, payload?: Record<string, unknown>) =>
    logFrontend("warn", source, location, message, payload),
  info: (source: string, location: string, message: string, payload?: Record<string, unknown>) =>
    logFrontend("info", source, location, message, payload),
  debug: (source: string, location: string, message: string, payload?: Record<string, unknown>) =>
    logFrontend("debug", source, location, message, payload),
  trace: (source: string, location: string, message: string, payload?: Record<string, unknown>) =>
    logFrontend("trace", source, location, message, payload),
};

export async function invokeLogged<T>(
  command: string,
  args?: InvokeArgs,
  options: InvokeLogOptions = {},
): Promise<T> {
  const source = options.source ?? `${command.split("_")[0] || "app"}-command`;
  const location = options.location ?? command;
  const category = classifyCommand(command);
  const startMs = nowMs();
  const basePayload = {
    command,
    category,
    args: toLogValue(args ?? null),
    ...(options.payload ?? {}),
  };

  if (checkNativeShell()) {
    void logFrontend(
      category === "query" ? "debug" : "info",
      source,
      location,
      category === "query" ? "读取 Tauri command" : "调用 Tauri command",
      {...basePayload, phase: "start"},
    );
  }

  try {
    const result = await rawInvoke<T>(command, args);
    if (checkNativeShell()) {
      void logFrontend(
        category === "query" ? "debug" : "info",
        source,
        location,
        "Tauri command 完成",
        {...basePayload, phase: "success", durationMs: elapsedMs(startMs)},
      );
    }
    return result;
  } catch (error) {
    if (checkNativeShell()) {
      void logFrontend("error", source, location, "Tauri command 失败", {
        ...basePayload,
        phase: "error",
        durationMs: elapsedMs(startMs),
        error: serializeError(error),
      });
    }
    throw error;
  }
}

function checkNativeShell(): boolean {
  if (typeof window === "undefined") return false;
  const w = window as Window & { __TAURI_INTERNALS__?: unknown };
  return Boolean(w.__TAURI_INTERNALS__);
}
/**
 * 日志设置 DTO（与 Rust LogSettings 对应）
 */
export interface LogSettings {
  globalLevel: FrontendLogLevel;
  moduleLevels: Record<string, FrontendLogLevel>;
}

/**
 * 获取当前日志级别设置
 */
export async function getLogSettings(): Promise<LogSettings | null> {
  if (!checkNativeShell()) return null;
  try {
    return await rawInvoke<LogSettings>("log_get_level");
  } catch {
    return null;
  }
}

/**
 * 设置日志级别
 */
export async function setLogSettings(settings: LogSettings): Promise<void> {
  if (!checkNativeShell()) return;
  try {
    await rawInvoke("log_set_level", { settings });
  } catch {
    // 静默忽略
  }
}

function classifyCommand(command: string): "query" | "mutation" {
  if (/_(get|read)_/.test(command)) {
    return "query";
  }
  return "mutation";
}

function nowMs(): number {
  return typeof performance === "undefined" ? Date.now() : performance.now();
}

function elapsedMs(startMs: number): number {
  return Math.round(nowMs() - startMs);
}

function serializeError(error: unknown): Record<string, unknown> {
  if (error instanceof Error) {
    return {
      name: error.name,
      message: error.message,
      stack: error.stack,
    };
  }
  return {message: String(error ?? "未知错误")};
}

function toLogValue(value: unknown, depth = 0): unknown {
  if (value === null || value === undefined) return value ?? null;
  if (typeof value === "string") return value.length > 512 ? `${value.slice(0, 512)}...` : value;
  if (typeof value === "number" || typeof value === "boolean") return value;
  if (depth >= 3) return "[truncated]";

  if (Array.isArray(value)) {
    return value.slice(0, 20).map((item) => toLogValue(item, depth + 1));
  }

  if (typeof value === "object") {
    const out: Record<string, unknown> = {};
    for (const [key, item] of Object.entries(value).slice(0, 30)) {
      out[key] = shouldMaskKey(key) ? "[masked]" : toLogValue(item, depth + 1);
    }
    return out;
  }

  return String(value);
}

function shouldMaskKey(key: string): boolean {
  return /token|ticket|cookie|secret|password|authorization/i.test(key);
}

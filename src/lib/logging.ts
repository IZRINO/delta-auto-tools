/**
 * 前端日志接口
 *
 * 通过 Tauri Command 将前端日志写入后端统一日志文件。
 * 格式与 Rust 后端日志完全一致。
 */

import { invoke } from "@tauri-apps/api/core";

export type FrontendLogLevel = "error" | "warn" | "info" | "debug" | "trace";

let _sessionId: string | null = null;
let _traceId: string = "--";

/**
 * 初始化：从 Rust 获取 session_id
 */
export async function initLogging(): Promise<void> {
  if (!checkNativeShell()) return;
  try {
    _sessionId = await invoke<string>("log_get_session_id");
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
    await invoke("log_write_frontend", {
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
    return await invoke<LogSettings>("log_get_level");
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
    await invoke("log_set_level", { settings });
  } catch {
    // 静默忽略
  }
}

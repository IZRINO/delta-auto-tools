import { useEffect, useState } from "react";

function checkNativeShell(): boolean {
  if (typeof window === "undefined") {
    return false;
  }
  const w = window as Window & {
    __TAURI_INTERNALS__?: unknown;
    __TAURI__?: unknown;
  };
  // Tauri v2 使用 __TAURI_INTERNALS__；保留 __TAURI__ 作为兼容 fallback。
  return Boolean(w.__TAURI_INTERNALS__ ?? w.__TAURI__);
}

export function useNativeShell(): boolean {
  const [isNative, setIsNative] = useState(() => checkNativeShell());

  useEffect(() => {
    if (isNative) {
      return;
    }

    // 首次渲染时 Tauri 全局对象可能尚未注入，这里做短暂轮询兜底，
    // 避免桌面端被误判为浏览器预览而导致所有控件被禁用。
    let attempts = 0;
    const maxAttempts = 10;
    const timer = window.setInterval(() => {
      attempts++;
      if (checkNativeShell() || attempts >= maxAttempts) {
        setIsNative(checkNativeShell());
        window.clearInterval(timer);
      }
    }, 50);

    return () => {
      window.clearInterval(timer);
    };
  }, [isNative]);

  return isNative;
}

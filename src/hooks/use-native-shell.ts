import { useMemo } from "react";

export function useNativeShell(): boolean {
  return useMemo(() => {
    const tauriWindow = window as Window & { __TAURI_INTERNALS__?: unknown };
    return Boolean(tauriWindow.__TAURI_INTERNALS__);
  }, []);
}

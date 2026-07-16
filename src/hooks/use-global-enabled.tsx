import {createContext, type ReactNode, useCallback, useContext, useEffect, useState,} from "react";
import {invokeLogged as invoke} from "@/lib/logging";
import {GLOBAL_EVENTS} from "@/lib/tauri-events";
import {subscribeTauriEvent} from "@/lib/tauri-listener";
import {invokeWithStartupRetry} from "@/lib/tauri-startup-retry";

import {useNativeShell} from "@/hooks/use-native-shell";

const GLOBAL_ENABLED_STORAGE_KEY = "delta-auto-tools:global-enabled";

const GlobalEnabledContext = createContext<{
    globalEnabled: boolean;
    setGlobalEnabled: (enabled: boolean) => void;
}>({
    globalEnabled: true,
    setGlobalEnabled: () => {
    },
});

export function GlobalEnabledProvider({children}: { children: ReactNode }) {
    const isNativeShell = useNativeShell();
    const [globalEnabled, setGlobalEnabledState] = useState(() => {
        if (typeof window === "undefined") return true;
        try {
            const stored = window.localStorage.getItem(GLOBAL_ENABLED_STORAGE_KEY);
            return stored === null ? true : stored === "true";
        } catch {
            return true;
        }
    });

    useEffect(() => {
        if (!isNativeShell) return;

        let disposed = false;

        void invokeWithStartupRetry<boolean>("global_get_enabled").then((enabled) => {
            if (disposed) return;
            setGlobalEnabledState(enabled);
            try {
                window.localStorage.setItem(GLOBAL_ENABLED_STORAGE_KEY, String(enabled));
            } catch {
            }
        });

        const unlisten = subscribeTauriEvent<boolean>(GLOBAL_EVENTS.enabledChanged, (event) => {
            if (disposed) return;
            setGlobalEnabledState(event.payload);
            try {
                window.localStorage.setItem(GLOBAL_ENABLED_STORAGE_KEY, String(event.payload));
            } catch {
            }
        });

        return () => {
            disposed = true;
            unlisten();
        };
    }, [isNativeShell]);

    const setGlobalEnabled = useCallback(
        (enabled: boolean) => {
            setGlobalEnabledState(enabled);
            try {
                window.localStorage.setItem(GLOBAL_ENABLED_STORAGE_KEY, String(enabled));
            } catch {
            }
            if (isNativeShell) {
                void invoke("global_set_enabled", {enabled});
            }
        },
        [isNativeShell],
    );

    return (
        <GlobalEnabledContext.Provider value={{globalEnabled, setGlobalEnabled}}>
            {children}
        </GlobalEnabledContext.Provider>
    );
}

export function useGlobalEnabled() {
    return useContext(GlobalEnabledContext);
}

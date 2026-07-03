import {
    createContext,
    type ReactNode,
    useCallback,
    useContext,
    useEffect,
    useMemo,
    useRef,
    useState,
} from "react";
import {invoke} from "@tauri-apps/api/core";

import {listen} from "@tauri-apps/api/event";
import {THEME_EVENTS} from "@/lib/tauri-events";
import {useNativeShell} from "@/hooks/use-native-shell";
import {
    type ThemeBootstrap,
    type ThemeDefinition,
    type ThemeSettings,
    type ThemeTokenOverride,
    THEME_STORAGE_KEY,
} from "@/components/app/theme-types";
import {
    applyPersistedThemeTokens,
    buildCustomOverrideSettings,
    previewThemeTokens,
    restorePersistedThemeTokens,
    type ThemeTokenSession,
} from "@/components/app/theme-utils";

/** 主题 Context 对外暴露的接口。 */
type ThemeContextValue = {
    /** 当前 bootstrap（含全部主题列表与合并后的 token）。 */
    bootstrap: ThemeBootstrap | null;
    /** 是否正在加载初始 bootstrap。 */
    loading: boolean;
    /** 错误信息。 */
    error: string | null;
    /** 保存主题设置（含 activeThemeId/customThemes/overrides），保存后 Rust 会 emit changed 事件。 */
    saveSettings: (settings: ThemeSettings) => Promise<void>;
    /** 仅切换激活主题（不动 customThemes/overrides）。 */
    setActiveTheme: (themeId: string) => Promise<void>;
    /** 保存自定义配色，并取消当前预设主题选中态。 */
    setOverrides: (overrides: ThemeTokenOverride[]) => Promise<void>;
    /** 把自定义主题加入列表并设为激活。 */
    addCustomTheme: (theme: ThemeDefinition) => Promise<void>;
    /** 删除自定义主题。 */
    deleteCustomTheme: (themeId: string) => Promise<void>;
    /** 重命名自定义主题。 */
    renameCustomTheme: (themeId: string, name: string) => Promise<void>;
    /**
     * 实时预览：将指定 token 列表写入 documentElement（不持久化）。
     * 主题面板用此接口预览颜色变更，避免自己操作 CSS 变量导致与 Provider 不同步。
     */
    previewTokens: (tokens: readonly ThemeTokenOverride[], options?: {persistOnClose?: boolean}) => void;
    /** 恢复到最近一次已持久化的 token 列表。 */
    restorePersistedTokens: () => void;
};

const ThemeContext = createContext<ThemeContextValue | null>(null);

export function useTheme(): ThemeContextValue {
    const ctx = useContext(ThemeContext);
    if (!ctx) {
        throw new Error("useTheme 必须在 ThemeProvider 内使用");
    }
    return ctx;
}

type ThemeProviderProps = {
    children: ReactNode;
};

function isThemeBootstrapStateError(err: unknown): boolean {
    const message = String(err);
    return message.includes("state not managed") && message.includes("theme_get_bootstrap");
}

/**
 * 主题 Provider。
 *
 * - 浏览器预览模式：从 localStorage 读取 fallback bootstrap（仅含 activeThemeId），
 *   CSS 变量注入由首次 bootstrap 的 mergedTokens 完成。
 * - native shell：调 `theme_get_bootstrap` 拿真实配置，listen `theme://changed` 事件刷新 CSS 变量。
 *
 * CSS 变量注入策略：每次收到 mergedTokens（bootstrap 或 changed 事件）时，
 * 用 `applyThemeTokens` 原子化切换 `document.documentElement` 的 inline style，
 * 先清除上一次应用的 token 再写入新的，避免旧主题残留。
 */
export function ThemeProvider({children}: ThemeProviderProps) {
    const isNativeShell = useNativeShell();
    const [bootstrap, setBootstrap] = useState<ThemeBootstrap | null>(null);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);

    // 记录上一次应用和持久化的 token 列表，用于预览/恢复
    const tokenSessionRef = useRef<ThemeTokenSession>({
        appliedTokens: [],
        persistedTokens: [],
    });

    // 把 mergedTokens 写入 documentElement 的 inline style
    const applyTokens = useCallback((tokens: readonly ThemeTokenOverride[]) => {
        tokenSessionRef.current = applyPersistedThemeTokens(
            document.documentElement,
            tokens,
            tokenSessionRef.current,
        );
    }, []);

    // 初始化：拉取 bootstrap
    useEffect(() => {
        if (!isNativeShell) {
            // 浏览器预览模式：不调后端，直接结束 loading（CSS 变量保持 :root 默认值）
            setLoading(false);
            return;
        }

        let disposed = false;
        void invoke<ThemeBootstrap>("theme_get_bootstrap")
            .then((boot) => {
                if (disposed) return;
                setBootstrap(boot);
                applyTokens(boot.mergedTokens);
                // 同步一份到 localStorage 供浏览器预览/下次启动快速恢复
                try {
                    window.localStorage.setItem(
                        THEME_STORAGE_KEY,
                        JSON.stringify({activeThemeId: boot.activeThemeId}),
                    );
                } catch {
                    // 隐私模式 / 配额限制：静默吞掉
                }
            })
            .catch((err: unknown) => {
                if (disposed) return;
                if (isThemeBootstrapStateError(err)) {
                    setError(null);
                    return;
                }
                setError(String(err));
            })
            .finally(() => {
                if (!disposed) setLoading(false);
            });

        return () => {
            disposed = true;
        };
    }, [isNativeShell, applyTokens]);

    // 监听 theme://changed 事件：Rust 端 save_settings 后推送合并后的 token 列表
    useEffect(() => {
        if (!isNativeShell) return;

        let disposed = false;
        let unlisten: (() => void) | undefined;

        void listen<ThemeTokenOverride[]>(THEME_EVENTS.changed, (event) => {
            if (disposed) return;
            applyTokens(event.payload);
        }).then((dispose) => {
            unlisten = dispose;
        });

        // 同时刷新 bootstrap 以拿到最新的 customThemes 列表
        void invoke<ThemeBootstrap>("theme_get_bootstrap")
            .then((boot) => {
                if (disposed) return;
                setBootstrap(boot);
            })
            .catch((err: unknown) => {
                if (disposed) return;
                if (isThemeBootstrapStateError(err)) {
                    setError(null);
                    return;
                }
                setError(String(err));
            });

        return () => {
            disposed = true;
            unlisten?.();
        };
    }, [isNativeShell, applyTokens]);

    /** 内部：根据当前 bootstrap 构造下一次要保存的 settings，调 save_settings。 */
    const persistSettings = useCallback(
        async (mutator: (current: ThemeBootstrap) => ThemeSettings): Promise<void> => {
            if (!isNativeShell) return;
            if (!bootstrap) {
                setError("主题尚未加载完成");
                return;
            }
            const next = mutator(bootstrap);
            try {
                // save_settings 返回最新的 ThemeBootstrap（含合并后的 mergedTokens），
                // 立即用它更新 bootstrap 与 CSS 变量，不必等待 theme://changed 事件。
                // 这样面板关闭时可恢复到最新持久化 token，
                // 不会用过时的主题色覆盖已保存的自定义颜色。
                const returned = await invoke<ThemeBootstrap>("theme_save_settings", {settingsValue: next});
                setBootstrap(returned);
                applyTokens(returned.mergedTokens);
            } catch (err: unknown) {
                setError(String(err));
                throw err;
            }
        },
        [isNativeShell, bootstrap, applyTokens],
    );

    const saveSettings = useCallback(
        async (settings: ThemeSettings) => {
            await persistSettings(() => settings);
        },
        [persistSettings],
    );

    const setActiveTheme = useCallback(
        async (themeId: string) => {
            await persistSettings((current) => ({
                activeThemeId: themeId,
                customThemes: current.customThemes,
                overrides: [], // 切主题时清空 overrides
            }));
        },
        [persistSettings],
    );

    const setOverrides = useCallback(
        async (overrides: ThemeTokenOverride[]) => {
            await persistSettings((current) => buildCustomOverrideSettings(current, overrides));
        },
        [persistSettings],
    );

    const addCustomTheme = useCallback(
        async (theme: ThemeDefinition) => {
            await persistSettings((current) => ({
                activeThemeId: theme.id,
                customThemes: [...current.customThemes, theme],
                overrides: [],
            }));
        },
        [persistSettings],
    );

    const deleteCustomTheme = useCallback(
        async (themeId: string) => {
            await persistSettings((current) => {
                const customThemes = current.customThemes.filter((t) => t.id !== themeId);
                // 若删除的是当前激活主题，回退到第一个内置主题
                const activeThemeId =
                    current.activeThemeId === themeId
                        ? current.builtinThemes[0]?.id ?? ""
                        : current.activeThemeId;
                return {activeThemeId, customThemes, overrides: []};
            });
        },
        [persistSettings],
    );

    const renameCustomTheme = useCallback(
        async (themeId: string, name: string) => {
            await persistSettings((current) => ({
                activeThemeId: current.activeThemeId,
                customThemes: current.customThemes.map((t) =>
                    t.id === themeId ? {...t, name} : t,
                ),
                overrides: current.overrides,
            }));
        },
        [persistSettings],
    );

    /** 预览 token：直接写入 CSS 变量但不持久化。面板卸载后由 Provider 统一恢复。 */
    const previewTokens = useCallback((
        tokens: readonly ThemeTokenOverride[],
        options?: {persistOnClose?: boolean},
    ) => {
        tokenSessionRef.current = previewThemeTokens(
            document.documentElement,
            tokens,
            tokenSessionRef.current,
            options,
        );
    }, []);

    const restorePersistedTokens = useCallback(() => {
        tokenSessionRef.current = restorePersistedThemeTokens(
            document.documentElement,
            tokenSessionRef.current,
        );
    }, []);

    const value = useMemo<ThemeContextValue>(
        () => ({
            bootstrap,
            loading,
            error,
            saveSettings,
            setActiveTheme,
            setOverrides,
            addCustomTheme,
            deleteCustomTheme,
            renameCustomTheme,
            previewTokens,
            restorePersistedTokens,
        }),
        [
            bootstrap,
            loading,
            error,
            saveSettings,
            setActiveTheme,
            setOverrides,
            addCustomTheme,
            deleteCustomTheme,
            renameCustomTheme,
            previewTokens,
            restorePersistedTokens,
        ],
    );

    return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}

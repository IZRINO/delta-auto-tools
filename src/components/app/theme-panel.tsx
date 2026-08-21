import {useCallback, useEffect, useMemo, useRef, useState} from "react";
import {RiDownloadLine, RiUploadLine, RiCheckLine, RiPaletteLine} from "@remixicon/react";

import {Button} from "@/components/ui/button";
import {ScrollArea} from "@/components/ui/scroll-area";
import {FieldUnit, SoftAlert} from "@/components/app/app-ui";
import {cn} from "@/lib/utils";

import {
    EDITABLE_TOKEN_KEYS,
    type ThemeDefinition,
    type ThemeSettings,
    type ThemeTokenOverride,
    TOKEN_LABELS,
    type UiWorld,
} from "@/components/app/theme-types";
import {
    buildCustomOverrideSettings,
    mergeThemeTokens,
    parseImportedTheme,
    serializeThemeForExport,
} from "@/components/app/theme-utils";
import {useTheme} from "@/hooks/use-theme";
import {ThemeColorPicker} from "@/components/app/theme-color-picker";

/**
 * 主题面板：预设 / Tokens 编辑 / 导入导出 三段式。
 *
 * 复用 `useTheme()` 拿到的状态与保存函数，自身只维护「编辑中的 overrides」本地态。
 * 切换预设会清空 overrides；保存时调 `setOverrides` 或 `setActiveTheme` 落盘。
 *
 * CSS 变量预览统一通过 `previewTokens` 由 ThemeProvider 管理，
 * 面板卸载时 Provider 自动恢复到最近一次持久化的 token，不会出现不同步。
 */
export function ThemePanel() {
    const {
        bootstrap,
        loading,
        error,
        saveSettings,
        setActiveTheme,
        setOverrides,
        addCustomTheme,
        previewTokens,
        restorePersistedTokens,
        uiWorld,
        setUiWorld,
    } = useTheme();

    // 本地编辑态：当前激活主题 + overrides 的合并结果，用于颜色选择器实时预览
    // 用 bootstrap.overrides 做初始值，避免 mount 时空数组被 preview 写入 CSS 导致颜色闪烁回预设主题
    const [localOverrides, setLocalOverrides] = useState<ThemeTokenOverride[]>(
        () => bootstrap?.overrides ?? [],
    );
    const pendingSettingsRef = useRef<ThemeSettings | null>(null);
    const saveTimeoutRef = useRef<number | null>(null);
    const saveSettingsRef = useRef(saveSettings);

    useEffect(() => {
        saveSettingsRef.current = saveSettings;
    }, [saveSettings]);

    const flushPendingSettings = useCallback(() => {
        if (saveTimeoutRef.current !== null) {
            window.clearTimeout(saveTimeoutRef.current);
            saveTimeoutRef.current = null;
        }
        const pending = pendingSettingsRef.current;
        pendingSettingsRef.current = null;
        if (pending) {
            void saveSettingsRef.current(pending);
        }
    }, []);

    const scheduleCustomSettingsSave = useCallback((settings: ThemeSettings) => {
        pendingSettingsRef.current = settings;
        if (saveTimeoutRef.current !== null) {
            window.clearTimeout(saveTimeoutRef.current);
        }
        saveTimeoutRef.current = window.setTimeout(() => {
            flushPendingSettings();
        }, 250);
    }, [flushPendingSettings]);

    const cancelPendingSettingsSave = useCallback(() => {
        if (saveTimeoutRef.current !== null) {
            window.clearTimeout(saveTimeoutRef.current);
            saveTimeoutRef.current = null;
        }
        pendingSettingsRef.current = null;
    }, []);

    // 当前激活主题定义（自定义 + 内置中查找）
    const activeTheme = useMemo<ThemeDefinition | undefined>(() => {
        if (!bootstrap) return undefined;
        if (bootstrap.activeThemeId === "") {
            return {
                id: "__custom__",
                name: "自定义配色",
                builtin: false,
                tokens: bootstrap.overrides.length > 0 ? bootstrap.overrides : bootstrap.mergedTokens,
            };
        }
        const all = [...bootstrap.customThemes, ...bootstrap.builtinThemes];
        return all.find((t) => t.id === bootstrap.activeThemeId);
    }, [bootstrap]);

    // bootstrap 变化时同步本地 overrides
    useEffect(() => {
        setLocalOverrides(bootstrap?.overrides ?? []);
    }, [bootstrap]);

    // 实时预览：localOverrides 或 activeTheme 变化时通过 ThemeProvider 统一写入 CSS 变量
    useEffect(() => {
        if (!activeTheme) return;
        const merged = mergeThemeTokens(activeTheme, localOverrides);
        previewTokens(merged);
    }, [activeTheme, localOverrides, previewTokens]);

    // 面板卸载时恢复 ThemeProvider 最近一次持久化的 CSS 变量
    useEffect(() => {
        return () => {
            flushPendingSettings();
            restorePersistedTokens();
        };
    }, [flushPendingSettings, restorePersistedTokens]);

    // 找到某个 editable token 在 previewTokens 中的当前值（fallback 到 activeTheme 原值）
    const tokenValue = (key: string): string => {
        const fromOverride = localOverrides.find((o) => o.key === key);
        if (fromOverride) return fromOverride.value;
        const fromTheme = activeTheme?.tokens.find((t) => t.key === key);
        return fromTheme?.value ?? "";
    };

    const updateToken = (key: string, value: string) => {
        if (!activeTheme || !bootstrap) return;
        setLocalOverrides((prev) => {
            const without = prev.filter((o) => o.key !== key);
            const next = [...without, {key, value}];
            previewTokens(mergeThemeTokens(activeTheme, next), {persistOnClose: true});
            scheduleCustomSettingsSave(buildCustomOverrideSettings(bootstrap, next));
            return next;
        });
    };

    const handlePresetClick = async (themeId: string) => {
        if (!bootstrap) return;
        // 切换预设：清空 overrides 并保存
        cancelPendingSettingsSave();
        const all = [...bootstrap.customThemes, ...bootstrap.builtinThemes];
        const nextTheme = all.find((theme) => theme.id === themeId);
        if (nextTheme) {
            previewTokens(nextTheme.tokens, {persistOnClose: true});
        }
        setLocalOverrides([]);
        await setActiveTheme(themeId);
    };

    const handleSaveOverrides = async () => {
        await setOverrides(localOverrides);
    };

    const handleDiscardOverrides = () => {
        cancelPendingSettingsSave();
        if (bootstrap?.mergedTokens) {
            previewTokens(bootstrap.mergedTokens, {persistOnClose: true});
        }
        setLocalOverrides(bootstrap?.overrides ?? []);
    };

    const handleExport = async () => {
        if (!activeTheme) return;
        const json = serializeThemeForExport(activeTheme);
        try {
            const blob = new Blob([json], {type: "application/json"});
            const url = URL.createObjectURL(blob);
            const a = document.createElement("a");
            a.href = url;
            a.download = `theme-${activeTheme.id}.json`;
            a.click();
            URL.revokeObjectURL(url);
        } catch {
            // 浏览器预览模式无下载能力：复制到剪贴板
            await navigator.clipboard.writeText(json);
        }
    };

    const handleImportClick = () => {
        const input = document.createElement("input");
        input.type = "file";
        input.accept = "application/json,.json";
        input.onchange = async () => {
            const file = input.files?.[0];
            if (!file) return;
            const text = await file.text();
            try {
                const theme = parseImportedTheme(text);
                // 导入后派生为新自定义主题（生成新 id 避免冲突）
                const imported: ThemeDefinition = {
                    ...theme,
                    id: `imported-${Date.now()}`,
                };
                await addCustomTheme(imported);
            } catch (err) {
                alert(`导入失败：${err instanceof Error ? err.message : String(err)}`);
            }
        };
        input.click();
    };

    if (loading) {
        return (
            <div className="flex min-h-[200px] items-center justify-center gap-2 text-sm text-base-content/70">
                <span className="loading loading-spinner loading-sm"/>
                正在加载主题
            </div>
        );
    }

    if (error) {
        return (
            <SoftAlert className="text-sm">
                主题加载失败：{error}
            </SoftAlert>
        );
    }

    if (!bootstrap) {
        return (
            <div className="flex flex-col gap-4">
                <UiWorldPicker uiWorld={uiWorld} onChange={setUiWorld}/>
                <div className="alert text-sm text-base-content/70">
                    浏览器预览模式不支持配色切换，请在桌面应用内使用。
                </div>
            </div>
        );
    }

    const allThemes = [...bootstrap.customThemes, ...bootstrap.builtinThemes];
    const dirty =
        JSON.stringify(localOverrides) !== JSON.stringify(bootstrap.overrides);

    return (
        <ScrollArea className="max-h-[60vh]">
            <div className="flex flex-col gap-4 pr-3">
                <UiWorldPicker uiWorld={uiWorld} onChange={setUiWorld}/>
                {/* 预设区 */}
                <FieldUnit header="预设主题">
                    {/* 当存在自定义 overrides 时显示「已自定义」状态条，预设均不选中 */}
                    {bootstrap.overrides.length > 0 && (
                        <SoftAlert className="mb-3 py-2 text-sm" tone="info">
                            <RiPaletteLine className="size-3" aria-hidden="true"/>
                            已自定义配色，点击预设可恢复原主题配色
                        </SoftAlert>
                    )}
                    <div className="grid grid-cols-2 gap-2 sm:grid-cols-3">
                        {allThemes.map((theme) => {
                            // 存在自定义 overrides 时不选中任何预设，避免误点击导致 overrides 被清空
                            const isActive = theme.id === bootstrap.activeThemeId && bootstrap.overrides.length === 0;
                            // 用主题前 4 个 daisyUI 语义色做缩略预览：底色/文字/主色/错误色
                            const swatches = ["--color-base-100", "--color-base-content", "--color-primary", "--color-error"]
                                .map((k) => theme.tokens.find((t) => t.key === k)?.value ?? "")
                                .filter(Boolean);
                            return (
                                <button
                                    key={theme.id}
                                    type="button"
                                    onClick={() => handlePresetClick(theme.id)}
                                    className={cn(
                                        "card card-border group relative flex flex-col gap-2 bg-base-100 p-3 text-left transition-colors",
                                        isActive
                                            ? "border-primary bg-primary/10"
                                            : "hover:border-primary/60 hover:bg-base-200",
                                    )}
                                >
                                    <div className="flex h-7 w-full overflow-hidden rounded-field border border-base-300">
                                        {swatches.map((c, i) => (
                                            <span
                                                key={i}
                                                className="flex-1"
                                                style={{backgroundColor: c}}
                                                aria-hidden="true"
                                            />
                                        ))}
                                    </div>
                                    <div className="flex items-center justify-between gap-1">
                                        <span
                                            className={cn(
                                                "truncate text-sm font-semibold",
                                                theme.builtin ? "text-base-content" : "text-base-content",
                                            )}
                                        >
                                            {theme.name}
                                        </span>
                                        {isActive ? (
                                            <RiCheckLine
                                                className="size-3.5 shrink-0 text-base-content"
                                                aria-hidden="true"
                                            />
                                        ) : null}
                                    </div>
                                    <span className="badge badge-ghost badge-sm w-fit">
                                        {theme.builtin ? "内置" : "自定义"}
                                    </span>
                                </button>
                            );
                        })}
                    </div>
                </FieldUnit>

                {/* Tokens 编辑区 */}
                <FieldUnit
                    header="自定义颜色"
                    footer={
                        dirty ? (
                            <div className="flex items-center justify-end gap-2">
                                <Button
                                    variant="ghost"
                                    size="sm"
                                    onClick={handleDiscardOverrides}
                                    className="h-7 text-xs"
                                >
                                    撤销
                                </Button>
                                <Button
                                    size="sm"
                                    onClick={handleSaveOverrides}
                                    className="h-7 text-xs"
                                >
                                    保存自定义
                                </Button>
                            </div>
                        ) : null
                    }
                >
                    <div className="flex flex-col gap-2">
                        {EDITABLE_TOKEN_KEYS.map((key) => (
                            <div
                                key={key}
                                className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-3"
                            >
                                <div className="min-w-0">
                                    <p className="truncate text-sm font-medium">
                                        {TOKEN_LABELS[key] ?? key}
                                    </p>
                                    <p className="truncate font-mono text-xs text-base-content/60">
                                        {key}
                                    </p>
                                </div>
                                <ThemeColorPicker
                                    value={tokenValue(key)}
                                    onChange={(v) => updateToken(key, v)}
                                    label={TOKEN_LABELS[key]}
                                />
                            </div>
                        ))}
                    </div>
                </FieldUnit>

                {/* 导入导出 */}
                <FieldUnit header="导入导出">
                    <div className="flex flex-wrap gap-2">
                        <Button
                            variant="outline"
                            size="sm"
                            onClick={handleExport}
                            disabled={!activeTheme}
                            className="h-8 text-xs"
                        >
                            <RiDownloadLine
                                className="size-3.5"
                                data-icon="inline-start"
                                aria-hidden="true"
                            />
                            导出当前主题
                        </Button>
                        <Button
                            variant="outline"
                            size="sm"
                            onClick={handleImportClick}
                            className="h-8 text-xs"
                        >
                            <RiUploadLine
                                className="size-3.5"
                                data-icon="inline-start"
                                aria-hidden="true"
                            />
                            导入主题 JSON
                        </Button>
                    </div>
                    <p className="mt-2 text-xs text-base-content/60">
                        导入的主题会作为新自定义主题加入列表，不影响内置主题。
                    </p>
                </FieldUnit>
            </div>
        </ScrollArea>
    );
}

function UiWorldPicker({onChange, uiWorld}: {onChange: (world: UiWorld) => void; uiWorld: UiWorld}) {
    return (
        <FieldUnit header="界面世界">
            <p className="mb-2 text-xs text-base-content/60">
                与配色正交。三套配色只服务战地。overlay 窗不跟随。
            </p>
            <div className="grid grid-cols-2 gap-2">
                <button
                    className={cn(
                        "btn btn-outline h-12 min-h-12 justify-start rounded-field",
                        uiWorld === "console" && "btn-active border-primary",
                    )}
                    onClick={() => onChange("console")}
                    type="button"
                >
                    战地控制台
                </button>
                <button
                    className={cn(
                        "btn btn-outline h-12 min-h-12 justify-start rounded-field",
                        uiWorld === "blackmark" && "btn-active border-primary",
                    )}
                    onClick={() => onChange("blackmark")}
                    type="button"
                >
                    夜航黑标
                </button>
            </div>
        </FieldUnit>
    );
}

import {useEffect, useMemo, useRef, useState} from "react";
import {RiDownloadLine, RiUploadLine, RiCheckLine} from "@remixicon/react";

import {Button} from "@/components/ui/button";
import {ScrollArea} from "@/components/ui/scroll-area";
import {FieldUnit} from "@/components/app/app-ui";
import {cn} from "@/lib/utils";

import {
    EDITABLE_TOKEN_KEYS,
    type ThemeDefinition,
    type ThemeTokenOverride,
    TOKEN_LABELS,
} from "@/components/app/theme-types";
import {
    applyThemeTokens,
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
 */
export function ThemePanel() {
    const {bootstrap, loading, error, setActiveTheme, setOverrides, addCustomTheme} = useTheme();

    // 本地编辑态：当前激活主题 + overrides 的合并结果，用于颜色选择器实时预览
    const [localOverrides, setLocalOverrides] = useState<ThemeTokenOverride[]>([]);

    // 当前激活主题定义（自定义 + 内置中查找）
    const activeTheme = useMemo<ThemeDefinition | undefined>(() => {
        if (!bootstrap) return undefined;
        const all = [...bootstrap.customThemes, ...bootstrap.builtinThemes];
        return all.find((t) => t.id === bootstrap.activeThemeId);
    }, [bootstrap]);

    // bootstrap 变化时同步本地 overrides
    useEffect(() => {
        setLocalOverrides(bootstrap?.overrides ?? []);
    }, [bootstrap]);

    // 实时预览：localOverrides 或 activeTheme 变化时立即写入 documentElement
    const previewTokensRef = useRef<readonly ThemeTokenOverride[]>([]);
    useEffect(() => {
        if (!activeTheme) return;
        const merged = mergeThemeTokens(activeTheme, localOverrides);
        applyThemeTokens(document.documentElement, merged, previewTokensRef.current);
        previewTokensRef.current = merged;
    }, [activeTheme, localOverrides]);

    // 找到某个 editable token 在 previewTokens 中的当前值（fallback 到 activeTheme 原值）
    const tokenValue = (key: string): string => {
        const fromOverride = localOverrides.find((o) => o.key === key);
        if (fromOverride) return fromOverride.value;
        const fromTheme = activeTheme?.tokens.find((t) => t.key === key);
        return fromTheme?.value ?? "";
    };

    const updateToken = (key: string, value: string) => {
        setLocalOverrides((prev) => {
            const without = prev.filter((o) => o.key !== key);
            return [...without, {key, value}];
        });
    };

    const handlePresetClick = async (themeId: string) => {
        // 切换预设：清空 overrides 并保存
        setLocalOverrides([]);
        await setActiveTheme(themeId);
    };

    const handleSaveOverrides = async () => {
        await setOverrides(localOverrides);
    };

    const handleDiscardOverrides = () => {
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
            <div className="flex min-h-[200px] items-center justify-center font-mono text-xs text-[var(--zinc)]">
                [ 正在加载主题... ]
            </div>
        );
    }

    if (error) {
        return (
            <div className="border-2 border-[var(--alert-red)] bg-[var(--alert-red)]/10 px-3 py-2 font-mono text-xs text-[var(--alert-red)]">
                主题加载失败：{error}
            </div>
        );
    }

    if (!bootstrap) {
        return (
            <div className="font-mono text-xs text-[var(--zinc)]">
                浏览器预览模式不支持主题切换，请在桌面应用内使用。
            </div>
        );
    }

    const allThemes = [...bootstrap.customThemes, ...bootstrap.builtinThemes];
    const dirty =
        JSON.stringify(localOverrides) !== JSON.stringify(bootstrap.overrides);

    return (
        <ScrollArea className="max-h-[60vh]">
            <div className="flex flex-col gap-4 pr-3">
                {/* 预设区 */}
                <FieldUnit header="[ PRESET / 预设主题 ]">
                    <div className="grid grid-cols-2 gap-2 sm:grid-cols-3">
                        {allThemes.map((theme) => {
                            const isActive = theme.id === bootstrap.activeThemeId;
                            // 用主题前 4 个语义色做缩略预览
                            const swatches = ["--carbon", "--chalk", "--amber", "--alert-red"]
                                .map((k) => theme.tokens.find((t) => t.key === k)?.value ?? "")
                                .filter(Boolean);
                            return (
                                <button
                                    key={theme.id}
                                    type="button"
                                    onClick={() => handlePresetClick(theme.id)}
                                    className={cn(
                                        "group relative flex flex-col gap-2 border-2 p-2 text-left transition-colors",
                                        isActive
                                            ? "border-[var(--amber)] bg-[var(--amber)]/10"
                                            : "border-[var(--chalk)] hover:border-[var(--zinc)] hover:bg-[var(--slate)]",
                                    )}
                                >
                                    <div className="flex h-6 w-full overflow-hidden border border-[var(--seam)]">
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
                                                "truncate text-xs font-black tracking-tight",
                                                theme.builtin ? "text-[var(--chalk)]" : "text-[var(--chalk)]",
                                            )}
                                        >
                                            {theme.name}
                                        </span>
                                        {isActive ? (
                                            <RiCheckLine
                                                className="size-3.5 shrink-0 text-[var(--amber)]"
                                                aria-hidden="true"
                                            />
                                        ) : null}
                                    </div>
                                    <span className="font-mono text-[0.58rem] font-bold tracking-[0.12em] text-[var(--zinc)] uppercase">
                                        {theme.builtin ? "BUILTIN" : "CUSTOM"}
                                    </span>
                                </button>
                            );
                        })}
                    </div>
                </FieldUnit>

                {/* Tokens 编辑区 */}
                <FieldUnit
                    header="[ TOKENS / 自定义颜色 ]"
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
                                    className="h-7 border-2 border-[var(--chalk)] bg-[var(--chalk)] text-[var(--carbon)] text-xs hover:bg-[var(--chalk)]/90"
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
                                    <p className="truncate text-xs font-black tracking-tight">
                                        {TOKEN_LABELS[key] ?? key}
                                    </p>
                                    <p className="truncate font-mono text-[0.58rem] text-[var(--zinc)]">
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
                <FieldUnit header="[ IMPORT / EXPORT ]">
                    <div className="flex flex-wrap gap-2">
                        <Button
                            variant="outline"
                            size="sm"
                            onClick={handleExport}
                            disabled={!activeTheme}
                            className="h-8 border-2 border-[var(--chalk)] text-xs"
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
                            className="h-8 border-2 border-[var(--chalk)] text-xs"
                        >
                            <RiUploadLine
                                className="size-3.5"
                                data-icon="inline-start"
                                aria-hidden="true"
                            />
                            导入主题 JSON
                        </Button>
                    </div>
                    <p className="mt-2 font-mono text-[0.58rem] text-[var(--zinc)]">
                        导入的主题会作为新自定义主题加入列表，不影响内置主题。
                    </p>
                </FieldUnit>
            </div>
        </ScrollArea>
    );
}

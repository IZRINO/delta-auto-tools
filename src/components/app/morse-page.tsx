import {startTransition, useCallback, useEffect, useMemo, useRef, useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import {RiCheckboxCircleLine, RiHistoryLine, RiLayoutGridLine, RiRefreshLine,} from "@remixicon/react";

import {listen} from "@tauri-apps/api/event";
import {MORSE_EVENTS} from "@/lib/tauri-events";
import {useNativeShell} from "@/hooks/use-native-shell";
import {useAutosave} from "@/hooks/use-autosave";
import {useBootstrapForm} from "@/hooks/use-bootstrap-form";
import {useHotkeyRecorder} from "@/hooks/use-hotkey-recorder";

import {Badge} from "@/components/ui/badge";
import {Button} from "@/components/ui/button";
import {Input} from "@/components/ui/input";
import {Collapsible, CollapsibleContent, CollapsibleTrigger} from "@/components/ui/collapsible";
import {ScrollArea} from "@/components/ui/scroll-area";
import {Switch} from "@/components/ui/switch";
import {
    AppPage,
    ChannelTabs,
    ConfigRow,
    EmptyState,
    FieldUnit,
    HelpHint,
    MacroHeader,
    PagePreviewBanner,
    SaveStateBadge,
    StatusMatrix,
} from "@/components/app/app-ui";
import {RegionSelectionOverlay} from "@/components/app/morse-overlay";
import {
    AUTOSAVE_DELAY_MS,
    type MorseBootstrap,
    type MorsePageProps,
    type MorseRunResult,
    type MorseSettings,
    type MorseSettingsForm,
    CLICK_REGION_LABELS,
    REGION_LABELS,
    type RegionSelectionOutcome,
    type RegionSelectionProgress,
    type VerificationStatus,
} from "@/components/app/morse-types";
import {
    clickRegionRows,
    formatRecordedHotkey,
    formatRegion,
    formatTimestamp,
    getErrorMessage,
    normalizeRunDetails,
    parseOverlaySlots,
    parseSettingsForm,
    settingsToForm,
} from "@/components/app/morse-utils";

export function MorsePage({overlayMode = false}: MorsePageProps) {
    const overlaySlots = useMemo(() => (overlayMode ? parseOverlaySlots() : []), [overlayMode]);
    const isNativeShell = useNativeShell();
    const hotkeyButtonRef = useRef<HTMLButtonElement | null>(null);
    const [running, setRunning] = useState(false);
    const [selectingSlot, setSelectingSlot] = useState<number | null>(null);
    const [verificationValue, setVerificationValue] = useState("");
    const [verificationStatus, setVerificationStatus] = useState<VerificationStatus>("idle");
    const [verificationMessage, setVerificationMessage] = useState("点击验证输入框即可执行一次仅识别流程，结果会直接回填到这里。");
    const [activeTab, setActiveTab] = useState("selection");

    const bf = useBootstrapForm<MorseBootstrap, MorseSettings, MorseSettingsForm>({
        spec: {
            getBootstrapCommand: "morse_get_bootstrap",
            saveSettingsCommand: "morse_save_settings",
            settingsToForm,
            parseSettingsForm,
        },
        isNativeShell,
        skipInitialLoad: overlayMode,
        loadStatusMessage: "正在加载摩斯工具...",
        readyStatusMessage: "就绪，可开始框选区域或执行识别。",
        previewStatusMessage: "浏览器预览模式：当前仅验证布局与滚动，原生命令请在桌面端运行。",
        saveSuccessMessage: "设置已保存。新的热键与识别参数已生效。",
        saveInProgressMessage: "正在保存设置...",
        useStartTransition: true,
    });

    const {
        bootstrap,
        setBootstrap,
        form,
        setForm,
        isDirty,
        updateForm,
        saveSettings,
        syncBootstrap,
        loading,
        saving,
        pageError: _pageError,
        setPageError,
        statusMessage: _statusMessage,
        setStatusMessage,
        autosaveVersionRef: autosaveRef
    } = bf;

    const recorder = useHotkeyRecorder({
        formatKey: formatRecordedHotkey,
        onCommit: (key) => {
            updateForm("hotkey", key);
            setPageError(null);
        },
        onCancel: (draft) => updateForm("hotkey", draft),
        onStatusMessage: setStatusMessage,
        keyRecordedMessage: (key) => `新的热键已录制：${key}`,
        recordingCancelledMessage: "已取消热键录制。",
    });

    useAutosave<MorseSettingsForm>({
        form,
        isDirty,
        disabled: overlayMode || !isNativeShell || loading || !bootstrap || !form || recorder.isRecording || selectingSlot !== null,
        onSave: (formSnapshot, nextVersion) => {
            const settingsValue = parseSettingsForm(formSnapshot);
            return saveSettings(settingsValue, nextVersion);
        },
        onError: (message) => {
            setPageError(message);
            setStatusMessage(`保存失败：${message}`);
        },
        delay: AUTOSAVE_DELAY_MS,
        autosaveVersionRef: autosaveRef,
    });

    useEffect(() => {
        if (recorder.isRecording) {
            hotkeyButtonRef.current?.focus();
        }
    }, [recorder.isRecording]);

    useEffect(() => {
        if (overlayMode || !isNativeShell) {
            return;
        }
        let disposed = false;
        void invoke("morse_set_hotkey_recording", {recording: recorder.isRecording}).catch((error) => {
            if (!disposed) {
                const message = getErrorMessage(error);
                setPageError(message);
                setStatusMessage(message);
            }
        });
        return () => {
            disposed = true;
        };
    }, [isNativeShell, recorder.isRecording, overlayMode]);

    useEffect(() => {
        if (!overlayMode) {
            return;
        }
        document.body.dataset.overlayMode = "true";
        return () => {
            delete document.body.dataset.overlayMode;
        };
    }, [overlayMode]);

    useEffect(() => {
        if (overlayMode || !isNativeShell) {
            return;
        }
        let isDisposed = false;
        let unlistenRunFinished: (() => void) | undefined;
        let unlistenSelectionProgress: (() => void) | undefined;
        let unlistenHotkeyError: (() => void) | undefined;

        void listen<MorseRunResult>(MORSE_EVENTS.runFinished, async (event) => {
            if (isDisposed) return;
            const result = event.payload;
            startTransition(() => {
                setBootstrap((current) => (current ? {...current, latestRun: result} : current));
                setStatusMessage(result.error ? `识别失败：${result.error}` : `识别完成：${result.value ?? "无结果"}`);
            });
            try {
                await syncBootstrap({syncMode: "none"});
            } catch (error) {
                if (!isDisposed) {
                    setPageError(getErrorMessage(error));
                }
            }
        }).then((dispose) => {
            unlistenRunFinished = dispose;
        });

        void listen<RegionSelectionProgress>(MORSE_EVENTS.selectionProgress, (event) => {
            if (isDisposed) return;
            const progress = event.payload;
            startTransition(() => {
                setBootstrap((current) =>
                    current ? {...current, settings: {...current.settings, regions: progress.regions}} : current
                );
                setForm((current) =>
                    current ? {...current, regions: progress.regions} : current
                );
            });
        }).then((dispose) => {
            unlistenSelectionProgress = dispose;
        });

        void listen<string>(MORSE_EVENTS.hotkeyError, (event) => {
            if (isDisposed) return;
            startTransition(() => {
                setBootstrap((current) => (current ? {...current, hotkeyError: event.payload} : current));
            });
        }).then((dispose) => {
            unlistenHotkeyError = dispose;
        });

        return () => {
            isDisposed = true;
            unlistenRunFinished?.();
            unlistenSelectionProgress?.();
            unlistenHotkeyError?.();
        };
    }, [isNativeShell, overlayMode, syncBootstrap]);

    const latestRun = bootstrap?.latestRun ?? null;
    const history = bootstrap?.history ?? [];
    const savedSettings = bootstrap?.settings ?? null;
    const configuredCount = savedSettings?.regions.filter(Boolean).length ?? 0;
    const runDetails = normalizeRunDetails(latestRun);
    const latestResultValue = latestRun?.value ?? null;
    const hasLatestResult = Boolean(latestRun);
    const canRun = configuredCount === REGION_LABELS.length;
    const isBusy = loading || saving || running || selectingSlot !== null;

    const performSelectionSession = useCallback(async (slots: number[]) => {
        if (slots.length === 0) return false;
        if (!isNativeShell) {
            setStatusMessage("浏览器预览模式下不可执行区域框选，请在桌面端使用。");
            return false;
        }
        // ponytail: any slot ≥3 means click mode (sampling has only 0-2)
        const target = slots.some((s) => s >= REGION_LABELS.length) ? "click" : "sampling";
        const actualSlots = target === "click"
            ? slots.filter((s) => s < CLICK_REGION_LABELS.length)
            : slots;
        setSelectingSlot(actualSlots.length === 1 ? actualSlots[0] : -1);
        setStatusMessage(
            target === "click"
                ? "请在悬浮层中框选点击区域。"
                : actualSlots.length === REGION_LABELS.length
                    ? "请在悬浮层中依次完成 3 个区域框选。"
                    : `请在悬浮层中框选 ${REGION_LABELS[actualSlots[0]]}。`
        );
        try {
            const outcome = await invoke<RegionSelectionOutcome>("morse_begin_region_selection", {
                slots: actualSlots,
                target,
            });
            await syncBootstrap({syncMode: target === "click" ? "full" : "regions"});
            if (outcome.kind === "selected") {
                setStatusMessage(target === "click" ? "点击区域已更新。" : actualSlots.length === REGION_LABELS.length ? "3 个区域已全部更新。" : `${REGION_LABELS[actualSlots[0]]} 已更新。`);
                return true;
            }
            if (outcome.kind === "cancelled") {
                setStatusMessage("区域选择已取消。");
                return false;
            }
            setStatusMessage("区域选择窗口已关闭。");
            return false;
        } catch (error) {
            const message = getErrorMessage(error);
            setPageError(message);
            setStatusMessage(message);
            return false;
        } finally {
            setSelectingSlot(null);
        }
    }, [isNativeShell, syncBootstrap]);

    const handleVerificationRun = useCallback(async () => {
        if (!isNativeShell) {
            setVerificationStatus("error");
            setVerificationMessage("浏览器预览模式下不可执行测试验证，请在桌面端运行。");
            setStatusMessage("浏览器预览模式下不可执行识别，请在桌面端运行。");
            return;
        }
        setVerificationStatus("running");
        setVerificationMessage("正在执行一次仅识别测试验证...");
        try {
            setRunning(true);
            setStatusMessage("正在执行测试验证...");
            const result = await invoke<MorseRunResult>("morse_run_recognition", {autoType: false});
            startTransition(() => {
                setBootstrap((current) => (current ? {...current, latestRun: result} : current));
                setPageError(result.error);
                setStatusMessage(result.error ? `识别失败：${result.error}` : `识别完成：${result.value ?? "无结果"}`);
                if (result.error) {
                    setVerificationStatus("error");
                    setVerificationMessage(result.error);
                    return;
                }
                if (!result.value) {
                    setVerificationValue("");
                    setVerificationStatus("empty");
                    setVerificationMessage("识别流程已执行，但本次没有得到可用结果。");
                    return;
                }
                setVerificationValue(result.value);
                setVerificationStatus("success");
                setVerificationMessage("验证完成，结果已回填到输入框。再次聚焦会重新执行识别。");
            });
        } catch (error) {
            const message = getErrorMessage(error);
            setPageError(message);
            setStatusMessage(message);
            setVerificationStatus("error");
            setVerificationMessage(message);
        } finally {
            setRunning(false);
            try {
                await syncBootstrap({syncMode: "none"});
            } catch (error) {
                setPageError(getErrorMessage(error));
            }
        }
    }, [isNativeShell, syncBootstrap]);

    if (overlayMode) {
        return <RegionSelectionOverlay slots={overlaySlots}/>;
    }

    const statusItems: { id: string; state: "idle" | "active" | "valid" | "warning" | "error"; label: string }[] = [
        {id: "regions", state: configuredCount >= 1 ? "valid" : "idle", label: "采样窗位"},
        {id: "hotkey", state: form?.hotkey ? "valid" : "idle", label: "热键绑定"},
        {id: "threshold", state: form?.binaryThreshold ? "valid" : "idle", label: "二值化阈值"},
        {
            id: "verification",
            state: verificationStatus === "success" ? "valid" : verificationStatus === "error" ? "error" : verificationStatus === "running" ? "active" : "idle",
            label: "验证状态"
        },
        {id: "latest", state: latestResultValue ? "valid" : "idle", label: "最新报码"},
        {id: "ready", state: canRun ? "valid" : "warning", label: "就绪状态"},
    ];

    return (
        <AppPage className="auto-rows-max gap-3">
            <MacroHeader
                code="MX-01"
                title="MORSE / DECODER"
                subtitle="把采样窗位、阈值校准、单次验证与识别回溯串成一条硬线路。"
                badges={
                    <>
                        <Badge variant={canRun ? "default" : "secondary"}>{canRun ? "三区就绪" : "等待窗位标定"}</Badge>
                        <SaveStateBadge dirty={isDirty} saving={saving}/>
                    </>
                }
                actions={
                    <div className="flex items-center gap-2">
                        <Badge variant={isBusy ? "outline" : "secondary"}>{isBusy ? "链路占用" : "链路待命"}</Badge>
                    </div>
                }
            />

            {/* Status Matrix */}
            <div className="col-span-12">
                <StatusMatrix items={statusItems}/>
            </div>

            {!isNativeShell ? (
                <div className="col-span-12">
                    <PagePreviewBanner/>
                </div>
            ) : null}

            {/* Channel Tabs */}
            <div className="col-span-12">
                <ChannelTabs
                    tabs={[
                        {id: "selection", label: "窗位", active: activeTab === "selection"},
                        {id: "workbench", label: "校准", active: activeTab === "workbench"},
                        {id: "result", label: "报码", active: activeTab === "result"},
                        {id: "history", label: "档案", active: activeTab === "history"},
                    ]}
                    onTabChange={setActiveTab}
                />
            </div>

            {/* Tab Content */}
            <div className="col-span-12">
                {activeTab === "selection" && (
                    <FieldUnit header="[ UNIT 01 ] 采样阵列 — 锁定三段信号窗位">
                        <div className="grid gap-3 md:grid-cols-2 lg:grid-cols-3">
                            {REGION_LABELS.map((label, index) => {
                                const region = form?.regions[index] ?? null;
                                const isConfigured = Boolean(region);
                                return (
                                    <div key={label} className="border-2 border-[var(--chalk)] p-3">
                                        <div
                                            className="flex items-center justify-between gap-2 border-b border-[var(--seam)] pb-2 mb-2">
                                            <span
                                                className="font-mono text-xs font-black tracking-[0.18em] uppercase">{label}</span>
                                            <Badge variant={isConfigured ? "default" : "outline"}>
                                                {isConfigured ? "已锁定" : "待锁定"}
                                            </Badge>
                                        </div>
                                        {isConfigured ? (
                                            <div
                                                className="font-mono text-xs text-[var(--zinc)]">{formatRegion(region)}</div>
                                        ) : (
                                            <div className="font-mono text-xs text-[var(--dust)]">未配置</div>
                                        )}
                                        <Button
                                            className="mt-2 w-full"
                                            disabled={isBusy}
                                            onClick={() => void performSelectionSession([index])}
                                            type="button"
                                            variant={isConfigured ? "outline" : "default"}
                                            size="sm"
                                        >
                                            {isConfigured ? "重选" : "框选"}
                                        </Button>
                                    </div>
                                );
                            })}
                        </div>
                        <div className="mt-3 flex gap-2">
                            <Button
                                className="flex-1"
                                disabled={isBusy}
                                onClick={() => void performSelectionSession([0, 1, 2])}
                                type="button"
                            >
                                <RiRefreshLine data-icon="inline-start"/>
                                一次框选三段窗位
                            </Button>
                        </div>
                    </FieldUnit>
                )}

                {activeTab === "workbench" && (
                    <FieldUnit header="[ UNIT 02 ] 参数机架 — 校准识别链路">
                        <div className="grid gap-4 md:grid-cols-2">
                            <div className="space-y-3">
                                <ConfigRow
                                    label="热键"
                                    value={
                                        <Button
                                            ref={hotkeyButtonRef}
                                            className="h-auto w-full justify-between gap-4 border-2 border-[var(--chalk)] px-3 py-2 font-mono text-xs"
                                            onBlur={recorder.handleBlur}
                                            onClick={() => form && recorder.beginRecording(form.hotkey)}
                                            onKeyDown={recorder.handleKeyDown}
                                            type="button"
                                            variant="outline"
                                        >
                                            <span>{recorder.isRecording ? "正在录制..." : form?.hotkey || "点击录制"}</span>
                                            <HelpHint content="点击后按下目标快捷键，失焦取消录制。"/>
                                        </Button>
                                    }
                                    state={form?.hotkey ? "valid" : "idle"}
                                />
                                <ConfigRow
                                    label="二值化阈值"
                                    value={
                                        <Input
                                            className="border-2 border-[var(--chalk)] font-mono text-xs"
                                            inputMode="numeric"
                                            max="255"
                                            min="0"
                                            onChange={(e) => updateForm("binaryThreshold", e.currentTarget.value)}
                                            value={form?.binaryThreshold ?? ""}
                                        />
                                    }
                                    state={form?.binaryThreshold ? "valid" : "idle"}
                                />
                                <ConfigRow
                                    label="自动输入延迟"
                                    value={
                                        <Input
                                            className="border-2 border-[var(--chalk)] font-mono text-xs"
                                            inputMode="numeric"
                                            min="0"
                                            onChange={(e) => updateForm("autoInputDelay", e.currentTarget.value)}
                                            value={form?.autoInputDelay ?? ""}
                                        />
                                    }
                                    unit="ms"
                                    state={form?.autoInputDelay ? "valid" : "idle"}
                                />
                                <div className="flex items-center gap-2 border-b border-[var(--seam)] px-3 py-2">
                                    <Switch
                                        checked={form?.autoClickEnabled ?? false}
                                        disabled={isBusy}
                                        onCheckedChange={(v) => updateForm("autoClickEnabled", v)}
                                    />
                                    <span
                                        className="font-mono text-xs font-black tracking-[0.18em] uppercase">自动点击链路</span>
                                    <HelpHint content="识别成功后按设定顺序执行点击。"/>
                                </div>
                                {form?.autoClickEnabled && (
                                    <div className="space-y-3 px-3 pb-3">
                                        <ConfigRow
                                            label="点击完成后按键"
                                            value={
                                                <Input
                                                    className="border-2 border-[var(--chalk)] font-mono text-xs"
                                                    placeholder="留空不执行，例如 F4"
                                                    onChange={(e) => updateForm("afterClickHotkey", e.currentTarget.value)}
                                                    value={form?.afterClickHotkey ?? ""}
                                                />
                                            }
                                            state={form?.afterClickHotkey ? "valid" : "idle"}
                                        />
                                        <Collapsible className="border-2 border-[var(--chalk)] bg-[var(--carbon)]">
                                            <CollapsibleTrigger asChild>
                                                <Button
                                                    className="h-auto w-full justify-between rounded-none px-3 py-2 font-mono text-xs font-black tracking-[0.18em]"
                                                    type="button" variant="ghost">
                                                    点击区域配置
                                                    <Badge variant="outline">{(form?.clickRegions ?? []).filter((r) => r.rect).length}/7</Badge>
                                                </Button>
                                            </CollapsibleTrigger>
                                            <CollapsibleContent className="border-t-2 border-[var(--chalk)] px-3 py-3">
                                                <div className="flex flex-col gap-2">
                                                    {clickRegionRows(form?.clickRegions ?? []).map((cr) => (
                                                        <div key={cr.slotIndex}
                                                             className="flex items-center gap-3 border-2 border-[var(--chalk)] bg-[var(--slate)] p-2">
                                                            <Badge variant={cr.rect ? "default" : "outline"}
                                                                   className="shrink-0">
                                                                {cr.slotIndex + 1}
                                                            </Badge>
                                                            <span
                                                                className="flex-1 overflow-hidden text-ellipsis whitespace-nowrap font-mono text-xs text-[var(--zinc)]">
                                                                {formatRegion(cr.rect)}
                                                            </span>
                                                            <Input
                                                                className="w-20 border-2 border-[var(--chalk)] bg-[var(--carbon)] font-mono text-xs"
                                                                inputMode="numeric"
                                                                min="0"
                                                                value={cr.delayMs}
                                                                onChange={(e) => {
                                                                    const next = [...(form?.clickRegions ?? [])];
                                                                    next[cr.slotIndex] = {
                                                                        ...next[cr.slotIndex],
                                                                        delayMs: e.currentTarget.value
                                                                    };
                                                                    updateForm("clickRegions", next);
                                                                }}
                                                            />
                                                            <span className="text-xs text-[var(--dust)]">ms</span>
                                                            <Button
                                                                className="h-7 w-7 shrink-0 rounded-none px-0"
                                                                disabled={isBusy}
                                                                onClick={() => {
                                                                    const next = [...(form?.clickRegions ?? [])];
                                                                    next[cr.slotIndex] = {
                                                                        ...next[cr.slotIndex],
                                                                        rect: null
                                                                    };
                                                                    updateForm("clickRegions", next);
                                                                }}
                                                                type="button"
                                                                variant="ghost"
                                                            >
                                                                ×
                                                            </Button>
                                                        </div>
                                                    ))}
                                                    {(form?.clickRegions ?? []).filter((r) => r.rect).length < 7 && (
                                                        <Button
                                                            className="rounded-none"
                                                            disabled={isBusy}
                                                            onClick={() => {
                                                                const empty = (form?.clickRegions ?? []).findIndex((r) => !r.rect);
                                                                if (empty === -1) return;
                                                                void performSelectionSession([empty]);
                                                            }}
                                                            type="button"
                                                            variant="outline"
                                                        >
                                                            <RiLayoutGridLine data-icon="inline-start"/>
                                                            添加点击区域
                                                        </Button>
                                                    )}
                                                </div>
                                            </CollapsibleContent>
                                        </Collapsible>
                                    </div>
                                )}
                            </div>
                            <div className="space-y-3">
                                <div className="border-2 border-[var(--chalk)] p-3">
                                    <div className="flex items-center justify-between gap-2 mb-3">
                                        <span
                                            className="font-mono text-xs font-black tracking-[0.18em] uppercase">即时验证</span>
                                        <HelpHint content="聚焦输入框或按按钮执行一次仅识别测试。"/>
                                    </div>
                                    <Input
                                        className="border-2 border-[var(--chalk)] font-mono text-sm tracking-wider"
                                        onChange={(e) => setVerificationValue(e.currentTarget.value)}
                                        onFocus={() => void handleVerificationRun()}
                                        placeholder="聚焦此处执行测试验证"
                                        value={verificationValue}
                                    />
                                    <p className="mt-2 font-mono text-xs text-[var(--zinc)]">{verificationMessage}</p>
                                    <Button
                                        className="mt-2 w-full"
                                        disabled={verificationStatus === "running"}
                                        onClick={() => void handleVerificationRun()}
                                        type="button"
                                        variant="outline"
                                        size="sm"
                                    >
                                        <RiRefreshLine data-icon="inline-start"/>
                                        重新验证
                                    </Button>
                                </div>
                            </div>
                        </div>
                    </FieldUnit>
                )}

                {activeTab === "result" && (
                    <FieldUnit header="[ UNIT 03 ] 报码输出 — 审阅三码结果">
                        {hasLatestResult ? (
                            <div className="grid gap-4 md:grid-cols-2">
                                <div className="border-2 border-[var(--chalk)] bg-[var(--slate)] p-4">
                                    <div className="flex flex-wrap items-center gap-2 mb-4">
                                        <Badge
                                            variant={latestRun?.error ? "outline" : latestRun?.value ? "default" : "secondary"}>
                                            {latestRun?.error ? "失败" : latestRun?.value ? "成功" : "等待"}
                                        </Badge>
                                        {latestRun?.triggeredBy ?
                                            <Badge variant="outline">{latestRun.triggeredBy}</Badge> : null}
                                        {latestRun?.autoTyped ? <Badge variant="outline">已自动输入</Badge> : null}
                                    </div>
                                    <p className="font-mono text-xs font-black tracking-[0.18em] uppercase text-[var(--zinc)]">最新三码输出</p>
                                    <p className="mt-2 font-mono text-4xl font-semibold tracking-[0.24em] text-[var(--amber)]">
                                        {latestRun?.value ?? "---"}
                                    </p>
                                    <div className="mt-2 h-0.5 w-full bg-[var(--amber)]"/>
                                    <p className="mt-2 text-xs text-[var(--zinc)]">{latestRun?.error ?? "执行识别后会在这里显示最新三码输出。"}</p>
                                </div>
                                <div className="space-y-2">
                                    {runDetails.map((detail) => (
                                        <div key={detail.slot} className="border-2 border-[var(--chalk)] p-3">
                                            <div className="flex items-center justify-between gap-2">
                                                <span
                                                    className="font-mono text-xs font-black tracking-[0.18em] uppercase">{REGION_LABELS[detail.slot]}</span>
                                                <Badge
                                                    variant={detail.error ? "outline" : detail.digit ? "default" : "secondary"}>
                                                    {detail.error ? "失败" : detail.digit ?? "--"}
                                                </Badge>
                                            </div>
                                            <div className="mt-2 flex flex-wrap gap-2 text-xs text-[var(--zinc)]">
                                                <span className="font-mono">{detail.morse ?? "--"}</span>
                                                <span>{detail.thresholdMode}</span>
                                                <span>轮廓 {detail.contourCount}</span>
                                            </div>
                                        </div>
                                    ))}
                                </div>
                            </div>
                        ) : (
                            <EmptyState
                                icon={<RiCheckboxCircleLine/>}
                                title="等待报码"
                                description="完成前两单元后，三码结果会写入这里。"
                            />
                        )}
                    </FieldUnit>
                )}

                {activeTab === "history" && (
                    <FieldUnit header="[ UNIT 04 ] 运行档案 — 回看识别历史">
                        {history.length === 0 ? (
                            <EmptyState
                                icon={<RiHistoryLine/>}
                                title="暂无档案"
                                description="执行一次识别后会在这里生成运行回执。"
                            />
                        ) : (
                            <ScrollArea className="h-72">
                                <div className="flex flex-col gap-2 pe-4">
                                    {history.map((entry) => (
                                        <div key={entry.id} className="border-2 border-[var(--chalk)] p-3">
                                            <div
                                                className="flex flex-wrap items-center justify-between gap-2 border-b border-[var(--seam)] pb-2">
                                                <div className="flex flex-wrap items-center gap-2">
                          <span className="font-mono text-xs font-black tracking-[0.18em] uppercase">
                            {entry.result ? `报码 ${entry.result}` : "识别失败"}
                          </span>
                                                    <Badge
                                                        variant={entry.success ? "default" : "outline"}>{entry.success ? "成功" : "失败"}</Badge>
                                                    <Badge variant="outline">{entry.triggeredBy}</Badge>
                                                    {entry.autoTyped ?
                                                        <Badge variant="outline">已自动输入</Badge> : null}
                                                </div>
                                                <span
                                                    className="font-mono text-xs text-[var(--zinc)]">{formatTimestamp(entry.occurredAtMs)}</span>
                                            </div>
                                            <p className="mt-2 text-xs text-[var(--zinc)]">{entry.error ? "本轮识别失败，建议回查窗位与阈值。" : "识别链路执行完成，结果已写入历史档案。"}</p>
                                        </div>
                                    ))}
                                </div>
                            </ScrollArea>
                        )}
                    </FieldUnit>
                )}
            </div>
        </AppPage>
    );
}

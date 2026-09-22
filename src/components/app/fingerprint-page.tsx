import {startTransition, useCallback, useEffect, useMemo, useRef, useState} from "react";
import {invokeLogged as invoke} from "@/lib/logging";
import {RiDeleteBinLine, RiLayoutGridLine, RiPlayLine, RiRefreshLine} from "@remixicon/react";

import {FINGERPRINT_EVENTS} from "@/lib/tauri-events";
import {subscribeTauriEvent} from "@/lib/tauri-listener";
import {useNativeShell} from "@/hooks/use-native-shell";
import {useAutosave} from "@/hooks/use-autosave";
import {useBootstrapForm} from "@/hooks/use-bootstrap-form";
import {useHotkeyRecorder} from "@/hooks/use-hotkey-recorder";
import {getSettingsRevision} from "@/components/app/profile-utils";
import {useProfile} from "@/hooks/use-profile";

import {Badge} from "@/components/ui/badge";
import {Button} from "@/components/ui/button";
import {Collapsible, CollapsibleContent, CollapsibleTrigger} from "@/components/ui/collapsible";
import {Input} from "@/components/ui/input";
import {Switch} from "@/components/ui/switch";
import {
    ChannelTabs,
    ConfigRow,
    FieldUnit,
    HelpHint,
    SoftAlert,
    StampFold,
    ToolPageFrame,
} from "@/components/app/app-ui";
import {FingerprintRegionOverlay} from "@/components/app/fingerprint-overlay";
import {
    ARCHIVE_LABELS,
    AUTOSAVE_DELAY_MS,
    CANDIDATE_LABELS,
    type FingerprintBootstrap,
    type FingerprintPageProps,
    type FingerprintRunResult,
    type FingerprintSettings,
    type FingerprintSettingsForm,
    type LayoutTarget,
    type RegionSelectionOutcome,
    type RegionSelectionProgress,
} from "@/components/app/fingerprint-types";
import {
    archiveSlotsReady,
    clickRegionRows,
    formatRecordedHotkey,
    formatRegion,
    formatTimestamp,
    getErrorMessage,
    layoutReadyForRun,
    parseOverlaySlots,
    parseSettingsForm,
    personFingerprintCount,
    settingsToForm,
} from "@/components/app/fingerprint-utils";

export function FingerprintPage({overlayMode = false}: FingerprintPageProps) {
    const overlaySlots = useMemo(() => (overlayMode ? parseOverlaySlots() : []), [overlayMode]);
    const isNativeShell = useNativeShell();
    const {bootstrap: profileBootstrap} = useProfile();
    const hotkeyButtonRef = useRef<HTMLButtonElement>(null);
    const [running, setRunning] = useState(false);
    const [selecting, setSelecting] = useState(false);
    const [activeTab, setActiveTab] = useState("layout");
    const [newPersonName, setNewPersonName] = useState("");
    const [selectedPersonId, setSelectedPersonId] = useState<string | null>(null);

    const bf = useBootstrapForm<FingerprintBootstrap, FingerprintSettings, FingerprintSettingsForm>({
        spec: {
            getBootstrapCommand: "fingerprint_get_bootstrap",
            saveSettingsCommand: "fingerprint_save_settings",
            settingsToForm,
            parseSettingsForm,
        },
        isNativeShell,
        skipInitialLoad: overlayMode,
        loadStatusMessage: "正在加载指纹工具...",
        readyStatusMessage: "就绪。框完九宫格；名条只需框一次位置。",
        previewStatusMessage: "浏览器预览模式：当前仅验证布局，原生命令请在桌面端运行。",
        saveSuccessMessage: "设置已保存。",
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
        pageError,
        setPageError,
        setStatusMessage,
        autosaveVersionRef: autosaveRef,
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

    useAutosave<FingerprintSettingsForm>({
        form,
        isDirty,
        disabled: overlayMode || !isNativeShell || loading || !bootstrap || !form || recorder.isRecording || selecting,
        onSave: (formSnapshot, nextVersion) => saveSettings(parseSettingsForm(formSnapshot), nextVersion),
        onError: (message) => {
            setPageError(message);
            setStatusMessage(`保存失败：${message}`);
        },
        delay: AUTOSAVE_DELAY_MS,
        autosaveVersionRef: autosaveRef,
    });

    useEffect(() => {
        if (recorder.isRecording) hotkeyButtonRef.current?.focus();
    }, [recorder.isRecording]);

    useEffect(() => {
        if (overlayMode || !isNativeShell) return;
        let disposed = false;
        void invoke("fingerprint_set_hotkey_recording", {recording: recorder.isRecording}).catch((error) => {
            if (!disposed) {
                const message = getErrorMessage(error);
                setPageError(message);
                setStatusMessage(message);
            }
        });
        return () => {
            disposed = true;
        };
    }, [isNativeShell, overlayMode, recorder.isRecording, setPageError, setStatusMessage]);

    useEffect(() => {
        if (overlayMode || !isNativeShell) return;
        let disposed = false;
        const unlistenRun = subscribeTauriEvent<FingerprintRunResult>(FINGERPRINT_EVENTS.runFinished, async (event) => {
            if (disposed) return;
            const result = event.payload;
            startTransition(() => {
                setBootstrap((current) => (current ? {...current, latestRun: result} : current));
                setPageError(result.error);
                setStatusMessage(result.error ? `识别失败：${result.error}` : formatRun(result));
            });
            try {
                await syncBootstrap({syncMode: "none"});
            } catch (error) {
                if (!disposed) setPageError(getErrorMessage(error));
            }
        });
        const unlistenProgress = subscribeTauriEvent<RegionSelectionProgress>(FINGERPRINT_EVENTS.selectionProgress, (event) => {
            if (disposed) return;
            const progress = event.payload;
            startTransition(() => {
                setForm((current) => current ? applyProgress(current, progress) : current);
            });
        });
        const unlistenHotkey = subscribeTauriEvent<string>(FINGERPRINT_EVENTS.hotkeyError, (event) => {
            if (disposed) return;
            startTransition(() => {
                setBootstrap((current) => (current ? {...current, hotkeyError: event.payload} : current));
            });
        });
        return () => {
            disposed = true;
            unlistenRun();
            unlistenProgress();
            unlistenHotkey();
        };
    }, [isNativeShell, overlayMode, setBootstrap, setForm, setPageError, setStatusMessage, syncBootstrap]);

    const people = form?.people ?? [];
    const selectedPerson = people.find((person) => person.id === selectedPersonId) ?? people[0] ?? null;

    useEffect(() => {
        if (!selectedPersonId && people[0]) setSelectedPersonId(people[0].id);
    }, [people, selectedPersonId]);

    const layoutReady = form ? layoutReadyForRun(form) : false;
    const archiveReady = form ? archiveSlotsReady(form) : false;
    const canRun = layoutReady && people.some((person) => personFingerprintCount(person.fingerprintPaths) >= 4);
    const isBusy = loading || saving || running || selecting;

    const performSelection = useCallback(async (target: LayoutTarget, slots: number[]) => {
        if (!isNativeShell) {
            setStatusMessage("浏览器预览模式下不可框选，请在桌面端使用。");
            return;
        }
        setSelecting(true);
        setStatusMessage("请在悬浮层中框选区域。");
        try {
            const outcome = await invoke<RegionSelectionOutcome>("fingerprint_begin_region_selection", {slots, target});
            await syncBootstrap({syncMode: "full"});
            if (outcome.kind === "selected") {
                setStatusMessage("区域已更新。");
            } else if (outcome.kind === "cancelled") {
                setStatusMessage("区域选择已取消。");
            } else {
                setStatusMessage("区域选择窗口已关闭。");
            }
        } catch (error) {
            const message = getErrorMessage(error);
            setPageError(message);
            setStatusMessage(message);
        } finally {
            setSelecting(false);
        }
    }, [isNativeShell, setPageError, setStatusMessage, syncBootstrap]);

    const addPerson = useCallback(() => {
        const name = newPersonName.trim();
        if (!name) {
            setStatusMessage("先填人名。");
            return;
        }
        if (people.some((person) => person.name === name)) {
            setStatusMessage("已有这个角色。");
            return;
        }
        const id = crypto.randomUUID();
        updateForm("people", [
            ...people,
            {id, name, nameImagePath: "", fingerprintPaths: Array.from({length: 8}, () => null)},
        ]);
        setSelectedPersonId(id);
        setNewPersonName("");
        setStatusMessage(`已添加 ${name}。`);
    }, [newPersonName, people, setStatusMessage, updateForm]);

    const captureArchive = useCallback(async (slots: number[]) => {
        if (!selectedPerson) {
            setStatusMessage("先选角色。");
            return;
        }
        try {
            const next = await invoke<FingerprintBootstrap>("fingerprint_capture_archive", {
                personId: selectedPerson.id,
                slots,
                settingsRevision: getSettingsRevision(profileBootstrap),
            });
            setBootstrap(next);
            setForm(settingsToForm(next.settings));
            setStatusMessage(`已采集 ${selectedPerson.name} 档案 ${slots.map((slot) => slot + 1).join("、")}。`);
        } catch (error) {
            const message = getErrorMessage(error);
            setPageError(message);
            setStatusMessage(message);
        }
    }, [profileBootstrap, selectedPerson, setBootstrap, setForm, setPageError, setStatusMessage]);

    const deletePerson = useCallback(async () => {
        if (!selectedPerson) return;
        try {
            const next = await invoke<FingerprintBootstrap>("fingerprint_delete_person", {
                personId: selectedPerson.id,
                settingsRevision: getSettingsRevision(profileBootstrap),
            });
            setBootstrap(next);
            setForm(settingsToForm(next.settings));
            setSelectedPersonId(next.settings.people[0]?.id ?? null);
            setStatusMessage(`已删除 ${selectedPerson.name}。`);
        } catch (error) {
            const message = getErrorMessage(error);
            setPageError(message);
            setStatusMessage(message);
        }
    }, [profileBootstrap, selectedPerson, setBootstrap, setForm, setPageError, setStatusMessage]);

    const runOnce = useCallback(async (autoClick: boolean) => {
        if (!isNativeShell) {
            setStatusMessage("浏览器预览模式下不可识别。");
            return;
        }
        setRunning(true);
        try {
            const result = await invoke<FingerprintRunResult>("fingerprint_run", {autoClick});
            startTransition(() => {
                setBootstrap((current) => (current ? {...current, latestRun: result} : current));
                setPageError(result.error);
                setStatusMessage(result.error ? `识别失败：${result.error}` : formatRun(result));
            });
        } catch (error) {
            const message = getErrorMessage(error);
            setPageError(message);
            setStatusMessage(message);
        } finally {
            setRunning(false);
            try {
                await syncBootstrap({syncMode: "none"});
            } catch (error) {
                setPageError(getErrorMessage(error));
            }
        }
    }, [isNativeShell, setBootstrap, setPageError, setStatusMessage, syncBootstrap]);

    if (overlayMode) {
        return <FingerprintRegionOverlay slots={overlaySlots}/>;
    }

    const latestRun = bootstrap?.latestRun ?? null;

    return (
        <ToolPageFrame
            actions={
                <Badge variant={isBusy ? "outline" : bootstrap?.hotkeyError ? "outline" : canRun ? "default" : "ghost"}>
                    {isBusy ? "识别中" : bootstrap?.hotkeyError ? "快捷键异常" : canRun ? "就绪" : "未标定"}
                </Badge>
            }
            error={pageError || bootstrap?.hotkeyError ? <SoftAlert>{pageError ?? bootstrap?.hotkeyError}</SoftAlert> : undefined}
            title="指纹"
        >
            <div className="col-span-12">
                <ChannelTabs
                    tabs={[
                        {id: "layout", label: "布局", active: activeTab === "layout"},
                        {id: "library", label: "图库", active: activeTab === "library"},
                        {id: "run", label: "运行", active: activeTab === "run"},
                    ]}
                    onTabChange={setActiveTab}
                />
            </div>

            <div className="col-span-12">
                {activeTab === "layout" && (
                    <div className="grid gap-4">
                        <FieldUnit
                            header="名条位置（框一次）"
                            footer={
                                <Button className="w-full" disabled={isBusy} onClick={() => void performSelection("name", [0])} type="button">
                                    <RiRefreshLine data-icon="inline-start"/>
                                    框选名条
                                </Button>
                            }
                        >
                            <ConfigRow
                                label="用途"
                                value={<span className="text-xs text-base-content/60">只要框位置。识别时读这里的字认人，不要每个角色采集。</span>}
                            />
                            <ConfigRow
                                label="名条"
                                state={form?.nameRegion ? "valid" : "idle"}
                                value={<span className="font-mono text-xs text-base-content/60">{formatRegion(form?.nameRegion ?? null)}</span>}
                            />
                        </FieldUnit>
                        <FieldUnit
                            header="候选九宫格"
                            footer={
                                <Button className="w-full" disabled={isBusy} onClick={() => void performSelection("candidates", [0, 1, 2, 3, 4, 5, 6, 7, 8])} type="button">
                                    <RiRefreshLine data-icon="inline-start"/>
                                    一次框选 9 格
                                </Button>
                            }
                        >
                            <ConfigRow
                                label="用途"
                                value={<span className="text-xs text-base-content/60">热键只在这里认格、点数。框右边指纹块，含空位，不要框中间数字。</span>}
                            />
                            {CANDIDATE_LABELS.map((label, index) => {
                                const region = form?.candidateBoxes[index] ?? null;
                                return (
                                    <ConfigRow
                                        key={label}
                                        label={label}
                                        state={region ? "valid" : "idle"}
                                        value={
                                            <div className="flex items-center gap-2">
                                                <span className="font-mono text-xs text-base-content/60">{formatRegion(region)}</span>
                                                <Button disabled={isBusy} onClick={() => void performSelection("candidates", [index])} size="sm" type="button" variant="outline">
                                                    {region ? "重选" : "框选"}
                                                </Button>
                                            </div>
                                        }
                                    />
                                );
                            })}
                        </FieldUnit>
                    </div>
                )}

                {activeTab === "library" && (
                    <div className="grid gap-4">
                        <FieldUnit header="角色图库">
                        <ConfigRow
                            label="当前角色"
                            value={
                                <select
                                    className="select select-bordered select-sm w-full max-w-xs"
                                    onChange={(event) => setSelectedPersonId(event.currentTarget.value || null)}
                                    value={selectedPerson?.id ?? ""}
                                >
                                    {people.length === 0 ? <option value="">还没有角色</option> : null}
                                    {people.map((person) => (
                                        <option key={person.id} value={person.id}>
                                            {person.name} · {personFingerprintCount(person.fingerprintPaths)}/8
                                        </option>
                                    ))}
                                </select>
                            }
                        />
                        <ConfigRow
                            label="添加角色"
                            value={
                                <div className="flex w-full items-center gap-2">
                                    <Input
                                        className="max-w-xs"
                                        onChange={(event) => setNewPersonName(event.currentTarget.value)}
                                        placeholder="新角色名"
                                        value={newPersonName}
                                    />
                                    <Button disabled={isBusy} onClick={addPerson} type="button">
                                        添加
                                    </Button>
                                </div>
                            }
                        />
                        <ConfigRow
                            label="档案指纹"
                            value={
                                <div className="flex flex-wrap gap-2">
                                    <Button disabled={isBusy || !selectedPerson || !archiveReady} onClick={() => void captureArchive([0, 1, 2, 3])} type="button" variant="outline">
                                        采 1–4
                                    </Button>
                                    <Button disabled={isBusy || !selectedPerson || !archiveReady} onClick={() => void captureArchive([0, 1, 2, 3, 4, 5])} type="button" variant="outline">
                                        采 1–6
                                    </Button>
                                    <Button disabled={isBusy || !selectedPerson || !archiveReady} onClick={() => void captureArchive([0, 1, 2, 3, 4, 5, 6, 7])} type="button" variant="outline">
                                        采 1–8
                                    </Button>
                                    {ARCHIVE_LABELS.map((label, index) => (
                                        <Button
                                            disabled={isBusy || !selectedPerson || !archiveReady}
                                            key={label}
                                            onClick={() => void captureArchive([index])}
                                            size="sm"
                                            type="button"
                                            variant="ghost"
                                        >
                                            {index + 1}
                                        </Button>
                                    ))}
                                </div>
                            }
                        />
                        <ConfigRow
                            label="删除角色"
                            value={
                                <Button disabled={isBusy || !selectedPerson} onClick={() => void deletePerson()} type="button" variant="outline">
                                    <RiDeleteBinLine data-icon="inline-start"/>
                                    删除当前角色
                                </Button>
                            }
                        />
                        </FieldUnit>
                        <FieldUnit
                            header="档案槽 1–8"
                            footer={
                                <Button className="w-full" disabled={isBusy} onClick={() => void performSelection("archive", [0, 1, 2, 3, 4, 5, 6, 7])} type="button">
                                    <RiRefreshLine data-icon="inline-start"/>
                                    一次框选 8 槽
                                </Button>
                            }
                        >
                            <ConfigRow
                                label="用途"
                                value={<span className="text-xs text-base-content/60">只给上面采 1–X 用。热键识别不读这些槽。在角色档案页框，不要框解锁界面。</span>}
                            />
                            {ARCHIVE_LABELS.map((label, index) => {
                                const region = form?.archiveSlots[index] ?? null;
                                return (
                                    <ConfigRow
                                        key={label}
                                        label={label}
                                        state={region ? "valid" : "idle"}
                                        value={
                                            <div className="flex items-center gap-2">
                                                <span className="font-mono text-xs text-base-content/60">{formatRegion(region)}</span>
                                                <Button disabled={isBusy} onClick={() => void performSelection("archive", [index])} size="sm" type="button" variant="outline">
                                                    {region ? "重选" : "框选"}
                                                </Button>
                                            </div>
                                        }
                                    />
                                );
                            })}
                        </FieldUnit>
                    </div>
                )}

                {activeTab === "run" && (
                    <FieldUnit
                        header="识别"
                        footer={
                            <div className="flex gap-2">
                                <Button className="flex-1" disabled={isBusy || !canRun} onClick={() => void runOnce(false)} type="button" variant="outline">
                                    只识别
                                </Button>
                                <Button className="flex-1" disabled={isBusy || !canRun} onClick={() => void runOnce(true)} type="button">
                                    <RiPlayLine data-icon="inline-start"/>
                                    识别并点击
                                </Button>
                            </div>
                        }
                    >
                        <ConfigRow
                            label="热键"
                            state={form?.hotkey ? "valid" : "idle"}
                            value={
                                <Button
                                    ref={hotkeyButtonRef}
                                    className="h-auto w-full justify-between gap-4 border border-base-300 px-3 py-2 font-mono text-xs"
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
                        />
                        <div className="flex items-center gap-2 border-b border-base-300 px-3 py-2">
                            <label className="flex items-center gap-2">
                                <Switch
                                    checked={form?.autoClickEnabled ?? true}
                                    disabled={isBusy}
                                    onCheckedChange={(checked) => updateForm("autoClickEnabled", checked)}
                                />
                                <span className="font-mono text-xs font-semibold">自动点击链路</span>
                            </label>
                            <HelpHint content="识别成功后先点九宫格，再按顺序点配置区域，全部成功后按完成后按键。"/>
                        </div>
                        {form?.autoClickEnabled && (
                            <div className="space-y-3 px-3 pb-3">
                                <ConfigRow
                                    label="点击完成后按键"
                                    value={
                                        <Input
                                            className="border border-base-300 font-mono text-xs"
                                            placeholder="留空不执行，例如 F4"
                                            onChange={(event) => updateForm("afterClickHotkey", event.currentTarget.value)}
                                            value={form?.afterClickHotkey ?? ""}
                                        />
                                    }
                                    state={form?.afterClickHotkey ? "valid" : "idle"}
                                />
                                <Collapsible>
                                    <CollapsibleTrigger asChild>
                                        <StampFold
                                            label="点击区域配置"
                                            trailing={(
                                                <Badge variant="outline">{(form?.clickRegions ?? []).filter((region) => region.rect).length}/7</Badge>
                                            )}
                                        />
                                    </CollapsibleTrigger>
                                    <CollapsibleContent className="border-t-2 border-base-content px-3 py-3">
                                        <div className="flex flex-col gap-2">
                                            {clickRegionRows(form?.clickRegions ?? []).map((region) => (
                                                <div key={region.slotIndex} className="flex items-center gap-3 border border-base-300 bg-base-200 p-2">
                                                    <Badge variant={region.rect ? "default" : "outline"} className="shrink-0">
                                                        {region.slotIndex + 1}
                                                    </Badge>
                                                    <span className="flex-1 overflow-hidden text-ellipsis whitespace-nowrap font-mono text-xs text-base-content/60">
                                                        {formatRegion(region.rect)}
                                                    </span>
                                                    <Input
                                                        className="w-20 border border-base-300 bg-base-100 font-mono text-xs"
                                                        inputMode="numeric"
                                                        min="0"
                                                        value={region.delayMs}
                                                        onChange={(event) => {
                                                            const next = [...(form?.clickRegions ?? [])];
                                                            next[region.slotIndex] = {
                                                                ...next[region.slotIndex],
                                                                delayMs: event.currentTarget.value,
                                                            };
                                                            updateForm("clickRegions", next);
                                                        }}
                                                    />
                                                    <span className="text-xs text-base-content/40">ms</span>
                                                    <Button
                                                        className="h-7 w-7 shrink-0 px-0"
                                                        disabled={isBusy}
                                                        onClick={() => {
                                                            const next = [...(form?.clickRegions ?? [])];
                                                            next[region.slotIndex] = {
                                                                ...next[region.slotIndex],
                                                                rect: null,
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
                                            {(form?.clickRegions ?? []).filter((region) => region.rect).length < 7 && (
                                                <Button
                                                    className="rounded-none"
                                                    disabled={isBusy}
                                                    onClick={() => {
                                                        const empty = (form?.clickRegions ?? []).findIndex((region) => !region.rect);
                                                        if (empty === -1) return;
                                                        void performSelection("click", [empty]);
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
                        <ConfigRow
                            label="占用阈值"
                            value={
                                <div className="flex items-center gap-2">
                                    <Input
                                        className="font-mono text-xs"
                                        inputMode="decimal"
                                        onChange={(event) => updateForm("occupancyThreshold", event.currentTarget.value)}
                                        value={form?.occupancyThreshold ?? ""}
                                    />
                                    <HelpHint content="占用看格子中心纹路，空槽边框不计入。此值暂不参与。"/>
                                </div>
                            }
                        />
                        <ConfigRow
                            label="匹配阈值"
                            value={
                                <Input
                                    className="font-mono text-xs"
                                    inputMode="decimal"
                                    onChange={(event) => updateForm("matchThreshold", event.currentTarget.value)}
                                    value={form?.matchThreshold ?? ""}
                                />
                            }
                        />
                        <ConfigRow
                            label="点击间隔 ms"
                            value={
                                <Input
                                    className="font-mono text-xs"
                                    inputMode="numeric"
                                    onChange={(event) => updateForm("clickDelayMs", event.currentTarget.value)}
                                    value={form?.clickDelayMs ?? ""}
                                />
                            }
                        />
                        {latestRun ? (
                            <ConfigRow
                                label="最近一次"
                                value={
                                    <div className="text-xs">
                                        <p>{formatTimestamp(latestRun.occurredAtMs)} · {formatRun(latestRun)}</p>
                                        {latestRun.matches.length > 0 ? (
                                            <p className="mt-1 font-mono text-base-content/60">
                                                {latestRun.matches.map((item) => `${item.templateIndex}→格${item.candidateIndex}(${item.score.toFixed(2)})`).join(" · ")}
                                            </p>
                                        ) : null}
                                    </div>
                                }
                            />
                        ) : null}
                    </FieldUnit>
                )}
            </div>
        </ToolPageFrame>
    );
}

function formatRun(result: FingerprintRunResult): string {
    if (result.error) return result.error;
    const clicks = result.matches.map((item) => item.candidateIndex).join("→");
    return `${result.personName ?? "?"} · ${result.mode ?? "?"}档 · 点 ${clicks}${result.clicked ? " · 已点" : ""}`;
}

function applyProgress(form: FingerprintSettingsForm, progress: RegionSelectionProgress): FingerprintSettingsForm {
    if (progress.target === "name") {
        return {...form, nameRegion: progress.rects[0] ?? null};
    }
    if (progress.target === "candidates") {
        return {...form, candidateBoxes: padRects(progress.rects, 9)};
    }
    if (progress.target === "archive") {
        return {...form, archiveSlots: padRects(progress.rects, 8)};
    }
    if (progress.target === "click") {
        const padded = padRects(progress.rects, 7);
        return {
            ...form,
            clickRegions: padded.map((rect, index) => ({
                rect,
                delayMs: form.clickRegions[index]?.delayMs ?? "500",
            })),
        };
    }
    return form;
}

function padRects(rects: Array<import("@/components/app/fingerprint-types").RegionRect | null>, size: number) {
    const next = rects.slice(0, size);
    while (next.length < size) next.push(null);
    return next;
}

import {useCallback, useEffect, useMemo, useRef, useState} from "react";
import {invokeLogged as invoke} from "@/lib/logging";
import {open} from "@tauri-apps/plugin-dialog";
import {RECOGNITION_EVENTS} from "@/lib/tauri-events";
import {subscribeTauriEvent} from "@/lib/tauri-listener";
import {
    RiAddLine,
    RiArrowDownSLine,
    RiArrowRightSLine,
    RiDeleteBinLine,
} from "@remixicon/react";
import {toast} from "sonner";

import {Button} from "@/components/ui/button";
import {Input} from "@/components/ui/input";
import {Switch} from "@/components/ui/switch";
import {
    AddCardButton,
    FieldUnit,
    MasterSwitchCard,
    SoftAlert,
    ToolPageFrame,
} from "@/components/app/app-ui";
import {
    RecognitionCardEditor,
    RECOGNITION_HOTKEY_HELPER_TEXT,
    type RecognitionCardEditorAdapter,
    type RecognitionRecordingTarget,
} from "@/components/app/recognition-card-editor";
import {
    recognitionCardReducer,
    type RecognitionCardAction,
} from "@/components/app/recognition-card-reducer";
import type {
    RecognitionBootstrap,
    RecognitionCard,
    RecognitionGroup,
    RecognitionSettings,
    RecognitionSettingsForm,
} from "@/components/app/recognition-types";
import {RECOGNITION_AUTOSAVE_DELAY_MS} from "@/components/app/recognition-types";
import {
    createEmptyRecognitionCard,
    DEFAULT_RECOGNITION_GROUP_ID,
    mergeRecognitionWatchRegionsIntoForm,
    parseSettingsForm,
    rgbToHex,
    settingsToForm
} from "@/components/app/recognition-utils";
import {formatRecordedHotkey, getErrorMessage} from "@/components/app/morse-utils";
import {useNativeShell} from "@/hooks/use-native-shell";
import {useBootstrapForm} from "@/hooks/use-bootstrap-form";
import {useAutosave} from "@/hooks/use-autosave";
import {useHotkeyRecorder} from "@/hooks/use-hotkey-recorder";
import {useGlobalEnabled} from "@/hooks/use-global-enabled";

export {RecognitionRegionOverlay} from "@/components/app/recognition-overlay";

const RECOGNITION_BOOTSTRAP_SPEC = {
    getBootstrapCommand: "recognition_get_bootstrap",
    saveSettingsCommand: "recognition_save_settings",
    settingsToForm,
    parseSettingsForm,
};

export {RECOGNITION_HOTKEY_HELPER_TEXT};

export function getRecognitionGlobalStatusMessage(globalEnabled: boolean): string | null {
    return globalEnabled ? null : "全局开关关闭，识别触发不会响应。";
}

export function patchHotkeyEffectStep(
    card: RecognitionSettingsForm["cards"][number],
    hotkey: string,
    stepIndex = 0,
): Pick<RecognitionSettingsForm["cards"][number], "effectHotkey" | "hotkeyEffectSteps"> {
    const steps = card.hotkeyEffectSteps?.length
        ? card.hotkeyEffectSteps
        : [{hotkey: card.effectHotkey ?? "", delayMs: "0"}];
    const targetIndex = Math.min(Math.max(stepIndex, 0), steps.length - 1);
    const hotkeyEffectSteps = steps.map((step, index) => index === targetIndex ? {...step, hotkey} : step);
    return {
        effectHotkey: hotkeyEffectSteps[0]?.hotkey ?? "",
        hotkeyEffectSteps,
    };
}

export function cardsForGroup(form: RecognitionSettingsForm, groupId: string) {
    return form.cards
        .map((card, index) => ({card, index}))
        .filter((item) => (item.card.groupId ?? DEFAULT_RECOGNITION_GROUP_ID) === groupId)
        .sort((a, b) => (a.card.order ?? 0) - (b.card.order ?? 0));
}

export function reorderCardsWithinGroup(
    cards: RecognitionSettingsForm["cards"],
    groupId: string,
    cardId: string,
    delta: -1 | 1,
): RecognitionSettingsForm["cards"] {
    const groupCards = cards
        .filter((card) => (card.groupId ?? DEFAULT_RECOGNITION_GROUP_ID) === groupId)
        .sort((a, b) => (a.order ?? 0) - (b.order ?? 0));
    const index = groupCards.findIndex((card) => card.id === cardId);
    const nextIndex = index + delta;
    if (index < 0 || nextIndex < 0 || nextIndex >= groupCards.length) {
        return cards;
    }
    const reordered = [...groupCards];
    [reordered[index], reordered[nextIndex]] = [reordered[nextIndex], reordered[index]];
    const orderById = new Map(reordered.map((card, order) => [card.id, order]));
    return cards.map((card) =>
        (card.groupId ?? DEFAULT_RECOGNITION_GROUP_ID) === groupId
            ? {...card, order: orderById.get(card.id) ?? card.order}
            : card,
    );
}

function normalizedGroupId(groupId: string | null | undefined): string {
    return groupId?.trim() || DEFAULT_RECOGNITION_GROUP_ID;
}

function normalizeOrdersForGroups(
    cards: RecognitionSettingsForm["cards"],
    groupIds: Set<string>,
): RecognitionSettingsForm["cards"] {
    const orderById = new Map<string, number>();
    for (const groupId of groupIds) {
        cards
            .filter((card) => normalizedGroupId(card.groupId) === groupId)
            .sort((a, b) => (a.order ?? 0) - (b.order ?? 0))
            .forEach((card, order) => orderById.set(card.id, order));
    }
    return cards.map((card) =>
        orderById.has(card.id) ? {...card, order: orderById.get(card.id)} : card,
    );
}

export function moveCardToGroup(
    cards: RecognitionSettingsForm["cards"],
    cardId: string,
    targetGroupId: string | null,
): RecognitionSettingsForm["cards"] {
    const movingCard = cards.find((card) => card.id === cardId);
    if (!movingCard) return cards;

    const sourceGroupId = normalizedGroupId(movingCard.groupId);
    const nextGroupId = normalizedGroupId(targetGroupId);
    if (sourceGroupId === nextGroupId) return cards;

    const targetOrder = cards.filter((card) =>
        card.id !== cardId && normalizedGroupId(card.groupId) === nextGroupId,
    ).length;
    const nextCards = cards.map((card) =>
        card.id === cardId ? {...card, groupId: nextGroupId, order: targetOrder} : card,
    );
    return normalizeOrdersForGroups(nextCards, new Set([sourceGroupId, nextGroupId]));
}

export function patchRecognitionGroup(
    groups: RecognitionGroup[],
    groupId: string,
    patch: Partial<RecognitionGroup>,
): RecognitionGroup[] {
    return groups.map((group) => group.id === groupId ? {...group, ...patch} : group);
}

export function RecognitionPage() {
    const isNativeShell = useNativeShell();
    return <RecognitionWorkbench isNativeShell={isNativeShell}/>;
}

function RecognitionWorkbench({isNativeShell}: { isNativeShell: boolean }) {
    const {globalEnabled} = useGlobalEnabled();
    const globalStatusMessage = getRecognitionGlobalStatusMessage(globalEnabled);
    const globalStatusRef = useRef(globalStatusMessage);
    globalStatusRef.current = globalStatusMessage;
    const {
        form,
        setForm,
        setBootstrap,
        isDirty,
        updateForm,
        saveSettings,
        loading,
        pageError,
        setPageError,
        setStatusMessage,
        autosaveVersionRef,
    } = useBootstrapForm<RecognitionBootstrap, RecognitionSettings, RecognitionSettingsForm>({
        spec: RECOGNITION_BOOTSTRAP_SPEC,
        isNativeShell,
        loadStatusMessage: "正在加载识别触发设置...",
        readyStatusMessage: "识别触发模块就绪。",
    });
    const formRef = useRef(form);
    formRef.current = form;
    const saveSettingsRef = useRef(saveSettings);
    saveSettingsRef.current = saveSettings;

    const [recordingTarget, setRecordingTarget] = useState<RecognitionRecordingTarget>(null);
    const recordingTargetRef = useRef<RecognitionRecordingTarget>(null);
    recordingTargetRef.current = recordingTarget;

    const dispatchCardAction = useCallback((action: RecognitionCardAction) => {
        setForm((current) => current ? {
            ...current,
            cards: recognitionCardReducer(current.cards, action),
        } : current);
    }, [setForm]);

    const updateCardById = useCallback((cardId: string, patch: Partial<RecognitionSettingsForm["cards"][number]>) => {
        dispatchCardAction({type: "patch", cardId, patch});
    }, [dispatchCardAction]);

    const updateEffectHotkeyById = useCallback((cardId: string, hotkey: string, stepIndex = 0) => {
        dispatchCardAction({
            type: "update",
            cardId,
            update: (card) => ({
                ...card,
                ...patchHotkeyEffectStep(card, hotkey, stepIndex),
            }),
        });
    }, [dispatchCardAction]);

    const recorder = useHotkeyRecorder({
        formatKey: formatRecordedHotkey,
        onCommit: (key) => {
            const target = recordingTargetRef.current;
            if (!target) return;
            setRecordingTarget(null);
            if (target.field === "effectHotkey") {
                updateEffectHotkeyById(target.cardId, key, target.stepIndex);
                return;
            }
            const patch = target.field === "triggerHotkey"
                ? {hotkey: key}
                : {activationHotkey: key};
            updateCardById(target.cardId, patch);
        },
        onCancel: (draft) => {
            const target = recordingTargetRef.current;
            if (!target) return;
            setRecordingTarget(null);
            if (target.field === "effectHotkey") {
                updateEffectHotkeyById(target.cardId, draft, target.stepIndex);
                return;
            }
            const patch = target.field === "triggerHotkey"
                ? {hotkey: draft}
                : {activationHotkey: draft};
            updateCardById(target.cardId, patch);
        },
        onStatusMessage: setStatusMessage,
        keyRecordedMessage: (key) => `新的快捷键已录制：${key}`,
        recordingCancelledMessage: "已取消快捷键录制。",
    });
    const recorderRef = useRef(recorder);
    recorderRef.current = recorder;

    useEffect(() => {
        if (!isNativeShell) return;
        void invoke("recognition_set_hotkey_recording", {recording: !!recordingTarget}).catch((error) => {
            toast.error(getErrorMessage(error));
        });
    }, [isNativeShell, recordingTarget]);

    useAutosave<RecognitionSettingsForm>({
        form,
        isDirty,
        disabled: !isNativeShell || loading || !form || !!recordingTarget,
        onSave: (formSnapshot, nextVersion) => saveSettings(parseSettingsForm(formSnapshot), nextVersion),
        onError: (message) => {
            toast.error(`保存失败：${message}`);
        },
        delay: RECOGNITION_AUTOSAVE_DELAY_MS,
        autosaveVersionRef,
    });

    useEffect(() => {
        if (!isNativeShell) return;
        let disposed = false;

        const unlistenStateChanged = subscribeTauriEvent<RecognitionBootstrap>(RECOGNITION_EVENTS.stateChanged, (event) => {
            if (disposed) return;
            const next = event.payload;
            setBootstrap(next);
            setForm((current) => mergeRecognitionWatchRegionsIntoForm(current, next.settings));
            setPageError(null);
        });

        const unlistenHotkeyError = subscribeTauriEvent<string>(RECOGNITION_EVENTS.hotkeyError, (event) => {
            if (disposed) return;
            setPageError(event.payload);
            setStatusMessage(event.payload);
            toast.error(event.payload);
        });

        const unlistenHotkeyTriggered = subscribeTauriEvent<string>(RECOGNITION_EVENTS.hotkeyTriggered, (event) => {
            if (disposed) return;
            toast.info(`快捷键触发：卡片 ${event.payload}`);
            setStatusMessage(`快捷键触发：卡片 ${event.payload}`);
        });

        const unlistenRegionMatched = subscribeTauriEvent<string>(RECOGNITION_EVENTS.regionMatched, (event) => {
            if (disposed) return;
            toast.info(`区域匹配触发：卡片 ${event.payload}`);
            setStatusMessage(`区域匹配触发：卡片 ${event.payload}`);
        });

        return () => {
            disposed = true;
            unlistenStateChanged();
            unlistenHotkeyError();
            unlistenHotkeyTriggered();
            unlistenRegionMatched();
        };
    }, [isNativeShell, setBootstrap, setForm, setPageError, setStatusMessage]);

    const handleAddCard = useCallback(() => {
        dispatchCardAction({
            type: "transform",
            update: (cards) => [...cards, cardToForm(createEmptyRecognitionCard())],
        });
    }, [dispatchCardAction]);

    const updateGroupById = useCallback((groupId: string, patch: Partial<RecognitionGroup>) => {
        setForm((current) => {
            if (!current) return current;
            return {
                ...current,
                cardGroups: patchRecognitionGroup(current.cardGroups ?? [], groupId, patch),
            };
        });
    }, [setForm]);

    const moveCardWithinGroup = useCallback((groupId: string, cardId: string, delta: -1 | 1) => {
        dispatchCardAction({
            type: "transform",
            update: (cards) => reorderCardsWithinGroup(cards, groupId, cardId, delta),
        });
    }, [dispatchCardAction]);

    const moveCardToGroupId = useCallback((cardId: string, groupId: string | null) => {
        dispatchCardAction({
            type: "transform",
            update: (cards) => moveCardToGroup(cards, cardId, groupId),
        });
    }, [dispatchCardAction]);

    const addRecognitionGroup = useCallback(() => {
        setForm((current) => {
            if (!current) return current;
            const groups = current.cardGroups ?? [];
            const id = `recognition-group-${Date.now().toString(36)}`;
            return {
                ...current,
                cardGroups: [
                    ...groups,
                    {id, name: "新分组", order: groups.length, collapsed: false, enabled: true},
                ],
            };
        });
    }, [setForm]);

    const removeEmptyRecognitionGroup = useCallback((groupId: string) => {
        setForm((current) => {
            if (!current) return current;
            if (groupId === DEFAULT_RECOGNITION_GROUP_ID) return current;
            if (current.cards.some((card) => (card.groupId ?? DEFAULT_RECOGNITION_GROUP_ID) === groupId)) {
                return current;
            }
            return {
                ...current,
                cardGroups: (current.cardGroups ?? []).filter((group) => group.id !== groupId),
            };
        });
    }, [setForm]);

    const beginHotkeyRecording = useCallback((
        card: RecognitionSettingsForm["cards"][number],
        field: NonNullable<RecognitionRecordingTarget>["field"],
        stepIndex = 0,
    ) => {
        const currentValue = field === "triggerHotkey"
                ? card.hotkey
                : field === "activationHotkey"
                    ? card.activationHotkey ?? ""
                    : card.hotkeyEffectSteps?.[stepIndex]?.hotkey ?? card.effectHotkey ?? "";
        setRecordingTarget({cardId: card.id, field, stepIndex: field === "effectHotkey" ? stepIndex : undefined});
        recorderRef.current.beginRecording(currentValue);
        setStatusMessage(`正在录制 ${card.name || "识别卡片"} 的快捷键，按下主键会保存；失焦会取消。`);
    }, [setStatusMessage]);

    const handleHotkeyRecorderKeyDown = useCallback((
        card: RecognitionSettingsForm["cards"][number],
        field: NonNullable<RecognitionRecordingTarget>["field"],
        stepIndex: number | undefined,
        event: React.KeyboardEvent<HTMLButtonElement>,
    ) => {
        const target = recordingTargetRef.current;
        if (target?.cardId !== card.id || target.field !== field) {
            return;
        }
        if (field === "effectHotkey" && target.stepIndex !== stepIndex) {
            return;
        }
        recorderRef.current.handleKeyDown(event);
    }, []);

    const handleHotkeyRecorderBlur = useCallback(() => recorderRef.current.handleBlur(), []);

    // 操作前通过同一 queue 强制保存当前 form，避免测试命令读到 autosave 尚未落地的旧设置。
    const flushSettings = useCallback(async (): Promise<void> => {
        const currentForm = formRef.current;
        if (!isNativeShell || !currentForm) return;
        try {
            await saveSettingsRef.current(parseSettingsForm(currentForm));
        } catch (error) {
            const message = getErrorMessage(error);
            toast.error(`保存设置失败：${message}`);
            // 用带标记的 Error 抛出，调用方通过 name 判定避免重复弹窗
            const wrapped = new Error(message);
            wrapped.name = "FlushSettingsError";
            throw wrapped;
        }
    }, [isNativeShell]);

    const handleTestPlay = useCallback(
        async (cardId: string) => {
            if (!isNativeShell) return;
            try {
                await flushSettings();
                await invoke("recognition_test_play", {cardId});
                const message = globalStatusRef.current ?? "播放测试已触发";
                setStatusMessage(message);
                if (globalStatusRef.current) {
                    toast.info(message);
                } else {
                    toast.success(message);
                }
            } catch (error) {
                if (error instanceof Error && error.name === "FlushSettingsError") return;
                toast.error(getErrorMessage(error));
            }
        },
        [isNativeShell, flushSettings, setStatusMessage],
    );

    const handleTestMatch = useCallback(
        async (cardId: string) => {
            if (!isNativeShell) return;
            try {
                await flushSettings();
                type TestMatchResult = { similarity: number; triggered: boolean; matchPosition: { x: number; y: number } | null };
                const result = await invoke<TestMatchResult>("recognition_test_match", {cardId});
                const pos = result.matchPosition ? ` (位置: ${result.matchPosition.x}, ${result.matchPosition.y})` : "";
                toast.success(
                    `匹配度: ${(result.similarity * 100).toFixed(1)}% ${result.triggered ? "(已触发)" : "(未触发)"}${pos}`
                );
                if (globalStatusRef.current) {
                    setStatusMessage(globalStatusRef.current);
                }
            } catch (error) {
                if (error instanceof Error && error.name === "FlushSettingsError") return;
                toast.error(getErrorMessage(error));
            }
        },
        [isNativeShell, flushSettings, setStatusMessage],
    );

    const handleBeginRegionSelection = useCallback(
        async (cardId: string, probeIndex?: number, selectionTarget?: string) => {
            if (!isNativeShell) return;
            try {
                await flushSettings();
                await invoke("recognition_begin_region_selection", {cardId, probeIndex, selectionTarget});
            } catch (error) {
                if (error instanceof Error && error.name === "FlushSettingsError") return;
                toast.error(getErrorMessage(error));
            }
        },
        [isNativeShell, flushSettings],
    );

    const handleTestColorMatch = useCallback(
        async (cardId: string) => {
            if (!isNativeShell) return;
            type ColorTargetResult = {
                matched: boolean;
                targetColor: [number, number, number];
                tolerance: number;
                sampledColor: [number, number, number];
                distance: number;
                matchingPixelCount?: number;
            };
            type ColorProbeResult = {
                matched: boolean;
                sampledColor: [number, number, number];
                distance: number;
                targetColor: [number, number, number];
                tolerance: number;
                matchingPixelCount?: number;
                targets: ColorTargetResult[];
            };
            type ColorTestResult = {
                triggered: boolean;
                hitCount: number;
                totalCount: number;
                probes: ColorProbeResult[];
            };
            try {
                await flushSettings();
                const result = await invoke<ColorTestResult>("recognition_test_color_match", {cardId});
                const detail = result.probes
                    .map((p, i) => {
                        const sample = p.sampledColor.map((v) => v.toString(16).padStart(2, "0")).join("");
                        const count = p.matchingPixelCount && p.matchingPixelCount > 0 ? ` 命中${p.matchingPixelCount}px` : "";
                        return `#${i + 1}: ${p.matched ? "命中" : "未中"} (采样 #${sample} 距离 ${p.distance.toFixed(1)}${count})`;
                    })
                    .join("\n");
                const summary = `识色: ${result.hitCount}/${result.totalCount} 命中 ${result.triggered ? "(已触发)" : "(未触发)"}`;
                toast.success(`${summary}\n${detail}`, {duration: 6000});
                if (globalStatusRef.current) {
                    setStatusMessage(globalStatusRef.current);
                }
            } catch (error) {
                if (error instanceof Error && error.name === "FlushSettingsError") return;
                toast.error(getErrorMessage(error));
            }
        },
        [isNativeShell, flushSettings, setStatusMessage],
    );

    const handleLoadReferencePreview = useCallback(
        async (referenceImagePath: string): Promise<string | null> => {
            if (!isNativeShell) return null;
            try {
                const dataUrl = await invoke<string>("recognition_read_reference_image", {
                    referenceImagePath,
                });
                return dataUrl;
            } catch {
                return null;
            }
        },
        [isNativeShell],
    );

    const handlePickReferenceImages = useCallback(
        async (cardId: string) => {
            if (!isNativeShell) return;
            try {
                const picked = await open({
                    multiple: true,
                    directory: false,
                    filters: [{name: "图像文件", extensions: ["png", "jpg", "jpeg", "webp", "bmp", "gif", "tiff"]}],
                });
                if (!picked) return;
                const paths = Array.isArray(picked) ? picked : [picked];
                dispatchCardAction({
                    type: "update",
                    cardId,
                    update: (card) => {
                        const existing = card.watchReferenceImagePaths
                            ?? (card.watchReferenceImagePath ? [card.watchReferenceImagePath] : []);
                        return {...card, watchReferenceImagePaths: [...new Set([...existing, ...paths])]};
                    },
                });
            } catch (error) {
                toast.error(getErrorMessage(error));
            }
        },
        [dispatchCardAction, isNativeShell],
    );

    const handlePickAudioFile = useCallback(
        async (cardId: string) => {
            if (!isNativeShell) return;
            try {
                const picked = await open({
                    multiple: true,
                    directory: false,
                    filters: [{name: "音频文件", extensions: ["mp3", "wav", "ogg", "flac", "aac", "m4a", "wma"]}],
                });
                if (!picked) return;
                const newPaths = Array.isArray(picked) ? picked.filter((p): p is string => typeof p === "string") : [picked];
                if (newPaths.length === 0) return;
                dispatchCardAction({
                    type: "update",
                    cardId,
                    update: (card) => ({...card, audioFiles: [...(card.audioFiles ?? []), ...newPaths]}),
                });
            } catch (error) {
                toast.error(getErrorMessage(error));
            }
        },
        [dispatchCardAction, isNativeShell],
    );

    const cardEditorAdapter = useMemo<RecognitionCardEditorAdapter>(() => ({
        moveToGroup: moveCardToGroupId,
        moveWithinGroup: moveCardWithinGroup,
        testPlay: handleTestPlay,
        testMatch: handleTestMatch,
        beginRegionSelection: handleBeginRegionSelection,
        pickReferenceImages: handlePickReferenceImages,
        pickAudioFile: handlePickAudioFile,
        loadReferencePreview: handleLoadReferencePreview,
        testColorMatch: handleTestColorMatch,
        beginHotkeyRecording,
        hotkeyKeyDown: handleHotkeyRecorderKeyDown,
        hotkeyRecorderBlur: handleHotkeyRecorderBlur,
    }), [
        beginHotkeyRecording,
        handleBeginRegionSelection,
        handleHotkeyRecorderBlur,
        handleHotkeyRecorderKeyDown,
        handleLoadReferencePreview,
        handlePickAudioFile,
        handlePickReferenceImages,
        handleTestColorMatch,
        handleTestMatch,
        handleTestPlay,
        moveCardToGroupId,
        moveCardWithinGroup,
    ]);

    const enabled = form?.recognitionEnabled ?? form?.audioEnabled ?? false;

    const cardGroups = form?.cardGroups ?? [];

    return (
        <ToolPageFrame
            error={pageError ? <SoftAlert className="py-2 text-xs">{pageError}</SoftAlert> : undefined}
            title="识别触发"
        >
            {globalStatusMessage && (
                <div
                    className="col-span-12 border border-warning bg-warning/10 px-3 py-2 text-xs font-semibold text-warning">
                    {globalStatusMessage}
                </div>
            )}

            <MasterSwitchCard
                ariaLabel="识别触发总开关"
                checked={enabled}
                label={
                    <span className="font-mono text-xs font-semibold text-base-content">
                        {enabled ? "已启用" : "已禁用"}
                    </span>
                }
                onCheckedChange={(checked) => {
                    updateForm("recognitionEnabled", checked);
                    updateForm("audioEnabled", checked);
                }}
            />

            <div className="col-span-12 mt-3 flex justify-end">
                <Button
                    variant="outline"
                    size="sm"
                    disabled={!isNativeShell || loading}
                    onClick={addRecognitionGroup}
                    data-icon="inline-start"
                >
                    <RiAddLine className="size-4" aria-hidden="true"/>
                    新分组
                </Button>
            </div>

            <section className="@container col-span-12 grid min-h-0 gap-3 @xl:grid-cols-2">
                {form && cardGroups.map((group) => {
                    const groupCards = cardsForGroup(form, group.id);
                    return (
                        <div key={group.id} className="contents">
                            <FieldUnit
                                className="@xl:col-span-2"
                                padBody={false}
                                header={(
                                    <Input
                                        className="h-auto w-40 border-0 bg-transparent p-0 font-mono text-xs font-semibold"
                                        value={group.name}
                                        onChange={(event) => updateGroupById(group.id, {name: event.target.value})}
                                        aria-label="分组名称"
                                    />
                                )}
                                description={`${groupCards.length} 卡片`}
                                headerActions={(
                                    <>
                                        <Button
                                            variant="outline"
                                            size="icon-sm"
                                            onClick={() => updateGroupById(group.id, {collapsed: !group.collapsed})}
                                            aria-label={`${group.collapsed ? "展开" : "折叠"}分组 ${group.name}`}
                                        >
                                            {group.collapsed
                                                ? <RiArrowRightSLine className="size-4" aria-hidden="true"/>
                                                : <RiArrowDownSLine className="size-4" aria-hidden="true"/>}
                                        </Button>
                                        <Switch
                                            checked={group.enabled ?? true}
                                            onCheckedChange={(checked) => updateGroupById(group.id, {enabled: checked})}
                                            aria-label={`${group.name} 分组开关`}
                                        />
                                        {group.id !== DEFAULT_RECOGNITION_GROUP_ID && groupCards.length === 0 && (
                                            <Button
                                                variant="outline"
                                                size="icon-sm"
                                                onClick={() => removeEmptyRecognitionGroup(group.id)}
                                                aria-label="删除空分组"
                                            >
                                                <RiDeleteBinLine className="size-4 text-error" aria-hidden="true"/>
                                            </Button>
                                        )}
                                    </>
                                )}
                            />
                            {groupCards.map(({card, index}, position) => (
                                <RecognitionCardEditor
                                    key={card.id}
                                    card={card}
                                    index={index}
                                    position={position}
                                    groupSize={groupCards.length}
                                    cardGroups={cardGroups}
                                    collapsed={group.collapsed}
                                    isNativeShell={isNativeShell}
                                    dispatch={dispatchCardAction}
                                    adapter={cardEditorAdapter}
                                    recordingTarget={recordingTarget}
                                />
                            ))}
                        </div>
                    );
                })}
                <AddCardButton
                    className="min-h-36"
                    disabled={!isNativeShell || loading}
                    title="新增识别卡片"
                    description="添加新的快捷键、区域监听或识色触发卡片。"
                    onClick={handleAddCard}
                />
            </section>
        </ToolPageFrame>
    );
}


function cardToForm(card: RecognitionCard): RecognitionSettingsForm["cards"][number] {
    const audio = card.effects?.audio ?? (card.audioFiles && card.audioFiles.length > 0 ? {
        audioFiles: card.audioFiles,
        playMode: card.playMode ?? "single",
        comboWindowMs: card.comboWindowMs ?? 60000,
        comboWindows: card.comboWindows ?? [],
        volume: card.volume ?? 0.8,
        allowSimultaneous: card.allowSimultaneous ?? false,
    } : null);
    const activation = card.activation ?? {mode: "always", hotkey: null, durationMs: 10000};
    const click = card.effects?.click ?? null;
    return {
        id: card.id,
        name: card.name,
        enabled: card.enabled,
        triggerMode: card.triggerMode,
        hotkey: card.hotkey ?? "",
        watchRegion: card.watchRegion,
        watchReferenceImagePaths: card.watchReferenceImagePaths?.length
            ? card.watchReferenceImagePaths
            : card.watchReferenceImagePath?.trim()
                ? [card.watchReferenceImagePath.trim()]
                : [],
        watchMatchThreshold: String(card.watchMatchThreshold),
        watchPollIntervalMs: String(card.watchPollIntervalMs),
        retriggerAfterDisappear: card.retriggerAfterDisappear ?? false,
        activationMode: activation.mode,
        activationHotkey: activation.hotkey ?? "",
        activationDurationMs: String(activation.durationMs),
        audioEffectEnabled: Boolean(audio),
        hotkeyEffectEnabled: Boolean(card.effects?.hotkey),
        clickEffectEnabled: Boolean(click),
        effectHotkey: card.effects?.hotkey?.hotkey ?? "",
        clickMode: click?.mode ?? "customRegion",
        clickCustomRegion: click?.customRegion ?? null,
        clickColorProbeIndex: click?.colorProbeIndex == null ? "" : String(click.colorProbeIndex),
        audioFiles: audio?.audioFiles ?? [],
        playMode: audio?.playMode ?? "single",
        comboWindowMs: String(audio?.comboWindowMs ?? 60000),
        comboWindows: (audio?.audioFiles ?? []).map((_, i) => String((audio?.comboWindows ?? [])[i] ?? audio?.comboWindowMs ?? 60000)),
        volume: String(audio?.volume ?? 0.8),
        cooldownMs: String(card.cooldownMs),
        allowSimultaneous: audio?.allowSimultaneous ?? false,
        colorProbes: (card.colorProbes ?? []).map((p) => ({
            region: p.region,
            targets: (p.targets ?? []).map((t) => ({
                color: rgbToHex(t.color),
                tolerance: String(t.tolerance),
            })),
            probeMatchMode: p.probeMatchMode ?? "any",
        })),
        colorMatchMode: card.colorMatchMode ?? "all",
        colorMatchMethod: card.colorMatchMethod ?? "average",
    };
}

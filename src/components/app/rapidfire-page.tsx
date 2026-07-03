import type React from "react";
import {useCallback, useEffect, useMemo, useRef, useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import {listen} from "@tauri-apps/api/event";
import {RAPIDFIRE_EVENTS} from "@/lib/tauri-events";
import {
  RiAddLine,
  RiArrowDownSLine,
  RiArrowUpLine,
  RiDeleteBinLine,
  RiKeyboardLine,
  RiMapPinLine,
  RiPulseLine,
  RiStarFill,
  RiStarLine,
  RiStopLine,
} from "@remixicon/react";
import {toast} from "sonner";

import {Badge} from "@/components/ui/badge";
import {Button} from "@/components/ui/button";
import {CardHeader} from "@/components/ui/card";
import {Collapsible, CollapsibleContent, CollapsibleTrigger} from "@/components/ui/collapsible";
import {Field, FieldContent, FieldDescription, FieldGroup, FieldLabel, FieldTitle} from "@/components/ui/field";
import {Input} from "@/components/ui/input";
import {Kbd} from "@/components/ui/kbd";
import {Select, SelectContent, SelectItem, SelectTrigger, SelectValue} from "@/components/ui/select";
import {Switch} from "@/components/ui/switch";
import {PositionOverlay} from "@/components/ui/position-overlay";
import {
  AddCardButton,
  AppPage,
  CardBody,
  ChannelTabs,
  ControlTile,
  InlineNotice,
  MacroHeader,
  SaveStateBadge,
  SectionHeader,
  SignalTile,
  StatusMatrix,
  TacticalCard,
} from "@/components/app/app-ui";
import type {
  RapidfireBootstrap,
  RapidfireCardForm,
  RapidfireGroupForm,
  RapidfireRunState,
  RapidfireSelectionOutcome,
  RapidfireSettings,
  RapidfireSettingsForm,
} from "@/components/app/rapidfire-types";
import {
  createRapidfireCard,
  createRapidfireGroup,
  DEFAULT_RAPIDFIRE_GROUP_ID,
  formatTriggerHotkey,
  formatTriggerKey,
  moveRapidfireCard,
  parseRapidfireSettingsForm,
  RAPIDFIRE_AUTOSAVE_DELAY_MS,
  RAPIDFIRE_DEFAULT_COMPENSATION_DELAY_MAX_MS,
  RAPIDFIRE_DEFAULT_COMPENSATION_DELAY_MIN_MS,
  RAPIDFIRE_DISPLAY_MAX_WIDTH,
  RAPIDFIRE_DISPLAY_MIN_WIDTH,
  RAPIDFIRE_GLOBAL_DELAY_MAX_MS,
  RAPIDFIRE_GLOBAL_DELAY_MIN_MS,
  RAPIDFIRE_MIN_INTERVAL_MS,
  RAPIDFIRE_PRESS_JITTER_MAX_MS,
  RAPIDFIRE_PRESS_JITTER_MIN_MS,
  RAPIDFIRE_TRIGGER_JITTER_MAX_MS,
  rapidfireCardError,
  rapidfireCardStatus,
  rapidfireEffectiveCardsByGroup,
  rapidfireEnabledCards,
  rapidfireRunsById,
  rapidfireSettingsToForm,
  rapidfireStatusLabel,
} from "@/components/app/rapidfire-types";
import {getErrorMessage} from "@/lib/error-utils";
import {useFavorites} from "@/hooks/use-favorites";
import {useNativeShell} from "@/hooks/use-native-shell";
import {useBootstrapForm} from "@/hooks/use-bootstrap-form";
import {useAutosave} from "@/hooks/use-autosave";
import {useHotkeyRecorder} from "@/hooks/use-hotkey-recorder";
import {cn} from "@/lib/utils";

const RAPIDFIRE_BOOTSTRAP_SPEC = {
    getBootstrapCommand: "rapidfire_get_bootstrap",
    saveSettingsCommand: "rapidfire_save_settings",
    settingsToForm: rapidfireSettingsToForm,
    parseSettingsForm: parseRapidfireSettingsForm,
};

type RapidfireDisplayMode = "display" | "position";
type RecordingTarget = { cardId: string; field: "triggerKey" | "targetKey" } | null;

function rapidfireCardId(): string {
    const suffix = `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
    return `rapidfire-${suffix}`;
}

export type RapidfireHighlightTarget = {
    kind: "rapidfire";
    cardId: string;
    /** nonce 用于强制重触发高亮动画 */
    nonce: number;
};

type RapidfirePageProps = {
    overlayMode?: RapidfireDisplayMode;
    highlightCardId?: RapidfireHighlightTarget | null;
};

export function RapidfirePage({highlightCardId, overlayMode}: RapidfirePageProps) {
    const isNativeShell = useNativeShell();
    const overlayGroupId = new URLSearchParams(window.location.search).get("groupId") ?? DEFAULT_RAPIDFIRE_GROUP_ID;

    if (overlayMode === "display") {
        return <RapidfireDisplayOverlay groupId={overlayGroupId} isNativeShell={isNativeShell}/>;
    }

    if (overlayMode === "position") {
        return <RapidfirePositionOverlay isNativeShell={isNativeShell}/>;
    }

    return <RapidfireWorkbench highlightCardId={highlightCardId ?? null} isNativeShell={isNativeShell}/>;
}

function RapidfireWorkbench({highlightCardId, isNativeShell}: {
    highlightCardId: RapidfireHighlightTarget | null;
    isNativeShell: boolean
}) {
    const [recordingTarget, setRecordingTarget] = useState<RecordingTarget>(null);
    const draggingCardIdRef = useRef<string | null>(null);
    const [draggingCardId, setDraggingCardId] = useState<string | null>(null);
    const favorites = useFavorites();

    useEffect(() => {
        if (!highlightCardId) return;
        if (highlightCardId.kind !== "rapidfire") return;
        const cardId = highlightCardId.cardId;
        const timer = window.setTimeout(() => {
            const element = document.querySelector(`[data-favorite-card="rapidfire:${cardId}"]`);
            if (element instanceof HTMLElement) {
                element.classList.remove("favorite-highlight");
                void element.offsetWidth;
                element.classList.add("favorite-highlight");
                element.scrollIntoView({behavior: "smooth", block: "center"});
            }
        }, 80);
        return () => window.clearTimeout(timer);
    }, [highlightCardId]);

    useEffect(() => {
        if (!isNativeShell) return;
        const handlePointerUp = () => {
            draggingCardIdRef.current = null;
            setDraggingCardId(null);
        };
        window.addEventListener("pointerup", handlePointerUp);
        return () => window.removeEventListener("pointerup", handlePointerUp);
    }, [isNativeShell]);

    const [activeTab, setActiveTab] = useState<"cards" | "global" | "display">("cards");

    const beforeUpdateFormRef = useRef<() => void>(() => {
    });

    const bf = useBootstrapForm<RapidfireBootstrap, RapidfireSettings, RapidfireSettingsForm>({
        spec: RAPIDFIRE_BOOTSTRAP_SPEC,
        isNativeShell,
        loadStatusMessage: "正在加载连发器...",
        readyStatusMessage: "连发器已就绪。按住触发键开始；未开启不追加的卡片会在松开后自动补齐奇数次数。",
        previewStatusMessage: "浏览器预览模式：只显示界面，原生命令请在桌面端运行。",
        saveSuccessMessage: (next) => next.settings.rapidfireEnabled
            ? "连发器设置已保存，触发键已生效。"
            : "连发器已关闭：触发键已解绑，透明窗口已隐藏，配置已保留。",
        beforeUpdateForm: () => beforeUpdateFormRef.current(),
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
        statusMessage,
        setStatusMessage,
        autosaveVersionRef
    } = bf;

    const clearStaleConfigError = useCallback(() => {
        if (!pageError) return;
        setPageError(null);
        setStatusMessage("配置已更新，等待自动保存...");
    }, [pageError]);

    beforeUpdateFormRef.current = clearStaleConfigError;

    useEffect(() => {
        if (isNativeShell) return;
        setForm({
            rapidfireEnabled: false,
            showOverlay: true,
            overlayWidth: "400",
            compensationDelayMinMs: String(RAPIDFIRE_DEFAULT_COMPENSATION_DELAY_MIN_MS),
            compensationDelayMaxMs: String(RAPIDFIRE_DEFAULT_COMPENSATION_DELAY_MAX_MS),
            overlayPosition: null,
            groups: [
                {
                    id: DEFAULT_RAPIDFIRE_GROUP_ID,
                    name: "默认分组",
                    enabled: true,
                    showOverlay: true,
                    overlayPosition: null,
                    overlayWidth: "400",
                },
            ],
            cards: [createRapidfireCard(rapidfireCardId(), 0, DEFAULT_RAPIDFIRE_GROUP_ID)],
        });
    }, [isNativeShell]);

    useEffect(() => {
        if (!isNativeShell) return;

        let disposed = false;
        let unlistenStateChanged: (() => void) | undefined;
        let unlistenHotkeyError: (() => void) | undefined;

        void listen<RapidfireBootstrap>(RAPIDFIRE_EVENTS.stateChanged, (event) => {
            if (disposed) return;
            setBootstrap(event.payload);
        }).then((dispose) => {
            unlistenStateChanged = dispose;
        });

        void listen<string>(RAPIDFIRE_EVENTS.hotkeyError, (event) => {
            if (disposed) return;
            setPageError(event.payload);
            setStatusMessage(event.payload);
        }).then((dispose) => {
            unlistenHotkeyError = dispose;
        });

        return () => {
            disposed = true;
            unlistenStateChanged?.();
            unlistenHotkeyError?.();
        };
    }, [isNativeShell]);

    const runsById = useMemo(() => rapidfireRunsById(bootstrap?.runs ?? []), [bootstrap?.runs]);
    const controlsDisabled = loading || !isNativeShell;
    const activeRunCount = useMemo(
        () => (bootstrap?.runs ?? []).filter((run) => run.status !== "idle").length,
        [bootstrap?.runs],
    );
    const enabledCount = rapidfireEnabledCards(form);
    const totalFireCount = useMemo(
        () => (bootstrap?.runs ?? []).reduce((total, run) => total + run.count, 0),
        [bootstrap?.runs],
    );

    const updateCard = useCallback((id: string, value: Partial<RapidfireCardForm>) => {
        clearStaleConfigError();
        setForm((current) =>
            current
                ? {
                    ...current,
                    cards: current.cards.map((card) => (card.id === id ? {...card, ...value} : card)),
                }
                : current,
        );
    }, [clearStaleConfigError]);

    const recordingTargetRef = useRef<RecordingTarget>(null);
    recordingTargetRef.current = recordingTarget;

    const recorder = useHotkeyRecorder({
        formatKey: (event) => {
            const target = recordingTargetRef.current;
            if (!target) return null;
            if (target.field === "triggerKey") {
                const result = formatTriggerHotkey(event);
                return result || null;
            }
            return formatTriggerKey(event.key) || null;
        },
        validate: (key, event) => {
            const target = recordingTargetRef.current;
            if (!target || !key) return false;
            const modifierOnly = ["Control", "Alt", "Shift", "Meta"].includes(event.key);
            if (modifierOnly) {
                setStatusMessage(target.field === "triggerKey" ? "请按下组合键的主键。" : "目标键必须是单键。");
                return false;
            }
            if (target.field === "targetKey" && key.includes("+")) {
                setStatusMessage("目标键必须是单键。");
                return false;
            }
            return true;
        },
        onCommit: (key) => {
            const target = recordingTargetRef.current;
            if (!target) return;
            setRecordingTarget(null);
            updateCard(target.cardId, {[target.field]: key});
        },
        onCancel: (draft) => {
            const target = recordingTargetRef.current;
            if (!target) return;
            setRecordingTarget(null);
            updateCard(target.cardId, {[target.field]: draft});
        },
        onStatusMessage: setStatusMessage,
        keyRecordedMessage: (key) => `新的按键已录制：${key}`,
        recordingCancelledMessage: "已取消按键录制。",
    });

    const updateGroup = useCallback((id: string, value: Partial<RapidfireGroupForm>) => {
        clearStaleConfigError();
        setForm((current) =>
            current
                ? {
                    ...current,
                    groups: current.groups.map((group) => (group.id === id ? {...group, ...value} : group)),
                    showOverlay: id === DEFAULT_RAPIDFIRE_GROUP_ID && typeof value.showOverlay === "boolean" ? value.showOverlay : current.showOverlay,
                    overlayPosition: id === DEFAULT_RAPIDFIRE_GROUP_ID && "overlayPosition" in value ? value.overlayPosition ?? null : current.overlayPosition,
                    overlayWidth: id === DEFAULT_RAPIDFIRE_GROUP_ID && typeof value.overlayWidth === "string" ? value.overlayWidth : current.overlayWidth,
                }
                : current,
        );
    }, [clearStaleConfigError]);

    useAutosave<RapidfireSettingsForm>({
        form,
        isDirty,
        disabled: !isNativeShell || loading || !bootstrap || !form || !!recordingTarget,
        onSave: (formSnapshot, nextVersion) => {
            const settingsValue = parseRapidfireSettingsForm(formSnapshot);
            return saveSettings(settingsValue, nextVersion);
        },
        onError: (message) => {
            setPageError(message);
            setStatusMessage(`保存失败：${message}`);
        },
        delay: RAPIDFIRE_AUTOSAVE_DELAY_MS,
        autosaveVersionRef,
    });

    const beginRecording = useCallback((card: RapidfireCardForm, field: "triggerKey" | "targetKey") => {
        setRecordingTarget({cardId: card.id, field});
        recorder.beginRecording(field === "triggerKey" ? card.triggerKey : card.targetKey);
        setStatusMessage(`正在录制 ${card.name || "连发器"} 的${field === "triggerKey" ? "触发键" : "目标键"}，按下主键会保存；失焦会取消。触发键可按住 Ctrl/Alt/Shift/Win 录制组合键。`);
    }, [recorder]);

    const addCard = useCallback(() => {
        clearStaleConfigError();
        setForm((current) =>
            current
                ? {
                    ...current,
                    cards: [...current.cards, createRapidfireCard(rapidfireCardId(), current.cards.length, current.groups[0]?.id ?? DEFAULT_RAPIDFIRE_GROUP_ID)],
                }
                : current,
        );
    }, [clearStaleConfigError]);

    const addGroup = useCallback(() => {
        clearStaleConfigError();
        setForm((current) => current ? {
            ...current,
            groups: [...current.groups, createRapidfireGroup(current.groups.length)],
        } : current);
    }, [clearStaleConfigError]);

    const removeGroup = useCallback((groupId: string) => {
        clearStaleConfigError();
        setForm((current) => {
            if (!current) return current;
            if (current.groups.length <= 1) {
                toast.info("至少保留一个连发器分组。");
                return current;
            }
            if (current.cards.some((card) => card.groupId === groupId)) {
                toast.info("请先把此分组内的连发器移动到其他分组。");
                return current;
            }
            return {
                ...current,
                groups: current.groups.filter((group) => group.id !== groupId),
            };
        });
    }, [clearStaleConfigError]);

    const removeCard = useCallback((id: string) => {
        clearStaleConfigError();
        setForm((current) => {
            if (!current) {
                return current;
            }

            if (current.cards.length <= 1) {
                toast.info("至少保留一张连发器卡片，无需删除最后一张。");
                return current;
            }

            return {
                ...current,
                cards: current.cards.filter((card) => card.id !== id),
            };
        });
    }, [clearStaleConfigError]);

    const moveCard = useCallback((activeId: string, overId: string) => {
        clearStaleConfigError();
        setForm((current) =>
            current
                ? {
                    ...current,
                    cards: moveRapidfireCard(current.cards, activeId, overId),
                }
                : current,
        );
    }, [clearStaleConfigError]);

    const beginCardDrag = useCallback((id: string) => {
        draggingCardIdRef.current = id;
        setDraggingCardId(id);
    }, []);

    const moveDraggingCardOver = useCallback((overId: string) => {
        const activeId = draggingCardIdRef.current;
        if (!activeId || activeId === overId) {
            return;
        }
        moveCard(activeId, overId);
    }, [moveCard]);

    const stopAll = useCallback(async () => {
        if (!isNativeShell) return;
        try {
            setStatusMessage("正在停止所有连发...");
            const next = await invoke<RapidfireBootstrap>("rapidfire_stop");
            setBootstrap(next);
            setStatusMessage("已停止所有连发。");
        } catch (error) {
            const message = getErrorMessage(error);
            setPageError(message);
            setStatusMessage(message);
        }
    }, [isNativeShell]);

    const beginPositionSelection = useCallback(async (groupId?: string) => {
        if (!isNativeShell) return;
        try {
            setStatusMessage("正在打开连发器透明窗口位置设置...");
            const outcome = await invoke<RapidfireSelectionOutcome>("rapidfire_begin_position_selection", {groupId});
            await syncBootstrap({syncForm: true});
            if (outcome.kind === "selected") {
                setStatusMessage("连发器透明窗口位置已保存。");
            } else if (outcome.kind === "cancelled") {
                setStatusMessage("连发器透明窗口位置修改已取消。");
            } else {
                setStatusMessage("连发器透明窗口位置设置窗口已关闭。");
            }
        } catch (error) {
            const message = getErrorMessage(error);
            setPageError(message);
            setStatusMessage(message);
        }
    }, [isNativeShell, syncBootstrap]);

    if (!form) {
        return (
            <AppPage className="auto-rows-max">
                <MacroHeader
                    code="R-03"
                    title="RAPIDFIRE / CONTROL"
                    verticalLabel="连发器"
                    subtitle="按住触发键即可持续压发目标键；松开后默认执行奇数补齐，也可在单通道切断补齐链路。"
                />
                {pageError ? (
                    <div className="col-span-12">
                        <InlineNotice title="连发器加载失败">{pageError}</InlineNotice>
                    </div>
                ) : (
                    <AddCardButton className="col-span-12 min-h-36" disabled title="连发器准备中"
                                   description={statusMessage} onClick={() => undefined}/>
                )}
            </AppPage>
        );
    }

    return (
        <AppPage className="auto-rows-max">
            {pageError && (
                <InlineNotice title="连发器配置未生效">
                    {pageError}
                </InlineNotice>
            )}

            {!isNativeShell ? (
                <div className="col-span-12">
                    <InlineNotice title="浏览器预览模式">
                        当前在浏览器中运行，所有设置控件已禁用。请通过桌面端应用操作：运行 <code
                        className="font-mono text-[var(--amber)]">bun run tauri dev</code> 或使用安装后的桌面应用。
                    </InlineNotice>
                </div>
            ) : null}

            <MacroHeader
                code="R-03"
                title="RAPIDFIRE / CONTROL"
                verticalLabel="连发"
                subtitle="按住触发键即可持续压发目标键；松开后默认执行奇数补齐，也可在单通道切断补齐链路。"
                badges={
                    <>
                        <Badge
                            variant={form.rapidfireEnabled ? "default" : "outline"}>{form.rapidfireEnabled ? "总线接通" : "总线断开"}</Badge>
                        <Badge variant="secondary">{enabledCount} 条通道待命</Badge>
                        <SaveStateBadge dirty={isDirty} saving={saving}/>
                    </>
                }
                actions={
                    <>
                        <SignalTile label="活跃通道" value={activeRunCount} detail="非空闲卡片数量"/>
                        <SignalTile label="待命通道" value={enabledCount} detail="已挂接触发键监听"/>
                        <SignalTile label="累计发射" value={totalFireCount} detail={statusMessage}/>
                        <Button variant="outline" size="sm" disabled={controlsDisabled}
                                onClick={() => void beginPositionSelection(DEFAULT_RAPIDFIRE_GROUP_ID)}>
                            <RiMapPinLine data-icon="inline-start"/>
                            校准位置
                        </Button>
                        <Button variant="outline" size="sm" disabled={controlsDisabled} onClick={stopAll}>
                            <RiStopLine data-icon="inline-start"/>
                            全部停止
                        </Button>
                    </>
                }
            />

            <div className="col-span-12">
                <StatusMatrix items={[
                    {id: "rapidfire", state: form.rapidfireEnabled ? "active" : "idle", label: "总线状态"},
                    {id: "active", state: activeRunCount > 0 ? "active" : "idle", label: "活跃通道"},
                    {id: "enabled", state: enabledCount > 0 ? "valid" : "warning", label: "待命通道"},
                    {id: "save", state: isDirty ? "warning" : "valid", label: "保存状态"},
                    {
                        id: "hotkey",
                        state: bootstrap?.hotkeyError ? "error" : form.rapidfireEnabled ? "valid" : "idle",
                        label: "热键状态"
                    },
                    {id: "ready", state: form.rapidfireEnabled ? "valid" : "idle", label: "就绪状态"},
                ]}/>
            </div>

            <div className="col-span-12">
                <ChannelTabs
                    tabs={[
                        {id: "cards", label: "通道", active: activeTab === "cards"},
                        {id: "global", label: "全局", active: activeTab === "global"},
                        {id: "display", label: "显示", active: activeTab === "display"},
                    ]}
                    onTabChange={(id) => setActiveTab(id as "cards" | "global" | "display")}
                />
            </div>

            {activeTab === "global" && (
                <TacticalCard className="col-span-12 xl:col-span-7">
                    <SectionHeader
                        eyebrow="主控排程"
                        icon={<RiPulseLine/>}
                        title="全局发射设定"
                        description="总开关与补齐延时写入同一主控档；单卡片节奏参数在各通道单元中独立校准。"
                    />
                    <CardBody className="grid gap-3">
                        <FieldGroup className="grid gap-3 md:grid-cols-2">
                            <ControlTile className="bg-[var(--carbon)]">
                                <Field orientation="horizontal">
                                    <Switch
                                        id="rapidfireEnabled"
                                        checked={form.rapidfireEnabled}
                                        disabled={controlsDisabled}
                                        onCheckedChange={(checked) => updateForm("rapidfireEnabled", checked)}
                                    />
                                    <span
                                        className={cn(
                                            "font-mono text-xs font-medium tracking-[0.12em] uppercase",
                                            form.rapidfireEnabled ? "text-[var(--amber)]" : "text-[var(--zinc)]",
                                        )}
                                    >
                                        {form.rapidfireEnabled ? "ARMED" : "DISARMED"}
                                    </span>
                                    <FieldContent>
                                        <FieldLabel htmlFor="rapidfireEnabled">连发器总开关</FieldLabel>
                                        <FieldDescription>断开后立即解绑触发键，并同步关闭透明窗口。</FieldDescription>
                                    </FieldContent>
                                </Field>
                            </ControlTile>
                            <ControlTile className="bg-[var(--carbon)]">
                                <Field>
                                    <FieldLabel htmlFor="compensationDelayMinMs">补齐延迟下限</FieldLabel>
                                    <div className="flex items-center gap-2">
                                        <Input
                                            id="compensationDelayMinMs"
                                            className="w-28 bg-[var(--carbon)] font-mono"
                                            type="number"
                                            min={RAPIDFIRE_GLOBAL_DELAY_MIN_MS}
                                            max={RAPIDFIRE_GLOBAL_DELAY_MAX_MS}
                                            value={form.compensationDelayMinMs}
                                            disabled={controlsDisabled}
                                            onChange={(event) => updateForm("compensationDelayMinMs", event.target.value)}
                                        />
                                        <FieldTitle>ms</FieldTitle>
                                    </div>
                                    <FieldDescription>执行奇数补齐前的随机等待下限。</FieldDescription>
                                </Field>
                            </ControlTile>
                        </FieldGroup>
                        <FieldGroup className="grid gap-3 md:grid-cols-2">
                            <ControlTile className="bg-[var(--carbon)]">
                                <Field>
                                    <FieldLabel htmlFor="compensationDelayMaxMs">补齐延迟上限</FieldLabel>
                                    <div className="flex items-center gap-2">
                                        <Input
                                            id="compensationDelayMaxMs"
                                            className="w-28 bg-[var(--carbon)] font-mono"
                                            type="number"
                                            min={RAPIDFIRE_GLOBAL_DELAY_MIN_MS}
                                            max={RAPIDFIRE_GLOBAL_DELAY_MAX_MS}
                                            value={form.compensationDelayMaxMs}
                                            disabled={controlsDisabled}
                                            onChange={(event) => updateForm("compensationDelayMaxMs", event.target.value)}
                                        />
                                        <FieldTitle>ms</FieldTitle>
                                    </div>
                                    <FieldDescription>下限不得大于上限。</FieldDescription>
                                </Field>
                            </ControlTile>
                        </FieldGroup>
                    </CardBody>
                </TacticalCard>
            )}

            {activeTab === "global" && (
                <TacticalCard className="col-span-12 xl:col-span-5">
                    <SectionHeader
                        eyebrow="分组母线"
                        icon={<RiPulseLine/>}
                        title="通道分组矩阵"
                        description="每个分组维护独立透明窗口；总开关、分组开关与卡片开关同时接通时才响应触发键。"
                        actions={
                            <Button type="button" variant="outline" size="sm" disabled={controlsDisabled}
                                    onClick={addGroup}>
                                <RiAddLine data-icon="inline-start"/>
                                新增分组
                            </Button>
                        }
                    />
                    <CardBody className="grid gap-3">
                        {form.groups.map((group, index) => (
                            <ControlTile key={group.id} className="flex flex-col gap-4 bg-[var(--carbon)]">
                                <div
                                    className="flex items-start justify-between gap-3 border-b-2 border-[var(--chalk)] pb-3">
                                    <div className="min-w-0">
                                        <p className="font-mono text-xs font-medium tracking-[0.12em] text-[var(--amber)] uppercase">
                                            第 {String(index + 1).padStart(2, "0")} 组
                                        </p>
                                        <p className="mt-2 text-sm font-semibold text-foreground">{group.name}</p>
                                        <p className="mt-1 font-mono text-xs font-medium tracking-[0.08em] text-muted-foreground">
                                            {rapidfireEffectiveCardsByGroup(form, group.id).length} 张有效卡片
                                        </p>
                                    </div>
                                    <Switch checked={group.enabled} disabled={controlsDisabled}
                                            onCheckedChange={(checked) => updateGroup(group.id, {enabled: checked})}/>
                                </div>
                                <FieldGroup className="grid gap-3 md:grid-cols-2">
                                    <Field>
                                        <FieldLabel>分组名称</FieldLabel>
                                        <Input className="bg-[var(--carbon)]" disabled={controlsDisabled} value={group.name}
                                               onChange={(event) => updateGroup(group.id, {name: event.currentTarget.value})}/>
                                    </Field>
                                    <Field>
                                        <FieldLabel>透明窗口宽度</FieldLabel>
                                        <Input
                                            className="bg-[var(--carbon)] font-mono"
                                            disabled={controlsDisabled || !group.enabled}
                                            max={RAPIDFIRE_DISPLAY_MAX_WIDTH}
                                            min={RAPIDFIRE_DISPLAY_MIN_WIDTH}
                                            onChange={(event) => updateGroup(group.id, {overlayWidth: event.currentTarget.value})}
                                            type="number"
                                            value={group.overlayWidth}
                                        />
                                    </Field>
                                </FieldGroup>
                                <div className="flex flex-wrap items-center gap-2 border-t-2 border-[var(--chalk)] pt-3">
                                    <ControlTile className="flex items-center gap-2 bg-[var(--slate)] px-3 py-2">
                                        <Switch checked={group.showOverlay} disabled={controlsDisabled || !group.enabled}
                                                onCheckedChange={(checked) => updateGroup(group.id, {showOverlay: checked})}/>
                                        <span
                                            className="font-mono text-xs font-medium tracking-[0.08em] text-muted-foreground uppercase">透明窗口</span>
                                    </ControlTile>
                                    <Button type="button" variant="outline" size="sm"
                                            disabled={controlsDisabled || !group.enabled}
                                            onClick={() => void beginPositionSelection(group.id)}>
                                        <RiMapPinLine data-icon="inline-start"/>
                                        校准位置
                                    </Button>
                                    <Button
                                        type="button"
                                        variant="outline"
                                        size="sm"
                                        disabled={controlsDisabled || form.groups.length <= 1 || form.cards.some((card) => card.groupId === group.id)}
                                        onClick={() => removeGroup(group.id)}
                                    >
                                        <RiDeleteBinLine data-icon="inline-start"/>
                                        删除空分组
                                    </Button>
                                </div>
                            </ControlTile>
                        ))}
                    </CardBody>
                </TacticalCard>
            )}

            {activeTab === "display" && (
                <TacticalCard className="col-span-12">
                    <SectionHeader
                        eyebrow="显示配置"
                        icon={<RiPulseLine/>}
                        title="透明窗口与显示设定"
                        description="控制连发器透明窗口的开关、宽度与位置校准；游戏内仅投送启用通道与当前发射计数。"
                    />
                    <CardBody className="grid gap-3">
                        <FieldGroup className="grid gap-3 md:grid-cols-3">
                            <ControlTile className="bg-[var(--carbon)]">
                                <Field orientation="horizontal">
                                    <Switch
                                        id="showOverlay"
                                        checked={form.showOverlay}
                                        disabled={controlsDisabled}
                                        onCheckedChange={(checked) => updateForm("showOverlay", checked)}
                                    />
                                    <FieldContent>
                                        <FieldLabel htmlFor="showOverlay">透明窗口</FieldLabel>
                                        <FieldDescription>游戏内仅投送启用通道与当前发射计数。</FieldDescription>
                                    </FieldContent>
                                </Field>
                            </ControlTile>
                            <ControlTile className="bg-[var(--carbon)]">
                                <Field>
                                    <FieldLabel htmlFor="overlayWidth">透明窗口宽度</FieldLabel>
                                    <Input
                                        id="overlayWidth"
                                        className="max-w-32 bg-[var(--carbon)] font-mono"
                                        type="number"
                                        min={RAPIDFIRE_DISPLAY_MIN_WIDTH}
                                        max={RAPIDFIRE_DISPLAY_MAX_WIDTH}
                                        value={form.overlayWidth}
                                        disabled={controlsDisabled}
                                        onChange={(event) => updateForm("overlayWidth", event.target.value)}
                                    />
                                    <FieldDescription>{RAPIDFIRE_DISPLAY_MIN_WIDTH}-{RAPIDFIRE_DISPLAY_MAX_WIDTH}px。</FieldDescription>
                                </Field>
                            </ControlTile>
                            <ControlTile className="bg-[var(--carbon)]">
                                <Button variant="outline" size="sm" disabled={controlsDisabled}
                                        onClick={() => void beginPositionSelection(DEFAULT_RAPIDFIRE_GROUP_ID)}>
                                    <RiMapPinLine data-icon="inline-start"/>
                                    校准位置
                                </Button>
                            </ControlTile>
                        </FieldGroup>
                    </CardBody>
                </TacticalCard>
            )}

            {activeTab === "cards" && (
            <section className="col-span-12 grid min-h-0 gap-4 xl:grid-cols-2">
                {form.cards.map((card, index) => {
                    const run = runsById.get(card.id);
                    const isRecording = recordingTarget?.cardId === card.id;
                    const cardError = rapidfireCardError(card, pageError);
                    return (
                        <RapidfireCardEditor
                            key={card.id}
                            card={card}
                            index={index}
                            total={form.cards.length}
                            previousId={form.cards[index - 1]?.id ?? null}
                            nextId={form.cards[index + 1]?.id ?? null}
                            run={run}
                            cardError={cardError}
                            disabled={controlsDisabled}
                            groupOptions={form.groups}
                            isFavorite={favorites.isFavorite("rapidfire", card.id)}
                            isHighlighted={Boolean(highlightCardId && highlightCardId.kind === "rapidfire" && highlightCardId.cardId === card.id)}
                            isRecording={isRecording}
                            isDragging={draggingCardId === card.id}
                            recordingField={recordingTarget?.field}
                            onUpdate={updateCard}
                            onRecord={beginRecording}
                            onRecorderKeyDown={recorder.handleKeyDown}
                            onRecorderBlur={recorder.handleBlur}
                            onMove={moveCard}
                            onDragStart={() => beginCardDrag(card.id)}
                            onDragOver={() => moveDraggingCardOver(card.id)}
                            onDelete={() => removeCard(card.id)}
                            onToggleFavorite={() => favorites.toggleFavorite("rapidfire", card.id)}
                        />
                    );
                })}
                <AddCardButton className="min-h-36" disabled={controlsDisabled} title="新增通道单元"
                               description="建立新的触发键、目标键与节奏配置。" onClick={addCard}/>
            </section>
            )}
        </AppPage>
    );
}

interface RapidfireCardEditorProps {
    card: RapidfireCardForm;
    index: number;
    total: number;
    previousId: string | null;
    nextId: string | null;
    run: RapidfireRunState | undefined;
    cardError: string | null;
    disabled: boolean;
    groupOptions: RapidfireGroupForm[];
    isFavorite: boolean;
    isHighlighted: boolean;
    isRecording: boolean;
    isDragging: boolean;
    recordingField: "triggerKey" | "targetKey" | undefined;
    onUpdate: (id: string, value: Partial<RapidfireCardForm>) => void;
    onRecord: (card: RapidfireCardForm, field: "triggerKey" | "targetKey") => void;
    onRecorderKeyDown: (event: React.KeyboardEvent<HTMLButtonElement>) => void;
    onRecorderBlur: () => void;
    onMove: (activeId: string, overId: string) => void;
    onDragStart: () => void;
    onDragOver: () => void;
    onDelete: () => void;
    onToggleFavorite: () => void;
}

function RapidfireCardEditor({
                                 card,
                                 index,
                                 total,
                                 previousId,
                                 nextId,
                                 run,
                                 cardError,
                                 disabled,
                                 groupOptions,
                                 isFavorite,
                                 isHighlighted,
                                 isRecording,
                                 isDragging,
                                 recordingField,
                                 onUpdate,
                                 onRecord,
                                 onRecorderKeyDown,
                                 onRecorderBlur,
                                 onMove,
                                 onDragStart,
                                 onDragOver,
                                 onDelete,
                                 onToggleFavorite,
                             }: RapidfireCardEditorProps) {
    const isRunning = run?.status === "firing";
    const isPending = run?.status === "pendingCompensation";
    const status = rapidfireCardStatus(card, run, cardError);

    return (
        <TacticalCard
            active={status.active || isRunning || isPending || isDragging}
            data-favorite-card={`rapidfire:${card.id}`}
            onPointerEnter={onDragOver}
            className={cn(
                "bg-[var(--carbon)]",
                !card.enabled && !status.error && "opacity-80",
                status.error && "border-[var(--amber)] bg-[var(--slate)] outline-2 outline-[var(--amber)]",
                isDragging && "outline-4 outline-[var(--amber)]",
                isHighlighted && "outline-4 outline-[var(--amber)]",
            )}
        >
            <SectionHeader
                eyebrow="连发器"
                title={(
                    <Input
                        className="h-auto w-full border-0 bg-transparent p-0 font-heading text-lg font-medium uppercase text-[var(--carbon)] placeholder:text-[var(--slate)] focus-visible:ring-0 focus-visible:ring-offset-0"
                        placeholder="输入卡片名称"
                        value={card.name || "连发器"}
                        disabled={disabled}
                        onChange={(event) => onUpdate(card.id, {name: event.target.value})}
                        aria-label="通道名称"
                    />
                )}
                description={`触发 ${card.triggerKey || "--"} / 目标 ${card.targetKey || "--"} / 间隔 ${card.intervalMs || "--"}ms / ${card.skipCompensation ? "补齐断开" : "补齐接通"}`}
                badge={
                    <Badge variant={status.variant}>{status.label}</Badge>
                }
                className={cn(status.error && "bg-[var(--amber)]")}
            />
            {isRunning || isPending ? (
                <div
                    className="flex items-center gap-2 border-b-2 border-[var(--chalk)] bg-[var(--amber)] px-3 py-1 font-mono text-[0.62rem] font-black tracking-[0.18em] text-[var(--carbon)] uppercase">
                    <span className="inline-block size-1.5 bg-[var(--carbon)]"/>
                    {isRunning ? "FIRING" : "ARMED"}
                </div>
            ) : null}
            <CardHeader className="border-b-2 border-[var(--chalk)] bg-[var(--slate)] pt-0">
                <div className="grid gap-3 xl:grid-cols-[1fr_auto]">
                    <div className="grid gap-3">
                        <div>
                            <p className="font-mono text-xs font-medium tracking-[0.12em] text-[var(--zinc)] uppercase">所属分组</p>
                            <Select disabled={disabled} value={card.groupId}
                                    onValueChange={(value) => onUpdate(card.id, {groupId: value})}>
                                <SelectTrigger className="mt-2 w-full max-w-full bg-[var(--carbon)]">
                                    <SelectValue placeholder="选择分组"/>
                                </SelectTrigger>
                                <SelectContent>
                                    {groupOptions.map((group) => (
                                        <SelectItem key={group.id} value={group.id}>
                                            {group.name}
                                        </SelectItem>
                                    ))}
                                </SelectContent>
                            </Select>
                        </div>
                    </div>
                    <div
                        className="flex flex-wrap items-center justify-end gap-1.5 border-t-2 border-[var(--chalk)] pt-3 xl:border-t-0 xl:border-l-2 xl:pl-3 xl:pt-0">
                        <RapidfireCardDragHandle disabled={disabled} onDragStart={onDragStart}/>

                        <Button
                            variant="outline"
                            size="icon-sm"
                            disabled={disabled || index === 0}
                            aria-label="上移卡片"
                            onClick={() => previousId && onMove(card.id, previousId)}
                        >
                            <RiArrowUpLine/>
                        </Button>
                        <Button
                            variant="outline"
                            size="icon-sm"
                            disabled={disabled || index >= total - 1}
                            aria-label="下移卡片"
                            onClick={() => nextId && onMove(card.id, nextId)}
                        >
                            <RiArrowDownSLine/>
                        </Button>
                        <Switch
                            checked={card.enabled}
                            disabled={disabled}
                            aria-label="启用卡片"
                            onCheckedChange={(checked) => onUpdate(card.id, {enabled: checked})}
                        />
                        <Button
                            aria-label={isFavorite ? "取消收藏" : "加入收藏"}
                            aria-pressed={isFavorite}
                            className={cn(isFavorite ? "text-[var(--amber)]" : "text-muted-foreground")}
                            data-icon="inline-start"
                            disabled={disabled}
                            onClick={onToggleFavorite}
                            size="icon-sm"
                            type="button"
                            variant="outline"
                        >
                            {isFavorite ? <RiStarFill/> : <RiStarLine/>}
                        </Button>
                        <Button variant="outline" size="icon-sm" disabled={disabled} onClick={onDelete}
                                aria-label="删除卡片">
                            <RiDeleteBinLine/>
                        </Button>
                    </div>
                </div>
            </CardHeader>
            <CardBody>
                {cardError ? (
                    <InlineNotice className="mb-3" title="这张卡片的配置未生效">
                        {cardError}
                    </InlineNotice>
                ) : null}
                <FieldGroup className="grid gap-3 md:grid-cols-2">
                    <ControlTile className="bg-[var(--carbon)]">
                        <Field>
                            <FieldLabel>触发键</FieldLabel>
                            <KeyRecorderButton
                                value={card.triggerKey}
                                active={isRecording && recordingField === "triggerKey"}
                                disabled={disabled}
                                onClick={() => onRecord(card, "triggerKey")}
                                onKeyDown={onRecorderKeyDown}
                                onBlur={onRecorderBlur}
                            />
                            <FieldDescription>按住此键即启动连续发射；支持 Shift+- 这类组合热键。</FieldDescription>
                        </Field>
                    </ControlTile>
                    <ControlTile className="bg-[var(--carbon)]">
                        <Field>
                            <FieldLabel>目标键</FieldLabel>
                            <KeyRecorderButton
                                value={card.targetKey}
                                active={isRecording && recordingField === "targetKey"}
                                disabled={disabled}
                                onClick={() => onRecord(card, "targetKey")}
                                onKeyDown={onRecorderKeyDown}
                                onBlur={onRecorderBlur}
                            />
                            <FieldDescription>矩阵运行时将重复压发此键。</FieldDescription>
                        </Field>
                    </ControlTile>
                    <ControlTile className="bg-[var(--carbon)]">
                        <Field orientation="horizontal">
                            <Switch
                                id={`${card.id}-skip-compensation`}
                                checked={card.skipCompensation}
                                disabled={disabled}
                                onCheckedChange={(checked) => onUpdate(card.id, {skipCompensation: checked})}
                            />
                            <FieldContent>
                                <FieldLabel htmlFor={`${card.id}-skip-compensation`}>不追加补齐</FieldLabel>
                                <FieldDescription>断开后不补发尾次，保持原始奇偶结果。</FieldDescription>
                            </FieldContent>
                        </Field>
                    </ControlTile>
                    <ControlTile className="bg-[var(--carbon)]">
                        <Field orientation="horizontal">
                            <Switch
                                id={`${card.id}-ignore-trigger-key`}
                                checked={card.ignoreTriggerKey}
                                disabled={disabled}
                                onCheckedChange={(checked) => onUpdate(card.id, {ignoreTriggerKey: checked})}
                            />
                            <FieldContent>
                                <FieldLabel htmlFor={`${card.id}-ignore-trigger-key`}>忽略触发键</FieldLabel>
                                <FieldDescription>连发时阻止触发键本身同步输入；同触发键的其他卡片仍可触发。</FieldDescription>
                            </FieldContent>
                        </Field>
                    </ControlTile>
                    <ControlTile className="bg-[var(--carbon)]">
                        <Field>
                            <FieldLabel htmlFor={`${card.id}-interval`}>连发间隔</FieldLabel>
                            <div className="flex items-center gap-2">
                                <Input
                                    id={`${card.id}-interval`}
                                    className="w-28 bg-[var(--carbon)] font-mono"
                                    type="number"
                                    min={RAPIDFIRE_MIN_INTERVAL_MS}
                                    value={card.intervalMs}
                                    disabled={disabled}
                                    onChange={(event) => onUpdate(card.id, {intervalMs: event.target.value})}
                                />
                                <FieldTitle>ms</FieldTitle>
                            </div>
                            <FieldDescription>最小 {RAPIDFIRE_MIN_INTERVAL_MS}ms。</FieldDescription>
                        </Field>
                    </ControlTile>
                    <Collapsible defaultOpen={Boolean(cardError)} className="md:col-span-2">
                        <ControlTile className="overflow-hidden bg-[var(--carbon)] p-0">
                            <CollapsibleTrigger asChild>
                                <Button
                                    className="w-full justify-between bg-[var(--slate)] px-3 py-3 font-mono text-[0.72rem] font-black tracking-[0.08em] uppercase"
                                    type="button" variant="ghost">
                                    高级校准面板
                                    <RiArrowDownSLine className="size-4"/>
                                </Button>
                            </CollapsibleTrigger>
                            <CollapsibleContent
                                className="border-t-2 border-[var(--chalk)] bg-[var(--carbon)] px-3 py-3">
                                <FieldGroup className="grid gap-3 md:grid-cols-2">
                                    <ControlTile className="bg-[var(--slate)]">
                                        <Field>
                                            <FieldLabel>触发抖动</FieldLabel>
                                            <div
                                                className="grid grid-cols-[minmax(4.75rem,1fr)_auto_minmax(4.75rem,1fr)_auto] items-center gap-2">
                                                <Input
                                                    id={`${card.id}-jitter-min`}
                                                    className="min-w-0 bg-[var(--carbon)] font-mono"
                                                    type="number"
                                                    min={RAPIDFIRE_PRESS_JITTER_MIN_MS}
                                                    max={RAPIDFIRE_PRESS_JITTER_MAX_MS}
                                                    value={card.pressJitterMinMs}
                                                    disabled={disabled}
                                                    aria-label="触发抖动最小值"
                                                    onChange={(event) => onUpdate(card.id, {pressJitterMinMs: event.target.value})}
                                                />
                                                <span className="text-xs text-muted-foreground">至</span>
                                                <Input
                                                    id={`${card.id}-jitter-max`}
                                                    className="min-w-0 bg-[var(--carbon)] font-mono"
                                                    type="number"
                                                    min={RAPIDFIRE_PRESS_JITTER_MIN_MS}
                                                    max={RAPIDFIRE_PRESS_JITTER_MAX_MS}
                                                    value={card.pressJitterMaxMs}
                                                    disabled={disabled}
                                                    aria-label="触发抖动最大值"
                                                    onChange={(event) => onUpdate(card.id, {pressJitterMaxMs: event.target.value})}
                                                />
                                                <FieldTitle>ms</FieldTitle>
                                            </div>
                                            <FieldDescription>目标键按下保持时间范围。</FieldDescription>
                                        </Field>
                                    </ControlTile>
                                    <ControlTile className="bg-[var(--slate)]">
                                        <Field>
                                            <FieldLabel
                                                htmlFor={`${card.id}-min-spacing`}>当前卡片按键最小间距</FieldLabel>
                                            <div className="flex items-center gap-2">
                                                <Input
                                                    id={`${card.id}-min-spacing`}
                                                    className="w-28 bg-[var(--carbon)] font-mono"
                                                    type="number"
                                                    min={RAPIDFIRE_GLOBAL_DELAY_MIN_MS}
                                                    max={RAPIDFIRE_GLOBAL_DELAY_MAX_MS}
                                                    value={card.minPressSpacingMs}
                                                    disabled={disabled}
                                                    onChange={(event) => onUpdate(card.id, {minPressSpacingMs: event.target.value})}
                                                />
                                                <FieldTitle>ms</FieldTitle>
                                            </div>
                                            <FieldDescription>仅限制本通道目标键的触发间距，不拖慢其他通道。</FieldDescription>
                                        </Field>
                                    </ControlTile>
                                    <ControlTile className="bg-[var(--slate)]">
                                        <Field>
                                            <FieldLabel
                                                htmlFor={`${card.id}-trigger-jitter`}>当前卡片启动抖动上限</FieldLabel>
                                            <div className="flex items-center gap-2">
                                                <Input
                                                    id={`${card.id}-trigger-jitter`}
                                                    className="w-28 bg-[var(--carbon)] font-mono"
                                                    type="number"
                                                    min={0}
                                                    max={RAPIDFIRE_TRIGGER_JITTER_MAX_MS}
                                                    value={card.triggerJitterMaxMs}
                                                    disabled={disabled}
                                                    onChange={(event) => onUpdate(card.id, {triggerJitterMaxMs: event.target.value})}
                                                />
                                                <FieldTitle>ms（0=关闭）</FieldTitle>
                                            </div>
                                            <FieldDescription>按下触发键后，最久等待此时长再开始连发。</FieldDescription>
                                        </Field>
                                    </ControlTile>
                                    <ControlTile className="bg-[var(--slate)]">
                                        <Field orientation="horizontal">
                                            <Switch
                                                id={`${card.id}-cancel-jitter`}
                                                checked={card.cancelJitterOnRelease}
                                                disabled={disabled}
                                                onCheckedChange={(checked) => onUpdate(card.id, {cancelJitterOnRelease: checked})}
                                            />
                                            <FieldContent>
                                                <FieldLabel
                                                    htmlFor={`${card.id}-cancel-jitter`}>抖动期间松手立即触发</FieldLabel>
                                                <FieldDescription>仅作用于本通道；松手后立即执行一次并进入奇数补齐判断。</FieldDescription>
                                            </FieldContent>
                                        </Field>
                                    </ControlTile>
                                </FieldGroup>
                            </CollapsibleContent>
                        </ControlTile>
                    </Collapsible>
                </FieldGroup>
            </CardBody>
        </TacticalCard>
    );
}

function KeyRecorderButton({
                               value,
                               active,
                               disabled,
                               onClick,
                               onKeyDown,
                               onBlur,
                           }: {
    value: string;
    active: boolean;
    disabled: boolean;
    onClick: () => void;
    onKeyDown: (event: React.KeyboardEvent<HTMLButtonElement>) => void;
    onBlur: () => void;
}) {
    return (
        <Button
            type="button"
            variant={active ? "default" : "outline"}
            size="default"
            disabled={disabled}
            className={cn(
                "w-full justify-between bg-[var(--carbon)] font-mono text-[0.72rem] font-black tracking-[0.08em] uppercase",
                active && "bg-[var(--chalk)] text-[var(--carbon)] outline-2 outline-[var(--amber)]",
            )}
            onClick={onClick}
            onBlur={onBlur}
            onKeyDown={onKeyDown}
        >
            <RiKeyboardLine data-icon="inline-start"/>
            <span className="truncate">{active ? "按任意键录入..." : value || "点击录入"}</span>
        </Button>
    );
}


function RapidfireDisplayOverlay({groupId, isNativeShell}: { groupId: string; isNativeShell: boolean }) {
    const [bootstrap, setBootstrap] = useState<RapidfireBootstrap | null>(null);

    useRapidfireOverlayBootstrap(isNativeShell, setBootstrap);

    const runsById = useMemo(() => rapidfireRunsById(bootstrap?.runs ?? []), [bootstrap?.runs]);
    const group = bootstrap?.settings.groups?.find((item) => item.id === groupId);
    const enabledCards = bootstrap?.settings.cards.filter((card) => card.enabled && card.groupId === groupId && (group?.enabled ?? true)) ?? [];

    return (
        <div
            className="flex h-screen w-screen items-start justify-start overflow-hidden bg-transparent p-1 font-mono text-white">
            <div
                className="h-full w-full overflow-hidden rounded-md border border-white/15 bg-black/20 px-2.5 py-1.5 backdrop-blur-[1px]">
                {enabledCards.length === 0 ? (
                    <div
                        className="flex h-full items-center justify-center text-xs font-semibold text-white/60">连发器未启用</div>
                ) : (
                    enabledCards.map((card) => {
                        const run = runsById.get(card.id);
                        const statusText = run ? rapidfireStatusLabel(run.status) : "空闲";
                        const countText = run && run.status !== "idle" ? ` ×${run.count}` : "";

                        return (
                            <div key={card.id}
                                 className="flex min-w-0 items-center justify-between gap-2 py-0.5 text-sm font-semibold tracking-wide">
                <span className="flex min-w-0 items-center gap-1 truncate text-white/90">
                  <Kbd>{card.triggerKey}</Kbd>
                  <span className="text-white/50">→</span>
                  <Kbd>{card.targetKey}</Kbd>
                  <span className="truncate text-white/70">{card.name}</span>
                </span>
                                <span
                                    className={cn(
                                        "shrink-0",
                                        run?.status === "firing" && "text-green-300",
                                        run?.status === "pendingCompensation" && "text-yellow-300",
                                        (!run || run.status === "idle") && "text-white/55",
                                    )}
                                >
                  {statusText}{countText}
                </span>
                            </div>
                        );
                    })
                )}
            </div>
        </div>
    );
}

function RapidfirePositionOverlay({isNativeShell}: { isNativeShell: boolean }) {
    return (
        <PositionOverlay
            isNativeShell={isNativeShell}
            label="连发器"
            commands={{
                commit: "rapidfire_position_commit",
                cancel: "rapidfire_position_cancel",
                moved: "rapidfire_position_moved",
            }}
        />
    );
}

function useRapidfireOverlayBootstrap(isNativeShell: boolean, setBootstrap: (value: RapidfireBootstrap) => void) {
    useEffect(() => {
        document.body.dataset.overlayMode = "true";
        return () => {
            delete document.body.dataset.overlayMode;
        };
    }, []);

    useEffect(() => {
        if (!isNativeShell) return;

        let disposed = false;
        let unlistenStateChanged: (() => void) | undefined;

        void invoke<RapidfireBootstrap>("rapidfire_get_bootstrap").then((next) => {
            if (!disposed) setBootstrap(next);
        });

        void listen<RapidfireBootstrap>(RAPIDFIRE_EVENTS.stateChanged, (event) => {
            if (!disposed) setBootstrap(event.payload);
        }).then((dispose) => {
            unlistenStateChanged = dispose;
        });

        return () => {
            disposed = true;
            unlistenStateChanged?.();
        };
    }, [isNativeShell, setBootstrap]);
}

function RapidfireCardDragHandle({disabled, onDragStart}: { disabled: boolean; onDragStart: () => void }) {
    return (
        <Button
            aria-label="拖动排序"
            className="cursor-grab active:cursor-grabbing border-l-[3px] border-[var(--chalk)] pl-1"
            disabled={disabled}
            onPointerDown={(event) => {
                event.preventDefault();
                onDragStart();
            }}
            size="icon-sm"
            type="button"
            variant="ghost"
        >
            <span aria-hidden className="font-mono text-sm font-black leading-none text-[var(--chalk)]">≡</span>
        </Button>
    );
}

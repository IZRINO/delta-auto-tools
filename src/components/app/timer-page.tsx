import {useCallback, useEffect, useMemo, useRef, useState} from "react";
import {invokeLogged as invoke} from "@/lib/logging";
import {TIMER_EVENTS} from "@/lib/tauri-events";
import {subscribeTauriEvent} from "@/lib/tauri-listener";
import {RiAddLine, RiDeleteBinLine, RiStarFill, RiStarLine, RiTimerLine,} from "@remixicon/react";
import {toast} from "sonner";

import {Badge} from "@/components/ui/badge";
import {Button} from "@/components/ui/button";
import {Field, FieldContent, FieldError, FieldGroup, FieldLabel} from "@/components/ui/field";
import {Input} from "@/components/ui/input";
import {Select, SelectContent, SelectItem, SelectTrigger, SelectValue} from "@/components/ui/select";
import {Switch} from "@/components/ui/switch";
import {ToggleGroup, ToggleGroupItem} from "@/components/ui/toggle-group";
import {
  AppPage,
  CardBody,
  ControlTile,
  DragButton,
  HotkeyField,
  InlineControl,
  MacroHeader,
  runStateClass,
  SaveStateBadge,
  SectionHeader,
  SignalTile,
  StatusMatrix,
  SurfaceToggleGroup,
  TacticalCard,
} from "@/components/app/app-ui";
import {SyncCardList} from "@/components/app/sync-card-list";
import {SyncGroupSection} from "@/components/app/sync-group-section";
import {TimerDisplayOverlay, TimerPositionOverlay} from "@/components/app/sync-overlay-window";
import type {
  TimerBootstrap,
  TimerGroupForm,
  TimerItemForm,
  TimerRunState,
  TimerRunsChanged,
  TimerSelectionOutcome,
  TimerSettings,
  TimerSettingsForm,
} from "@/components/app/timer-types";
import {DEFAULT_TIMER_GROUP_ID, TIMER_AUTOSAVE_DELAY_MS} from "@/components/app/timer-types";
import {getErrorMessage} from "@/lib/error-utils";
import {useNativeShell} from "@/hooks/use-native-shell";
import {useAutosave} from "@/hooks/use-autosave";
import {useBootstrapForm} from "@/hooks/use-bootstrap-form";
import {useHotkeyRecorder} from "@/hooks/use-hotkey-recorder";
import {useHighlightScroll} from "@/hooks/use-highlight-scroll";
import {cn} from "@/lib/utils";
import {
  createTimerGroup,
  createTimerItem,
  formatTimerHotkey,
  moveTimerItem,
  parseTimerSettingsForm,
  timerEffectiveTimersByGroup,
  timerRunsById,
  timerSettingsToForm,
  timerSignalChar,
} from "@/components/app/timer-utils";
import {useFavorites} from "@/hooks/use-favorites";

const TIMER_BOOTSTRAP_SPEC = {
    getBootstrapCommand: "timer_get_bootstrap",
    saveSettingsCommand: "timer_save_settings",
    settingsToForm: timerSettingsToForm,
    parseSettingsForm: parseTimerSettingsForm,
};

export type TimerHighlightTarget = {
    kind: "timer";
    cardId: string;
    nonce: number;
};

type TimerPageProps = {
    overlayMode?: "display" | "position";
    highlightCardId?: TimerHighlightTarget | null;
};

export function TimerPage({overlayMode, highlightCardId}: TimerPageProps) {
    const isNativeShell = useNativeShell();
    const overlayGroupId = new URLSearchParams(window.location.search).get("groupId");

    if (overlayMode === "display") {
        return <TimerDisplayOverlay groupId={overlayGroupId ?? DEFAULT_TIMER_GROUP_ID} isNativeShell={isNativeShell}/>;
    }

    if (overlayMode === "position") {
        return <TimerPositionOverlay isNativeShell={isNativeShell}/>;
    }

    return <TimerWorkbench highlightCardId={highlightCardId ?? null} isNativeShell={isNativeShell}/>;
}

function TimerWorkbench({highlightCardId, isNativeShell}: {
    highlightCardId: TimerHighlightTarget | null;
    isNativeShell: boolean
}) {
    const bf = useBootstrapForm<TimerBootstrap, TimerSettings, TimerSettingsForm>({
        spec: TIMER_BOOTSTRAP_SPEC,
        isNativeShell,
        loadStatusMessage: "正在加载计时器设置...",
        readyStatusMessage: "计时器面板已就绪。配置阶段节奏、透明窗口与快捷键。",
        previewStatusMessage: "浏览器预览模式：当前仅验证布局，原生命令请在桌面端运行。",
        saveSuccessMessage: (next) => {
            return `计时器设置已保存（${next.settings.timerEnabled ? "开启" : "关闭"}）。`;
        },
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

    const [recordingTarget, setRecordingTarget] = useState<{ type: "timer"; id: string } | null>(null);
    const [runtimeRuns, setRuntimeRuns] = useState<TimerRunState[] | null>(null);
    const draggingTimerIdRef = useRef<string | null>(null);
    const [draggingTimerId, setDraggingTimerId] = useState<string | null>(null);
    const favorites = useFavorites();
    const recordingTargetRef = useRef<typeof recordingTarget>(null);
    recordingTargetRef.current = recordingTarget;

    const timerHighlight = highlightCardId && highlightCardId.kind === "timer" ? highlightCardId : null;

    useHighlightScroll(timerHighlight, "timer");

    useEffect(() => {
        const handlePointerUp = () => {
            draggingTimerIdRef.current = null;
            setDraggingTimerId(null);
        };

        window.addEventListener("pointerup", handlePointerUp);
        return () => window.removeEventListener("pointerup", handlePointerUp);
    }, []);

    useEffect(() => {
        if (!isNativeShell) {
            return;
        }

        let disposed = false;

        const unlistenStateChanged = subscribeTauriEvent<TimerBootstrap>(TIMER_EVENTS.stateChanged, (event) => {
            if (disposed) {
                return;
            }
            setBootstrap(event.payload);
        });

        const unlistenRunsChanged = subscribeTauriEvent<TimerRunsChanged>(
            TIMER_EVENTS.runsChanged,
            (event) => {
                if (!disposed) setRuntimeRuns(event.payload.runs);
            },
            undefined,
            () => {
                void invoke<TimerBootstrap>("timer_get_bootstrap").then((next) => {
                    if (!disposed) setRuntimeRuns((current) => current ?? next.runs);
                }, () => undefined);
            },
        );

        const unlistenHotkeyTriggered = subscribeTauriEvent<string[]>(TIMER_EVENTS.hotkeyTriggered, (event) => {
            if (disposed) {
                return;
            }
            setStatusMessage(`快捷键已触发 ${event.payload.length} 个计时器。运行中的计时器会忽略重复触发。`);
        });

        return () => {
            disposed = true;
            unlistenStateChanged();
            unlistenRunsChanged();
            unlistenHotkeyTriggered();
        };
    }, [isNativeShell, setBootstrap, setStatusMessage]);

    const runs = runtimeRuns ?? bootstrap?.runs ?? [];
    const runsById = useMemo(() => timerRunsById(runs), [runs]);
    const controlsDisabled = loading || !isNativeShell;

    const updateTimer = useCallback((id: string, value: Partial<TimerItemForm>) => {
        setForm((current) => {
            if (!current) {
                return current;
            }
            return {
                ...current,
                timers: current.timers.map((timer) => timer.id === id ? {...timer, ...value} : timer),
            };
        });
    }, []);

    const updateTimerGroup = useCallback((id: string, value: Partial<TimerGroupForm>) => {
        setForm((current) => current ? {
            ...current,
            timerGroups: current.timerGroups.map((group) => group.id === id ? {...group, ...value} : group),
        } : current);
    }, []);

    const updateTimerGroupDisplay = useCallback((id: string, value: Partial<TimerGroupForm["display"]>) => {
        setForm((current) => current ? {
            ...current,
            timerGroups: current.timerGroups.map((group) => group.id === id ? {
                ...group,
                display: {...group.display, ...value}
            } : group),
            display: id === DEFAULT_TIMER_GROUP_ID ? {...current.display, ...value} : current.display,
        } : current);
    }, []);

    const updateTimerGroupDisplayRect = useCallback((id: string, value: Partial<TimerGroupForm["display"]["rect"]>) => {
        setForm((current) => current ? {
            ...current,
            timerGroups: current.timerGroups.map((group) => group.id === id ? {
                ...group,
                display: {...group.display, rect: {...group.display.rect, ...value}}
            } : group),
            display: id === DEFAULT_TIMER_GROUP_ID ? {
                ...current.display,
                rect: {...current.display.rect, ...value}
            } : current.display,
        } : current);
    }, []);

    const recorder = useHotkeyRecorder({
        formatKey: formatTimerHotkey,
        onCommit: (key) => {
            const target = recordingTargetRef.current;
            if (!target) return;
            setRecordingTarget(null);
            updateTimer(target.id, {hotkey: key});
        },
        onCancel: (draft) => {
            const target = recordingTargetRef.current;
            if (!target) return;
            setRecordingTarget(null);
            setForm((current) => {
                if (!current) return current;
                return {
                    ...current,
                    timers: current.timers.map((timer) => timer.id === target.id ? {...timer, hotkey: draft} : timer)
                };
            });
        },
        onStatusMessage: setStatusMessage,
        keyRecordedMessage: (key) => `新的快捷键已录制：${key}`,
        recordingCancelledMessage: "已取消快捷键录制。",
    });

    useAutosave<TimerSettingsForm>({
        form,
        isDirty,
        disabled: !isNativeShell || loading || !bootstrap || !form || !!recordingTarget,
        onSave: (formSnapshot, nextVersion) => saveSettings(parseTimerSettingsForm(formSnapshot), nextVersion),
        onError: (message) => {
            setPageError(message);
            setStatusMessage(`保存失败：${message}`);
        },
        delay: TIMER_AUTOSAVE_DELAY_MS,
        autosaveVersionRef,
    });

    const beginTimerHotkeyRecording = useCallback((timer: TimerItemForm) => {
        setRecordingTarget({type: "timer", id: timer.id});
        recorder.beginRecording(timer.hotkey);
        setStatusMessage(`正在录制 ${timer.name || "计时器"} 的快捷键，按下主键会保存；失焦会取消。`);
    }, [recorder]);

    const handleTimerHotkeyRecorderKeyDown = useCallback((timer: TimerItemForm, event: React.KeyboardEvent<HTMLButtonElement>) => {
        if (recordingTarget?.type !== "timer" || recordingTarget.id !== timer.id) {
            return;
        }
        recorder.handleKeyDown(event);
    }, [recordingTarget, recorder]);

    const addTimer = useCallback(() => {
        setForm((current) => current ? {
            ...current,
            timers: [...current.timers, (() => {
                const groupId = current.timerGroups[0]?.id ?? DEFAULT_TIMER_GROUP_ID;
                return {
                    ...createTimerItem(current.timers.length, groupId),
                    groupId,
                    durationSeconds: "30",
                    segmentCount: ""
                };
            })()],
        } : current);
    }, []);

    const addTimerGroup = useCallback(() => {
        setForm((current) => current ? {
            ...current,
            timerGroups: [...current.timerGroups, createTimerGroup(current.timerGroups.length)],
        } : current);
    }, []);

    const removeTimerGroup = useCallback((groupId: string) => {
        setForm((current) => {
            if (!current) return current;
            if (current.timerGroups.length <= 1) {
                toast.info("至少保留一个计时器分组。");
                return current;
            }
            if (current.timers.some((timer) => timer.groupId === groupId)) {
                toast.info("请先把此分组内的计时器移动到其他分组。");
                return current;
            }
            return {
                ...current,
                timerGroups: current.timerGroups.filter((group) => group.id !== groupId),
            };
        });
    }, []);

    const removeTimer = useCallback((id: string) => {
        setForm((current) => {
            if (!current) {
                return current;
            }

            if (current.timers.length <= 1) {
                toast.info("至少保留一个计时器，无需删除最后一张。");
                return current;
            }

            return {
                ...current,
                timers: current.timers.filter((timer) => timer.id !== id),
            };
        });
    }, []);

    const moveTimer = useCallback((activeId: string, overId: string) => {
        setForm((current) => current ? {
            ...current,
            timers: moveTimerItem(current.timers, activeId, overId),
        } : current);
    }, []);

    const beginTimerDrag = useCallback((id: string) => {
        draggingTimerIdRef.current = id;
        setDraggingTimerId(id);
    }, []);

    const moveDraggingTimerOver = useCallback((overId: string) => {
        const activeId = draggingTimerIdRef.current;
        if (!activeId || activeId === overId) {
            return;
        }
        moveTimer(activeId, overId);
    }, [moveTimer]);

    const beginPositionSelection = useCallback(async (groupId?: string) => {
        if (!isNativeShell) {
            setStatusMessage("浏览器预览模式下不可设置透明窗口位置，请在桌面端使用。");
            return;
        }

        setStatusMessage("请在透明位置框中拖动窗口，按 Enter 保存，按 Esc 退出修改。透明窗口宽度可在上方调整。");

        try {
            const outcome = await invoke<TimerSelectionOutcome>("timer_begin_position_selection", {groupId});
            await syncBootstrap({syncForm: true});
            if (outcome.kind === "selected") {
                setStatusMessage("计时器透明窗口位置已保存。");
            } else if (outcome.kind === "cancelled") {
                setStatusMessage("计时器透明窗口位置修改已取消。");
            } else {
                setStatusMessage("计时器透明窗口位置设置窗口已关闭。");
            }
        } catch (error) {
            const message = getErrorMessage(error);
            setPageError(message);
            setStatusMessage(message);
        }
    }, [isNativeShell, syncBootstrap]);

    return (
        <AppPage className="auto-rows-max">
            <MacroHeader
                code="01"
                title="TIMER BOARD"
                verticalLabel="计时"
                subtitle="计时器负责阶段节奏。透明窗口、定位窗口与快捷键独立控制。"
                badges={
                    <>
                        <Badge
                            variant={form?.timerEnabled ? "default" : "secondary"}>计时通道{form?.timerEnabled ? "开启" : "关闭"}</Badge>
                        <SaveStateBadge dirty={isDirty} saving={saving}/>
                        {bootstrap?.hotkeyError ? <Badge variant="outline">快捷键异常</Badge> : null}
                    </>
                }
                actions={
                    <>
                        <SignalTile
                            label="计时矩阵"
                            value={form?.timers.length ?? 0}
                            detail={`${runs.filter((run) => run.status === "running").length} 个运行中`}
                        />
                        <SignalTile
                            label="保存信号"
                            value={saving ? "保存中" : isDirty ? "待保存" : "已保存"}
                            detail={statusMessage}
                        />
                    </>
                }
            />

            {pageError ? (
                <div className="col-span-12">
                    <FieldError>{pageError}</FieldError>
                </div>
            ) : null}

            <div className="col-span-12">
                <StatusMatrix items={[
                    {id: "timer", state: form?.timerEnabled ? "active" : "idle", label: "计时通道"},
                    {
                        id: "running",
                        state: runs.some((run) => run.status === "running") ? "active" : "idle",
                        label: "计时运行"
                    },
                    {
                        id: "hotkey",
                        state: bootstrap?.hotkeyError ? "error" : (form?.timerEnabled) ? "valid" : "idle",
                        label: "热键状态"
                    },
                    {id: "save", state: isDirty ? "warning" : "valid", label: "保存状态"},
                    {id: "ready", state: form?.timerEnabled ? "valid" : "idle", label: "就绪状态"},
                ]}/>
            </div>

            <TacticalCard className="col-span-12">
                <SectionHeader
                    eyebrow="总控字段"
                    icon={<RiTimerLine/>}
                    title="计时总控"
                    description="总开关控制计时器透明窗口与快捷键是否生效。"
                />
                <CardBody className="grid gap-3">
                    <div className="grid gap-px overflow-hidden rounded-box border border-base-300 bg-base-content xl:grid-cols-1">
                        <ControlTile className="flex items-center gap-3 rounded-none border-0 bg-base-200">
                            <Switch checked={Boolean(form?.timerEnabled)} disabled={controlsDisabled || !form}
                                    onCheckedChange={(checked) => updateForm("timerEnabled", checked)}/>
                            <div className="min-w-0">
                                <p className="font-mono text-xs font-medium text-base-content">计时总开关</p>
                                <p className="mt-1 text-xs text-muted-foreground">控制计时器快捷键与透明窗口输出。</p>
                            </div>
                        </ControlTile>
                    </div>
                    <InlineControl
                        className="font-mono text-xs font-medium text-base-content/60">
                        {statusMessage}
                    </InlineControl>
                </CardBody>
            </TacticalCard>

            <div className="col-span-12 h-0.5 bg-base-content"/>

            <SectionHeader
                className="col-span-12"
                eyebrow="CHANNEL 01"
                icon={<RiTimerLine/>}
                title="计时器系统"
                description="计时器负责阶段节奏。每张卡片配置独立计时方向、触发模式与快捷键。"
                actions={
                    <Button type="button" variant="outline" size="sm" disabled={controlsDisabled || !form}
                            onClick={addTimerGroup}>
                        <RiAddLine data-icon="inline-start"/>
                        新增分组
                    </Button>
                }
            />

            <SyncGroupSection
                groups={form?.timerGroups ?? []}
                targetLabel="计时器"
                controlsDisabled={controlsDisabled || !form?.timerEnabled}
                canDelete={(groupId) => Boolean(form && form.timerGroups.length > 1 && !form.timers.some((timer) => timer.groupId === groupId))}
                effectiveCount={(groupId) => timerEffectiveTimersByGroup(form, groupId).length}
                onGroupUpdate={updateTimerGroup}
                onGroupDelete={removeTimerGroup}
                onPositionSelection={(groupId) => void beginPositionSelection(groupId)}
                onUpdateDisplay={updateTimerGroupDisplay}
                onUpdateRect={updateTimerGroupDisplayRect}
            />

            <SyncCardList
                items={form?.timers ?? []}
                renderCard={(timer, index) => (
                    <TimerCard
                        key={timer.id}
                        controlsDisabled={controlsDisabled}
                        index={index}
                        isFavorite={favorites.isFavorite("timer", timer.id)}
                        isHighlighted={Boolean(timerHighlight && timerHighlight.cardId === timer.id)}
                        isRecording={recordingTarget?.type === "timer" && recordingTarget.id === timer.id}
                        isDragging={draggingTimerId === timer.id}
                        groupOptions={form?.timerGroups ?? []}
                        run={runsById.get(timer.id)}
                        timer={timer}
                        onDragOver={() => moveDraggingTimerOver(timer.id)}
                        onDragStart={() => beginTimerDrag(timer.id)}
                        onBeginHotkeyRecording={() => beginTimerHotkeyRecording(timer)}
                        onHotkeyKeyDown={(event) => handleTimerHotkeyRecorderKeyDown(timer, event)}
                        onHotkeyRecorderBlur={recorder.handleBlur}
                        onRemove={() => removeTimer(timer.id)}
                        onToggleFavorite={() => favorites.toggleFavorite("timer", timer.id)}
                        onUpdate={(value) => updateTimer(timer.id, value)}
                    />
                )}
                addButtonTitle="添加计时器"
                addButtonDescription="名称、秒数、计时方向、快捷键均可自定义。"
                onAdd={addTimer}
                disabled={controlsDisabled || !form}
            />
        </AppPage>
    );
}

type TimerCardProps = {
    controlsDisabled: boolean;
    groupOptions: TimerGroupForm[];
    index: number;
    isFavorite: boolean;
    isHighlighted: boolean;
    isDragging: boolean;
    isRecording: boolean;
    run: TimerRunState | undefined;
    timer: TimerItemForm;
    onBeginHotkeyRecording: () => void;
    onDragOver: () => void;
    onDragStart: () => void;
    onHotkeyKeyDown: (event: React.KeyboardEvent<HTMLButtonElement>) => void;
    onHotkeyRecorderBlur: () => void;
    onRemove: () => void;
    onToggleFavorite: () => void;
    onUpdate: (value: Partial<TimerItemForm>) => void;
};

function TimerCard({
                       controlsDisabled,
                       groupOptions,
                       index,
                       isDragging,
                       isFavorite,
                       isHighlighted,
                       isRecording,
                       onBeginHotkeyRecording,
                       onDragOver,
                       onDragStart,
                       onHotkeyKeyDown,
                       onHotkeyRecorderBlur,
                       onRemove,
                       onToggleFavorite,
                       onUpdate,
                       run,
                       timer
                   }: TimerCardProps) {
    const isMultiSegment = timer.segmentCount !== "" && Number.parseInt(timer.segmentCount, 10) >= 2;

    return (
        <TacticalCard active={isDragging}
                      className={cn(timer.enabled ? "" : "opacity-80", isHighlighted ? "outline-4 outline-primary" : "", runStateClass(run?.status))}
                      data-timer-card={timer.id} data-favorite-card={`timer:${timer.id}`} onPointerEnter={onDragOver}>
            <SectionHeader
                eyebrow="计时器"
                icon={<RiTimerLine/>}
                title={(
                    <Input
                        className="h-auto w-full border-0 bg-transparent p-0 font-heading text-lg font-medium text-base-content placeholder:text-base-content/40 focus-visible:ring-0 focus-visible:ring-offset-0"
                        placeholder="输入卡片名称"
                        value={timer.name || "计时器"}
                        disabled={controlsDisabled}
                        onChange={(event) => onUpdate({name: event.currentTarget.value})}
                        aria-label="计时器名称"
                    />
                )}
                description={run ? `${timerSignalChar(timer, run)} ${Math.floor(run.currentSeconds)}s` : (timer.enabled ? "▢ 等待触发" : "○ 已禁用")}
                actions={(
                    <div className="flex items-center gap-1.5">
                        <Select disabled={controlsDisabled} value={timer.groupId}
                                onValueChange={(value) => onUpdate({groupId: value})}>
                            <SelectTrigger className="w-32 bg-base-100">
                                <SelectValue placeholder="分组"/>
                            </SelectTrigger>
                            <SelectContent>
                                {groupOptions.map((group) => (
                                    <SelectItem key={group.id} value={group.id}>{group.name}</SelectItem>
                                ))}
                            </SelectContent>
                        </Select>
                        <DragButton controlsDisabled={controlsDisabled} onDragStart={onDragStart}/>
                        <Button aria-label={isFavorite ? "取消收藏" : "加入收藏"} aria-pressed={isFavorite}
                                className={cn(isFavorite ? "text-primary" : "text-muted-foreground")}
                                disabled={controlsDisabled} onClick={onToggleFavorite} size="icon-sm" type="button"
                                variant="outline">
                            {isFavorite ? <RiStarFill/> : <RiStarLine/>}
                        </Button>
                        <Switch checked={timer.enabled} disabled={controlsDisabled} aria-label="启用计时器"
                                onCheckedChange={(checked) => onUpdate({enabled: checked})}/>
                        <Button disabled={controlsDisabled} onClick={onRemove} size="icon-sm" type="button"
                                variant="outline" aria-label="删除计时器">
                            <RiDeleteBinLine/>
                        </Button>
                    </div>
                )}
                badge={<Badge variant="outline">{String(index + 1).padStart(2, "0")}</Badge>}
            />
            <CardBody>
                <FieldGroup className="grid gap-4 sm:grid-cols-2">
                    <Field>
                        <FieldLabel htmlFor={`${timer.id}-duration`}>每段秒数</FieldLabel>
                        <FieldContent>
                            <Input id={`${timer.id}-duration`} disabled={controlsDisabled} inputMode="numeric" min="1"
                                   value={timer.durationSeconds}
                                   onChange={(event) => onUpdate({durationSeconds: event.currentTarget.value})}/>
                        </FieldContent>
                    </Field>
                    <Field>
                        <FieldLabel>计时方向</FieldLabel>
                        <FieldContent>
                            <SurfaceToggleGroup>
                                <ToggleGroup className="w-full" disabled={controlsDisabled} type="single"
                                             value={timer.direction} variant="outline"
                                             onValueChange={(value) => value ? onUpdate({direction: value as TimerItemForm["direction"]}) : undefined}>
                                    <ToggleGroupItem
                                        className="min-w-24 flex-1 border-base-content font-mono text-sm font-semibold data-[state=on]:bg-base-content data-[state=on]:text-base-100"
                                        value="countup">正</ToggleGroupItem>
                                    <ToggleGroupItem
                                        className="min-w-24 flex-1 border-base-content font-mono text-sm font-semibold data-[state=on]:bg-base-content data-[state=on]:text-base-100"
                                        value="countdown">反</ToggleGroupItem>
                                </ToggleGroup>
                            </SurfaceToggleGroup>
                        </FieldContent>
                    </Field>
                    <Field>
                        <FieldLabel>触发模式</FieldLabel>
                        <FieldContent>
                            <SurfaceToggleGroup>
                                <ToggleGroup className="w-full" disabled={controlsDisabled} type="single"
                                             value={timer.triggerMode} variant="outline"
                                             onValueChange={(value) => value ? onUpdate({triggerMode: value as TimerItemForm["triggerMode"]}) : undefined}>
                                    <ToggleGroupItem
                                        className="min-w-24 flex-1 border-base-content font-mono text-sm font-semibold data-[state=on]:bg-base-content data-[state=on]:text-base-100"
                                        value="press">按下</ToggleGroupItem>
                                    <ToggleGroupItem
                                        className="min-w-24 flex-1 border-base-content font-mono text-sm font-semibold data-[state=on]:bg-base-content data-[state=on]:text-base-100"
                                        value="release">释放</ToggleGroupItem>
                                </ToggleGroup>
                            </SurfaceToggleGroup>
                        </FieldContent>
                    </Field>
                    <Field>
                        <FieldLabel htmlFor={`${timer.id}-segment-count`}>多段数（留空=单段）</FieldLabel>
                        <FieldContent>
                            <Input id={`${timer.id}-segment-count`} disabled={controlsDisabled} inputMode="numeric"
                                   min="2" max="99" placeholder="留空为普通单段计时器" value={timer.segmentCount}
                                   onChange={(event) => onUpdate({segmentCount: event.currentTarget.value})}/>
                        </FieldContent>
                        {isMultiSegment ? (
                            <p className="text-xs text-muted-foreground">总时长 {Number.parseInt(timer.durationSeconds, 10) * Number.parseInt(timer.segmentCount, 10)} 秒，每次触发减少 {timer.durationSeconds} 秒</p>
                        ) : null}
                    </Field>
                    <ControlTile className="flex items-center gap-3 sm:col-span-2">
                        <Switch
                            checked={timer.ignoreRunning}
                            disabled={controlsDisabled}
                            onCheckedChange={(checked) => onUpdate({ignoreRunning: checked})}
                        />
                        <div className="min-w-0">
                            <p className="text-sm font-medium text-foreground">运行中忽略触发</p>
                            <p className="mt-1 text-xs text-muted-foreground">开启后运行时快捷键无效；关闭后运行时触发会重置计时器。</p>
                        </div>
                    </ControlTile>
                    <div className="sm:col-span-2">
                        <HotkeyField controlsDisabled={controlsDisabled} id={`${timer.id}-hotkey`}
                                     isRecording={isRecording} hotkey={timer.hotkey}
                                     onBeginHotkeyRecording={onBeginHotkeyRecording} onHotkeyKeyDown={onHotkeyKeyDown}
                                     onHotkeyRecorderBlur={onHotkeyRecorderBlur}/>
                    </div>

                </FieldGroup>
            </CardBody>
        </TacticalCard>
    );
}

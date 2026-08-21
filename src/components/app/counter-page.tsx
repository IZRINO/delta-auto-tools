import {useCallback, useEffect, useMemo, useRef, useState} from "react";
import {invokeLogged as invoke} from "@/lib/logging";
import {COUNTER_EVENTS} from "@/lib/tauri-events";
import {subscribeTauriEvent} from "@/lib/tauri-listener";
import {
  RiAddLine,
  RiDeleteBinLine,
  RiResetLeftLine,
  RiSubtractLine,
} from "@remixicon/react";
import {toast} from "sonner";

import {Badge} from "@/components/ui/badge";
import {Button} from "@/components/ui/button";
import {Input} from "@/components/ui/input";
import {Select, SelectContent, SelectItem, SelectTrigger, SelectValue} from "@/components/ui/select";
import {Switch} from "@/components/ui/switch";
import {
  CardNameInput,
  ConfigRow,
  DragButton,
  FavoriteButton,
  FieldUnit,
  HotkeyField,
  MasterSwitchCard,
  SoftAlert,
  ToolPageFrame,
} from "@/components/app/app-ui";
import {SyncCardList} from "@/components/app/sync-card-list";
import {SyncGroupSection} from "@/components/app/sync-group-section";
import type {
  CounterBootstrap,
  CounterItemForm,
  CounterRunState,
  CounterRunsChanged,
  CounterSelectionOutcome,
  CounterSettings,
  CounterSettingsForm,
  TimerGroupForm,
} from "@/components/app/timer-types";
import {DEFAULT_COUNTER_GROUP_ID, TIMER_AUTOSAVE_DELAY_MS} from "@/components/app/timer-types";
import {getErrorMessage} from "@/lib/error-utils";
import {useNativeShell} from "@/hooks/use-native-shell";
import {useAutosave} from "@/hooks/use-autosave";
import {useBootstrapForm} from "@/hooks/use-bootstrap-form";
import {useHotkeyRecorder} from "@/hooks/use-hotkey-recorder";
import {useHighlightScroll} from "@/hooks/use-highlight-scroll";
import {cn} from "@/lib/utils";
import {
  counterEffectiveByGroup,
  counterRunsById,
  counterSettingsToForm,
  createCounterGroup,
  createCounterItem,
  formatTimerHotkey,
  moveCounterItem,
  parseCounterSettingsForm,
} from "@/components/app/counter-utils";
import {useFavorites} from "@/hooks/use-favorites";
import {CounterDisplayOverlay, CounterPositionOverlay} from "@/components/app/sync-overlay-window";

const COUNTER_BOOTSTRAP_SPEC = {
    getBootstrapCommand: "counter_get_bootstrap",
    saveSettingsCommand: "counter_save_settings",
    settingsToForm: counterSettingsToForm,
    parseSettingsForm: parseCounterSettingsForm,
};

export type CounterHighlightTarget = {
    kind: "counter";
    cardId: string;
    /** nonce 用于强制重触发高亮动画（用户重复点击同一卡片） */
    nonce: number;
};

type CounterPageProps = {
    overlayMode?: "counter-display" | "counter-position";
    highlightCardId?: CounterHighlightTarget | null;
};

export function CounterPage({overlayMode, highlightCardId}: CounterPageProps) {
    const isNativeShell = useNativeShell();
    const overlayGroupId = new URLSearchParams(window.location.search).get("groupId");

    if (overlayMode === "counter-display") {
        return <CounterDisplayOverlay groupId={overlayGroupId ?? DEFAULT_COUNTER_GROUP_ID}
                                      isNativeShell={isNativeShell}/>;
    }

    if (overlayMode === "counter-position") {
        return <CounterPositionOverlay isNativeShell={isNativeShell}/>;
    }

    return <CounterWorkbench highlightCardId={highlightCardId ?? null} isNativeShell={isNativeShell}/>;
}

function CounterWorkbench({highlightCardId, isNativeShell}: {
    highlightCardId: CounterHighlightTarget | null;
    isNativeShell: boolean
}) {
    const bf = useBootstrapForm<CounterBootstrap, CounterSettings, CounterSettingsForm>({
        spec: COUNTER_BOOTSTRAP_SPEC,
        isNativeShell,
        loadStatusMessage: "正在加载计数器设置...",
        readyStatusMessage: "计数器面板已就绪。每张卡片有独立计数状态与快捷键。",
        previewStatusMessage: "浏览器预览模式：当前仅验证布局，原生命令请在桌面端运行。",
        saveSuccessMessage: (next) => {
            const counterMsg = next.settings.counterEnabled ? "计数器开启" : "计数器关闭";
            return `计数器设置已保存（${counterMsg}）。`;
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
        pageError,
        setPageError,
        setStatusMessage,
        autosaveVersionRef
    } = bf;

    const [recordingTarget, setRecordingTarget] = useState<{ type: "counter"; id: string } | null>(null);
    const [runtimeRuns, setRuntimeRuns] = useState<CounterRunState[] | null>(null);
    const draggingCounterIdRef = useRef<string | null>(null);
    const [draggingCounterId, setDraggingCounterId] = useState<string | null>(null);
    const favorites = useFavorites();
    const recordingTargetRef = useRef<typeof recordingTarget>(null);
    recordingTargetRef.current = recordingTarget;

    const counterHighlight = highlightCardId && highlightCardId.kind === "counter" ? highlightCardId : null;

    useHighlightScroll(counterHighlight, "counter");

    useEffect(() => {
        const handlePointerUp = () => {
            draggingCounterIdRef.current = null;
            setDraggingCounterId(null);
        };

        window.addEventListener("pointerup", handlePointerUp);
        return () => window.removeEventListener("pointerup", handlePointerUp);
    }, []);

    useEffect(() => {
        if (!isNativeShell) {
            return;
        }

        let disposed = false;

        const unlistenStateChanged = subscribeTauriEvent<CounterBootstrap>(COUNTER_EVENTS.stateChanged, (event) => {
            if (disposed) {
                return;
            }
            setBootstrap(event.payload);
        });

        const unlistenRunsChanged = subscribeTauriEvent<CounterRunsChanged>(
            COUNTER_EVENTS.runsChanged,
            (event) => {
                if (!disposed) setRuntimeRuns(event.payload.counterRuns);
            },
            undefined,
            () => {
                void invoke<CounterBootstrap>("counter_get_bootstrap").then((next) => {
                    if (!disposed) setRuntimeRuns((current) => current ?? next.counterRuns);
                }, () => undefined);
            },
        );

        const unlistenCounterTriggered = subscribeTauriEvent<string[]>(COUNTER_EVENTS.hotkeyTriggered, (event) => {
            if (disposed) {
                return;
            }
            setStatusMessage(`快捷键已触发 ${event.payload.length} 个计数器。`);
        });

        return () => {
            disposed = true;
            unlistenStateChanged();
            unlistenRunsChanged();
            unlistenCounterTriggered();
        };
    }, [isNativeShell]);

    const runs = runtimeRuns ?? bootstrap?.counterRuns ?? [];
    const counterRunsByIdMap = useMemo(() => counterRunsById(runs), [runs]);
    const controlsDisabled = loading || !isNativeShell;

    const updateCounter = useCallback((id: string, value: Partial<CounterItemForm>) => {
        setForm((current) => current ? {
            ...current,
            counters: current.counters.map((counter) => counter.id === id ? {...counter, ...value} : counter),
        } : current);
    }, []);

    const updateCounterGroup = useCallback((id: string, value: Partial<TimerGroupForm>) => {
        setForm((current) => current ? {
            ...current,
            counterGroups: current.counterGroups.map((group) => group.id === id ? {...group, ...value} : group),
        } : current);
    }, []);

    const updateCounterGroupDisplay = useCallback((id: string, value: Partial<TimerGroupForm["display"]>) => {
        setForm((current) => current ? {
            ...current,
            counterGroups: current.counterGroups.map((group) => group.id === id ? {
                ...group,
                display: {...group.display, ...value}
            } : group),
            display: id === DEFAULT_COUNTER_GROUP_ID ? {...current.display, ...value} : current.display,
        } : current);
    }, []);

    const updateCounterGroupDisplayRect = useCallback((id: string, value: Partial<TimerGroupForm["display"]["rect"]>) => {
        setForm((current) => current ? {
            ...current,
            counterGroups: current.counterGroups.map((group) => group.id === id ? {
                ...group,
                display: {...group.display, rect: {...group.display.rect, ...value}}
            } : group),
            display: id === DEFAULT_COUNTER_GROUP_ID ? {
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
            updateCounter(target.id, {hotkey: key});
        },
        onCancel: (draft) => {
            const target = recordingTargetRef.current;
            if (!target) return;
            setRecordingTarget(null);
            setForm((current) => {
                if (!current) return current;
                return {
                    ...current,
                    counters: current.counters.map((counter) => counter.id === target.id ? {
                        ...counter,
                        hotkey: draft
                    } : counter)
                };
            });
        },
        onStatusMessage: setStatusMessage,
        keyRecordedMessage: (key) => `新的快捷键已录制：${key}`,
        recordingCancelledMessage: "已取消快捷键录制。",
    });

    useAutosave<CounterSettingsForm>({
        form,
        isDirty,
        disabled: !isNativeShell || loading || !bootstrap || !form || !!recordingTarget,
        onSave: (formSnapshot, nextVersion) => saveSettings(parseCounterSettingsForm(formSnapshot), nextVersion),
        onError: (message) => {
            setPageError(message);
            setStatusMessage(`保存失败：${message}`);
        },
        delay: TIMER_AUTOSAVE_DELAY_MS,
        autosaveVersionRef,
    });

    const beginCounterHotkeyRecording = useCallback((counter: CounterItemForm) => {
        setRecordingTarget({type: "counter", id: counter.id});
        recorder.beginRecording(counter.hotkey);
        setStatusMessage(`正在录制 ${counter.name || "计数器"} 的快捷键，按下主键会保存；失焦会取消。`);
    }, [recorder]);

    const handleCounterHotkeyRecorderKeyDown = useCallback((counter: CounterItemForm, event: React.KeyboardEvent<HTMLButtonElement>) => {
        if (recordingTarget?.type !== "counter" || recordingTarget.id !== counter.id) {
            return;
        }
        recorder.handleKeyDown(event);
    }, [recordingTarget, recorder]);

    const addCounter = useCallback(() => {
        setForm((current) => current ? {
            ...current,
            counters: [...current.counters, (() => {
                const groupId = current.counterGroups[0]?.id ?? DEFAULT_COUNTER_GROUP_ID;
                return {...createCounterItem(current.counters.length, groupId), groupId, startValue: "0"};
            })()],
        } : current);
    }, []);

    const addCounterGroup = useCallback(() => {
        setForm((current) => current ? {
            ...current,
            counterGroups: [...current.counterGroups, createCounterGroup(current.counterGroups.length)],
        } : current);
    }, []);

    const removeCounterGroup = useCallback((groupId: string) => {
        setForm((current) => {
            if (!current) return current;
            if (current.counterGroups.length <= 1) {
                toast.info("至少保留一个计数器分组。");
                return current;
            }
            if (current.counters.some((counter) => counter.groupId === groupId)) {
                toast.info("请先把此分组内的计数器移动到其他分组。");
                return current;
            }
            return {
                ...current,
                counterGroups: current.counterGroups.filter((group) => group.id !== groupId),
            };
        });
    }, []);

    const removeCounter = useCallback((id: string) => {
        setForm((current) => {
            if (!current) {
                return current;
            }

            if (current.counters.length <= 1) {
                toast.info("至少保留一个计数器，无需删除最后一张。");
                return current;
            }

            return {
                ...current,
                counters: current.counters.filter((counter) => counter.id !== id),
            };
        });
    }, []);

    const moveCounter = useCallback((activeId: string, overId: string) => {
        setForm((current) => current ? {
            ...current,
            counters: moveCounterItem(current.counters, activeId, overId),
        } : current);
    }, []);

    const beginCounterDrag = useCallback((id: string) => {
        draggingCounterIdRef.current = id;
        setDraggingCounterId(id);
    }, []);

    const moveDraggingCounterOver = useCallback((overId: string) => {
        const activeId = draggingCounterIdRef.current;
        if (!activeId || activeId === overId) {
            return;
        }
        moveCounter(activeId, overId);
    }, [moveCounter]);

    const beginPositionSelection = useCallback(async (groupId?: string) => {
        if (!isNativeShell) {
            setStatusMessage("浏览器预览模式下不可设置透明窗口位置，请在桌面端使用。");
            return;
        }

        setStatusMessage("请在透明位置框中拖动窗口，按 Enter 保存，按 Esc 退出修改。透明窗口宽度可在上方调整。");

        try {
            const outcome = await invoke<CounterSelectionOutcome>("counter_begin_position_selection", {groupId});
            await syncBootstrap({syncForm: true});
            if (outcome.kind === "selected") {
                setStatusMessage("计数器透明窗口位置已保存。");
            } else if (outcome.kind === "cancelled") {
                setStatusMessage("计数器透明窗口位置修改已取消。");
            } else {
                setStatusMessage("计数器透明窗口位置设置窗口已关闭。");
            }
        } catch (error) {
            const message = getErrorMessage(error);
            setPageError(message);
            setStatusMessage(message);
        }
    }, [isNativeShell, syncBootstrap]);

    const resetCounter = useCallback(async (counterId: string) => {
        if (!isNativeShell) {
            setStatusMessage("浏览器预览模式下不可重置计数器，请在桌面端使用。");
            return;
        }

        try {
            const next = await invoke<CounterBootstrap>("counter_reset", {counterId});
            setBootstrap(next);
            setStatusMessage("计数器已重置为设置的起始数。");
        } catch (error) {
            const message = getErrorMessage(error);
            setPageError(message);
            setStatusMessage(message);
        }
    }, [isNativeShell]);

    const adjustCounter = useCallback(async (counterId: string, delta: number) => {
        if (!isNativeShell) {
            setStatusMessage("浏览器预览模式下不可调整计数器，请在桌面端使用。");
            return;
        }

        try {
            const next = await invoke<CounterBootstrap>("counter_adjust", {counterId, delta});
            setBootstrap(next);
            setStatusMessage(delta > 0 ? `计数器已加 ${delta}。` : `计数器已减 ${Math.abs(delta)}。`);
        } catch (error) {
            const message = getErrorMessage(error);
            setPageError(message);
            setStatusMessage(message);
        }
    }, [isNativeShell]);

    return (
        <ToolPageFrame
            actions={bootstrap?.hotkeyError ? <Badge variant="outline">快捷键异常</Badge> : undefined}
            error={pageError ? <SoftAlert>{pageError}</SoftAlert> : undefined}
            title="计数器"
        >
            <MasterSwitchCard
                ariaLabel="计数器总开关"
                checked={Boolean(form?.counterEnabled)}
                disabled={controlsDisabled || !form}
                label={form?.counterEnabled ? "开" : "关"}
                onCheckedChange={(checked) => updateForm("counterEnabled", checked)}
            />

            <div className="col-span-12 flex justify-end">
                <Button type="button" variant="outline" size="sm" disabled={controlsDisabled || !form}
                        onClick={addCounterGroup}>
                    <RiAddLine data-icon="inline-start"/>
                    新增分组
                </Button>
            </div>

            <SyncGroupSection
                groups={form?.counterGroups ?? []}
                targetLabel="计数器"
                controlsDisabled={controlsDisabled || !form?.counterEnabled}
                canDelete={(groupId) => Boolean(form && form.counterGroups.length > 1 && !form.counters.some((counter) => counter.groupId === groupId))}
                effectiveCount={(groupId) => counterEffectiveByGroup(form, groupId).length}
                onGroupUpdate={updateCounterGroup}
                onGroupDelete={removeCounterGroup}
                onPositionSelection={(groupId) => void beginPositionSelection(groupId)}
                onUpdateDisplay={updateCounterGroupDisplay}
                onUpdateRect={updateCounterGroupDisplayRect}
            />

            <SyncCardList
                items={form?.counters ?? []}
                renderCard={(counter, index) => (
                    <CounterCard
                        key={counter.id}
                        controlsDisabled={controlsDisabled}
                        counter={counter}
                        index={index}
                        isFavorite={favorites.isFavorite("counter", counter.id)}
                        isHighlighted={Boolean(counterHighlight && counterHighlight.cardId === counter.id)}
                        isDragging={draggingCounterId === counter.id}
                        isRecording={recordingTarget?.type === "counter" && recordingTarget.id === counter.id}
                        groupOptions={form?.counterGroups ?? []}
                        run={counterRunsByIdMap.get(counter.id)}
                        onAdjust={(delta) => void adjustCounter(counter.id, delta)}
                        onBeginHotkeyRecording={() => beginCounterHotkeyRecording(counter)}
                        onDragOver={() => moveDraggingCounterOver(counter.id)}
                        onDragStart={() => beginCounterDrag(counter.id)}
                        onHotkeyKeyDown={(event) => handleCounterHotkeyRecorderKeyDown(counter, event)}
                        onHotkeyRecorderBlur={recorder.handleBlur}
                        onRemove={() => removeCounter(counter.id)}
                        onReset={() => void resetCounter(counter.id)}
                        resetDisabled={controlsDisabled || !form?.counterEnabled}
                        onToggleFavorite={() => favorites.toggleFavorite("counter", counter.id)}
                        onUpdate={(value) => updateCounter(counter.id, value)}
                    />
                )}
                addButtonTitle="添加计数器"
                onAdd={addCounter}
                disabled={controlsDisabled || !form}
            />
        </ToolPageFrame>
    );
}

type CounterCardProps = {
    controlsDisabled: boolean;
    counter: CounterItemForm;
    groupOptions: TimerGroupForm[];
    index: number;
    isFavorite: boolean;
    isHighlighted: boolean;
    isDragging: boolean;
    isRecording: boolean;
    run: CounterRunState | undefined;
    onAdjust: (delta: number) => void;
    onBeginHotkeyRecording: () => void;
    onDragOver: () => void;
    onDragStart: () => void;
    onHotkeyKeyDown: (event: React.KeyboardEvent<HTMLButtonElement>) => void;
    onHotkeyRecorderBlur: () => void;
    onRemove: () => void;
    onReset: () => void;
    resetDisabled: boolean;
    onToggleFavorite: () => void;
    onUpdate: (value: Partial<CounterItemForm>) => void;
};

function CounterCard({
                         controlsDisabled,
                         counter,
                         groupOptions,
                         index,
                         isDragging,
                         isFavorite,
                         isHighlighted,
                         isRecording,
                         onAdjust,
                         onBeginHotkeyRecording,
                         onDragOver,
                         onDragStart,
                         onHotkeyKeyDown,
                         onHotkeyRecorderBlur,
                         onRemove,
                         onReset,
                         onToggleFavorite,
                         onUpdate,
                         resetDisabled,
                         run
                     }: CounterCardProps) {
    return (
        <FieldUnit
            padBody={false}
            className={cn(counter.enabled ? "" : "opacity-80", isHighlighted ? "outline-4 outline-primary" : "", isDragging && "ring-2 ring-primary")}
            data-counter-card={counter.id}
            data-favorite-card={`counter:${counter.id}`}
            onPointerEnter={onDragOver}
            header={(
                <CardNameInput
                    ariaLabel="计数器名称"
                    disabled={controlsDisabled}
                    fallback="计数器"
                    onChange={(name) => onUpdate({name})}
                    value={counter.name}
                />
            )}
            description={`当前计数 · ${run?.value ?? counter.startValue}`}
            headerActions={(
                <>
                    <Badge variant="outline">{String(index + 1).padStart(2, "0")}</Badge>
                    <Select disabled={controlsDisabled} value={counter.groupId}
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
                    <FavoriteButton disabled={controlsDisabled} isFavorite={isFavorite} onClick={onToggleFavorite}/>
                    <Switch checked={counter.enabled} disabled={controlsDisabled} aria-label="启用计数器"
                            onCheckedChange={(checked) => onUpdate({enabled: checked})}/>
                    <Button disabled={controlsDisabled} onClick={onRemove} size="icon-sm" type="button"
                            variant="outline" aria-label="删除计数器">
                        <RiDeleteBinLine/>
                    </Button>
                </>
            )}
            footer={(
                <div className="flex flex-wrap gap-2">
                    <Button
                        className="flex-1"
                        disabled={resetDisabled}
                        onClick={() => onAdjust(-1)}
                        type="button"
                        variant="outline"
                    >
                        <RiSubtractLine data-icon="inline-start"/>
                        -1
                    </Button>
                    <Button
                        className="flex-1"
                        disabled={resetDisabled}
                        onClick={() => onAdjust(1)}
                        type="button"
                        variant="outline"
                    >
                        <RiAddLine data-icon="inline-start"/>
                        +1
                    </Button>
                    <Button className="flex-1" disabled={resetDisabled} onClick={onReset} type="button"
                            variant="outline">
                        <RiResetLeftLine data-icon="inline-start"/>
                        重置为起始数
                    </Button>
                </div>
            )}
        >
            <ConfigRow
                label="起始数"
                value={(
                    <Input id={`${counter.id}-start`} className="w-28" disabled={controlsDisabled} inputMode="numeric"
                           value={counter.startValue}
                           onChange={(event) => onUpdate({startValue: event.currentTarget.value})}/>
                )}
            />
            <ConfigRow
                label="快捷键"
                value={(
                    <HotkeyField labeled={false} controlsDisabled={controlsDisabled} id={`${counter.id}-hotkey`}
                                 isRecording={isRecording} hotkey={counter.hotkey}
                                 onBeginHotkeyRecording={onBeginHotkeyRecording} onHotkeyKeyDown={onHotkeyKeyDown}
                                 onHotkeyRecorderBlur={onHotkeyRecorderBlur}/>
                )}
            />
        </FieldUnit>
    );
}

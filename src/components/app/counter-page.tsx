import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listenEvent, COUNTER_EVENTS } from "@/lib/tauri-events";
import {
  RiAddLine,
  RiDeleteBinLine,
  RiResetLeftLine,
  RiSpeedUpLine,
  RiStarFill,
  RiStarLine,
  RiSubtractLine,
} from "@remixicon/react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { CardHeader } from "@/components/ui/card";
import { Field, FieldContent, FieldError, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { HotkeyField } from "@/components/app/app-ui";
import { SyncCardList } from "@/components/app/sync-card-list";
import { SyncGroupSection } from "@/components/app/sync-group-section";
import type {
  CounterBootstrap,
  CounterSettings,
  CounterSettingsForm,
  CounterItemForm,
  CounterRunState,
  CounterSelectionOutcome,
  TimerGroupForm,
} from "@/components/app/timer-types";
import { DEFAULT_COUNTER_GROUP_ID, TIMER_AUTOSAVE_DELAY_MS } from "@/components/app/timer-types";
import { getErrorMessage } from "@/lib/error-utils";
import { useNativeShell } from "@/hooks/use-native-shell";
import { useAutosave } from "@/hooks/use-autosave";
import { useBootstrapForm } from "@/hooks/use-bootstrap-form";
import { useHotkeyRecorder } from "@/hooks/use-hotkey-recorder";
import { useHighlightScroll } from "@/hooks/use-highlight-scroll";
import { cn } from "@/lib/utils";
import {
  formatTimerHotkey,
  createCounterGroup,
  createCounterItem,
  counterEffectiveByGroup,
  moveCounterItem,
  counterRunsById,
  counterSettingsToForm,
  parseCounterSettingsForm,
} from "@/components/app/counter-utils";
import { useFavorites } from "@/hooks/use-favorites";
import {
  AppPage,
  CardBody,
  ControlTile,
  DragButton,
  InlineControl,
  MacroHeader,
  PagePreviewBanner,
  SaveStateBadge,
  SectionHeader,
  SignalTile,
  StatusMatrix,
  TacticalCard,
} from "@/components/app/app-ui";
import { CounterDisplayOverlay, CounterPositionOverlay } from "@/components/app/sync-overlay-window";

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

export function CounterPage({ overlayMode, highlightCardId }: CounterPageProps) {
  const isNativeShell = useNativeShell();
  const overlayGroupId = new URLSearchParams(window.location.search).get("groupId");

  if (overlayMode === "counter-display") {
    return <CounterDisplayOverlay groupId={overlayGroupId ?? DEFAULT_COUNTER_GROUP_ID} isNativeShell={isNativeShell} />;
  }

  if (overlayMode === "counter-position") {
    return <CounterPositionOverlay isNativeShell={isNativeShell} />;
  }

  return <CounterWorkbench highlightCardId={highlightCardId ?? null} isNativeShell={isNativeShell} />;
}

function CounterWorkbench({ highlightCardId, isNativeShell }: { highlightCardId: CounterHighlightTarget | null; isNativeShell: boolean }) {
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

  const { bootstrap, setBootstrap, form, setForm, isDirty, updateForm, saveSettings, syncBootstrap, loading, saving, pageError, setPageError, statusMessage, setStatusMessage, autosaveVersionRef } = bf;

  const [recordingTarget, setRecordingTarget] = useState<{ type: "counter"; id: string } | null>(null);
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
    let unlistenStateChanged: (() => void) | undefined;
    let unlistenCounterTriggered: (() => void) | undefined;

    void listenEvent(COUNTER_EVENTS.stateChanged, (event) => {
      if (disposed) {
        return;
      }
      setBootstrap(event.payload);
    }).then((dispose) => {
      unlistenStateChanged = dispose;
    });

    void listenEvent(COUNTER_EVENTS.hotkeyTriggered, (event) => {
      if (disposed) {
        return;
      }
      setStatusMessage(`快捷键已触发 ${event.payload.length} 个计数器。`);
    }).then((dispose) => {
      unlistenCounterTriggered = dispose;
    });

    return () => {
      disposed = true;
      unlistenStateChanged?.();
      unlistenCounterTriggered?.();
    };
  }, [isNativeShell]);

  const counterRunsByIdMap = useMemo(() => counterRunsById(bootstrap?.counterRuns ?? []), [bootstrap?.counterRuns]);
  const controlsDisabled = loading || !isNativeShell;

  const updateCounter = useCallback((id: string, value: Partial<CounterItemForm>) => {
    setForm((current) => current ? {
      ...current,
      counters: current.counters.map((counter) => counter.id === id ? { ...counter, ...value } : counter),
    } : current);
  }, []);

  const updateCounterGroup = useCallback((id: string, value: Partial<TimerGroupForm>) => {
    setForm((current) => current ? {
      ...current,
      counterGroups: current.counterGroups.map((group) => group.id === id ? { ...group, ...value } : group),
    } : current);
  }, []);

  const updateCounterGroupDisplay = useCallback((id: string, value: Partial<TimerGroupForm["display"]>) => {
    setForm((current) => current ? {
      ...current,
      counterGroups: current.counterGroups.map((group) => group.id === id ? { ...group, display: { ...group.display, ...value } } : group),
      display: id === DEFAULT_COUNTER_GROUP_ID ? { ...current.display, ...value } : current.display,
    } : current);
  }, []);

  const updateCounterGroupDisplayRect = useCallback((id: string, value: Partial<TimerGroupForm["display"]["rect"]>) => {
    setForm((current) => current ? {
      ...current,
      counterGroups: current.counterGroups.map((group) => group.id === id ? { ...group, display: { ...group.display, rect: { ...group.display.rect, ...value } } } : group),
      display: id === DEFAULT_COUNTER_GROUP_ID ? { ...current.display, rect: { ...current.display.rect, ...value } } : current.display,
    } : current);
  }, []);

  const recorder = useHotkeyRecorder({
    formatKey: formatTimerHotkey,
    onCommit: (key) => {
      const target = recordingTargetRef.current;
      if (!target) return;
      setRecordingTarget(null);
      updateCounter(target.id, { hotkey: key });
    },
    onCancel: (draft) => {
      const target = recordingTargetRef.current;
      if (!target) return;
      setRecordingTarget(null);
      setForm((current) => {
        if (!current) return current;
        return { ...current, counters: current.counters.map((counter) => counter.id === target.id ? { ...counter, hotkey: draft } : counter) };
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
    setRecordingTarget({ type: "counter", id: counter.id });
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
        return { ...createCounterItem(current.counters.length, groupId), groupId, startValue: "0" };
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
      const outcome = await invoke<CounterSelectionOutcome>("counter_begin_position_selection", { groupId });
      await syncBootstrap({ syncForm: true });
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
      const next = await invoke<CounterBootstrap>("counter_reset", { counterId });
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
      const next = await invoke<CounterBootstrap>("counter_adjust", { counterId, delta });
      setBootstrap(next);
      setStatusMessage(delta > 0 ? `计数器已加 ${delta}。` : `计数器已减 ${Math.abs(delta)}。`);
    } catch (error) {
      const message = getErrorMessage(error);
      setPageError(message);
      setStatusMessage(message);
    }
  }, [isNativeShell]);

  return (
    <AppPage className="auto-rows-max">
      <MacroHeader
        code="02"
        title="COUNTER BOARD"
        verticalLabel="计数"
        subtitle="计数器负责战局累加。每张卡片有独立计数状态与快捷键。"
        badges={
          <>
            <Badge variant={form?.counterEnabled ? "default" : "secondary"}>计数通道{form?.counterEnabled ? "开启" : "关闭"}</Badge>
            <SaveStateBadge dirty={isDirty} saving={saving} />
            {bootstrap?.hotkeyError ? <Badge variant="outline">快捷键异常</Badge> : null}
          </>
        }
        actions={
          <>
            <SignalTile
              label="计数矩阵"
              value={form?.counters.length ?? 0}
              detail={`${bootstrap?.counterRuns.length ?? 0} 个计数状态`}
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

      {!isNativeShell ? (
        <div className="col-span-12">
          <PagePreviewBanner />
        </div>
      ) : null}

      <div className="col-span-12">
        <StatusMatrix items={[
          { id: "counter", state: form?.counterEnabled ? "active" : "idle", label: "计数通道" },
          { id: "counted", state: (bootstrap?.counterRuns.filter((run) => run.value > 0).length ?? 0) > 0 ? "active" : "idle", label: "已计数" },
          { id: "hotkey", state: bootstrap?.hotkeyError ? "error" : form?.counterEnabled ? "valid" : "idle", label: "热键状态" },
          { id: "save", state: isDirty ? "warning" : "valid", label: "保存状态" },
          { id: "ready", state: form?.counterEnabled ? "valid" : "idle", label: "就绪状态" },
        ]} />
      </div>

      <TacticalCard className="col-span-12">
        <SectionHeader
          eyebrow="总控字段"
          icon={<RiSpeedUpLine />}
          title="计数总控"
          description="总开关控制计数器的透明窗口与快捷键是否生效。"
        />
        <CardBody className="grid gap-3">
          <div className="grid gap-px border-2 border-[var(--chalk)] bg-[var(--chalk)]">
            <ControlTile className="border-0 flex items-center gap-3 bg-[var(--carbon)]">
              <Switch checked={Boolean(form?.counterEnabled)} disabled={controlsDisabled || !form} onCheckedChange={(checked) => updateForm("counterEnabled", checked)} />
              <div className="min-w-0">
                <p className="font-mono text-xs font-medium tracking-[0.12em] text-[var(--chalk)] uppercase">计数总开关</p>
                <p className="mt-1 text-xs text-muted-foreground">控制计数器快捷键、透明窗口与现场累加。</p>
              </div>
            </ControlTile>
          </div>
          <InlineControl className="font-mono text-xs font-medium tracking-[0.08em] text-[var(--zinc)] uppercase">
            {statusMessage}
          </InlineControl>
        </CardBody>
      </TacticalCard>

      <div className="col-span-12 h-0.5 bg-[var(--chalk)]" />

      {/* ── 计数器系统 ── */}
      <SectionHeader
        className="col-span-12"
        eyebrow="CHANNEL 02"
        icon={<RiSpeedUpLine />}
        title="计数器系统"
        description="计数器负责战局累加。每张卡片有独立计数状态与快捷键。"
        actions={
          <Button type="button" variant="outline" size="sm" disabled={controlsDisabled || !form} onClick={addCounterGroup}>
            <RiAddLine data-icon="inline-start" />
            新增分组
          </Button>
        }
      />

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
        addButtonDescription="名称、起始数、快捷键均可自定义。"
        onAdd={addCounter}
        disabled={controlsDisabled || !form}
      />
    </AppPage>
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

function CounterCard({ controlsDisabled, counter, groupOptions, index, isDragging, isFavorite, isHighlighted, isRecording, onAdjust, onBeginHotkeyRecording, onDragOver, onDragStart, onHotkeyKeyDown, onHotkeyRecorderBlur, onRemove, onReset, onToggleFavorite, onUpdate, resetDisabled, run }: CounterCardProps) {
  return (
    <TacticalCard active={isDragging} className={cn(counter.enabled ? "" : "opacity-80", isHighlighted ? "outline-4 outline-[var(--amber)]" : "")} data-counter-card={counter.id} data-favorite-card={`counter:${counter.id}`} onPointerEnter={onDragOver}>
      <SectionHeader
        eyebrow="计数器"
        icon={<RiSpeedUpLine />}
        title={(
          <Input
            className="h-auto w-full border-0 bg-transparent p-0 font-heading text-lg font-medium uppercase text-[var(--carbon)] placeholder:text-[var(--slate)] focus-visible:ring-0 focus-visible:ring-offset-0"
            placeholder="输入卡片名称"
            value={counter.name || "计数器"}
            disabled={controlsDisabled}
            onChange={(event) => onUpdate({ name: event.currentTarget.value })}
            aria-label="计数器名称"
          />
        )}
        description={`当前计数 · ${run?.value ?? counter.startValue}`}
        badge={<><Badge variant="outline">{String(index + 1).padStart(2, "0")}</Badge><Badge variant={counter.enabled ? "default" : "outline"}>{counter.enabled ? "启用" : "禁用"}</Badge></>}
      />
      <CardHeader className="border-b-2 border-[var(--chalk)] bg-[var(--slate)] pt-0">
        <div className="grid gap-3 xl:grid-cols-[1fr_auto]">
          <div className="grid gap-3">
            <div>
              <p className="font-mono text-xs font-medium tracking-[0.12em] text-[var(--zinc)] uppercase">所属分组</p>
              <Select disabled={controlsDisabled} value={counter.groupId} onValueChange={(value) => onUpdate({ groupId: value })}>
                <SelectTrigger className="mt-2 w-full max-w-full bg-[var(--carbon)]">
                  <SelectValue placeholder="选择分组" />
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
          <div className="flex flex-wrap items-center justify-end gap-1.5 border-t-2 border-[var(--chalk)] pt-3 xl:border-t-0 xl:border-l-2 xl:pl-3 xl:pt-0">
            <DragButton controlsDisabled={controlsDisabled} onDragStart={onDragStart} />
            <Button
              aria-label={isFavorite ? "取消收藏" : "加入收藏"}
              aria-pressed={isFavorite}
              className={cn(isFavorite ? "text-[var(--amber)]" : "text-muted-foreground")}
              data-icon="inline-start"
              disabled={controlsDisabled}
              onClick={onToggleFavorite}
              size="icon-sm"
              type="button"
              variant="outline"
            >
              {isFavorite ? <RiStarFill /> : <RiStarLine />}
            </Button>
            <Switch checked={counter.enabled} disabled={controlsDisabled} aria-label="启用计数器" onCheckedChange={(checked) => onUpdate({ enabled: checked })} />
            <Button disabled={controlsDisabled} onClick={onRemove} size="icon-sm" type="button" variant="outline" aria-label="删除计数器">
              <RiDeleteBinLine />
            </Button>
          </div>
        </div>
      </CardHeader>
      <CardBody>
        <FieldGroup className="grid gap-4 sm:grid-cols-2">
          <Field>
            <FieldLabel htmlFor={`${counter.id}-start`}>起始数</FieldLabel>
            <FieldContent>
              <Input id={`${counter.id}-start`} disabled={controlsDisabled} inputMode="numeric" value={counter.startValue} onChange={(event) => onUpdate({ startValue: event.currentTarget.value })} />
            </FieldContent>
          </Field>
          <HotkeyField controlsDisabled={controlsDisabled} id={`${counter.id}-hotkey`} isRecording={isRecording} hotkey={counter.hotkey} onBeginHotkeyRecording={onBeginHotkeyRecording} onHotkeyKeyDown={onHotkeyKeyDown} onHotkeyRecorderBlur={onHotkeyRecorderBlur} />
          <div className="flex flex-wrap gap-2 sm:col-span-2">
            <Button
              className="flex-1"
              disabled={resetDisabled}
              onClick={() => onAdjust(-1)}
              type="button"
              variant="outline"
            >
              <RiSubtractLine data-icon="inline-start" />
              -1
            </Button>
            <Button
              className="flex-1"
              disabled={resetDisabled}
              onClick={() => onAdjust(1)}
              type="button"
              variant="outline"
            >
              <RiAddLine data-icon="inline-start" />
              +1
            </Button>
            <Button className="flex-1" disabled={resetDisabled} onClick={onReset} type="button" variant="outline">
              <RiResetLeftLine data-icon="inline-start" />
              重置为起始数
            </Button>
          </div>

        </FieldGroup>
      </CardBody>
    </TacticalCard>
  );
}

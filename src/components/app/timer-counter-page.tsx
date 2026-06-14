import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listenEvent, TIMER_EVENTS } from "@/lib/tauri-events";
import {
  RiAddLine,
  RiDeleteBinLine,
  RiResetLeftLine,
  RiSpeedUpLine,
  RiStarFill,
  RiStarLine,
  RiSubtractLine,
  RiTimerLine,
} from "@remixicon/react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { CardHeader } from "@/components/ui/card";
import { Field, FieldContent, FieldError, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Progress } from "@/components/ui/progress";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { PositionOverlay } from "@/components/ui/position-overlay";
import {
  AppPage,
  CardBody,
  ControlTile,
  DragButton,
  HotkeyField,
  InlineControl,
  MacroHeader,
  PagePreviewBanner,
  SaveStateBadge,
  SectionHeader,
  SignalTile,
  StatusMatrix,
  SurfaceToggleGroup,
  TacticalCard,
} from "@/components/app/app-ui";
import { SyncCardList } from "@/components/app/sync-card-list";
import { SyncGroupSection } from "@/components/app/sync-group-section";
import type {
  CounterItemForm,
  CounterRunState,
  TimerBootstrap,
  TimerDisplayMode,
  TimerGroupForm,
  TimerItemForm,
  TimerRunState,
  TimerSettings,
  TimerSettingsForm,
  TimerDisplayTarget,
  TimerSelectionOutcome,
} from "@/components/app/timer-types";
import { DEFAULT_COUNTER_GROUP_ID, DEFAULT_TIMER_GROUP_ID, TIMER_AUTOSAVE_DELAY_MS } from "@/components/app/timer-types";
import { getErrorMessage } from "@/lib/error-utils";
import { useNativeShell } from "@/hooks/use-native-shell";
import { useAutosave } from "@/hooks/use-autosave";
import { useBootstrapForm } from "@/hooks/use-bootstrap-form";
import { useHotkeyRecorder } from "@/hooks/use-hotkey-recorder";
import { useHighlightScroll } from "@/hooks/use-highlight-scroll";
import { cn } from "@/lib/utils";
import {
  counterRunsById,
  createCounterGroup,
  createCounterItem,
  createTimerGroup,
  createTimerItem,
  formatTimerHotkey,
  isTimerRunActive,
  moveTimerItem,
  parseTimerSettingsForm,
  timerEffectiveCountersByGroup,
  timerEffectiveTimersByGroup,
  timerProgressPercent,
  timerRunsById,
  timerSettingsToForm,
  useTimerOverlayBootstrap,
} from "@/components/app/timer-utils";
import { useFavorites } from "@/hooks/use-favorites";

const TIMER_BOOTSTRAP_SPEC = {
  getBootstrapCommand: "timer_get_bootstrap",
  saveSettingsCommand: "timer_save_settings",
  settingsToForm: timerSettingsToForm,
  parseSettingsForm: parseTimerSettingsForm,
};

export type TimerHighlightTarget = {
  kind: "timer";
  cardId: string;
  /** nonce 用于强制重触发高亮动画（用户重复点击同一卡片） */
  nonce: number;
};

export type CounterHighlightTarget = {
  kind: "counter";
  cardId: string;
  nonce: number;
};

type TimerCounterPageProps = {
  overlayMode?: TimerDisplayMode;
  highlightCardId?: TimerHighlightTarget | CounterHighlightTarget | null;
};

export function TimerCounterPage({ overlayMode, highlightCardId }: TimerCounterPageProps) {
  const isNativeShell = useNativeShell();
  const overlayGroupId = new URLSearchParams(window.location.search).get("groupId");

  if (overlayMode === "display") {
    return <TimerDisplayOverlay groupId={overlayGroupId ?? DEFAULT_TIMER_GROUP_ID} isNativeShell={isNativeShell} />;
  }

  if (overlayMode === "counter-display") {
    return <CounterDisplayOverlay groupId={overlayGroupId ?? DEFAULT_COUNTER_GROUP_ID} isNativeShell={isNativeShell} />;
  }

  if (overlayMode === "position") {
    return <TimerPositionOverlay isNativeShell={isNativeShell} />;
  }

  if (overlayMode === "counter-position") {
    return <CounterPositionOverlay isNativeShell={isNativeShell} />;
  }

  return <TimerCounterWorkbench highlightCardId={highlightCardId ?? null} isNativeShell={isNativeShell} />;
}

function timerSignalChar(timer: TimerItemForm, run?: TimerRunState): string {
  if (!timer.enabled) return "○";
  if (!run) return "▢";
  if (run.status === "running") return "▣";
  if (run.status === "finished") return "●";
  return "▢";
}

function timerSignalState(timer: { enabled: boolean }, run?: TimerRunState): "idle" | "active" | "valid" | "warning" | "error" {
  if (!timer.enabled) return "idle";
  if (!run) return "idle";
  if (run.status === "running") return "active";
  if (run.status === "finished") return "valid";
  return "idle";
}

function TimerCounterWorkbench({ highlightCardId, isNativeShell }: { highlightCardId: TimerHighlightTarget | CounterHighlightTarget | null; isNativeShell: boolean }) {
  const bf = useBootstrapForm<TimerBootstrap, TimerSettings, TimerSettingsForm>({
    spec: TIMER_BOOTSTRAP_SPEC,
    isNativeShell,
    loadStatusMessage: "正在加载同步设置...",
    readyStatusMessage: "同步面板已就绪。计时器与计数器共享配置，双通道独立控制。",
    previewStatusMessage: "浏览器预览模式：当前仅验证布局，原生命令请在桌面端运行。",
    saveSuccessMessage: (next) => {
      const timerMsg = next.settings.timerEnabled ? "计时器开启" : "计时器关闭";
      const counterMsg = next.settings.counterEnabled ? "计数器开启" : "计数器关闭";
      return `同步设置已保存（${timerMsg}，${counterMsg}）。`;
    },
  });

  const { bootstrap, setBootstrap, form, setForm, isDirty, updateForm, saveSettings, syncBootstrap, loading, saving, pageError, setPageError, statusMessage, setStatusMessage, autosaveVersionRef } = bf;

  const [recordingTarget, setRecordingTarget] = useState<{ type: "timer" | "counter"; id: string } | null>(null);
  const draggingTimerIdRef = useRef<string | null>(null);
  const [draggingTimerId, setDraggingTimerId] = useState<string | null>(null);
  const draggingCounterIdRef = useRef<string | null>(null);
  const [draggingCounterId, setDraggingCounterId] = useState<string | null>(null);
  const favorites = useFavorites();
  const recordingTargetRef = useRef<typeof recordingTarget>(null);
  recordingTargetRef.current = recordingTarget;

  const timerHighlight = highlightCardId && highlightCardId.kind === "timer" ? highlightCardId : null;
  const counterHighlight = highlightCardId && highlightCardId.kind === "counter" ? highlightCardId : null;

  useHighlightScroll(timerHighlight, "timer");
  useHighlightScroll(counterHighlight, "counter");

  useEffect(() => {
    const handlePointerUp = () => {
      draggingTimerIdRef.current = null;
      setDraggingTimerId(null);
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
    let unlistenHotkeyTriggered: (() => void) | undefined;
    let unlistenCounterTriggered: (() => void) | undefined;

    void listenEvent(TIMER_EVENTS.stateChanged, (event) => {
      if (disposed) {
        return;
      }
      setBootstrap(event.payload);
    }).then((dispose) => {
      unlistenStateChanged = dispose;
    });

    void listenEvent(TIMER_EVENTS.hotkeyTriggered, (event) => {
      if (disposed) {
        return;
      }
      setStatusMessage(`快捷键已触发 ${event.payload.length} 个计时器。运行中的计时器会忽略重复触发。`);
    }).then((dispose) => {
      unlistenHotkeyTriggered = dispose;
    });

    void listenEvent(TIMER_EVENTS.counterTriggered, (event) => {
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
      unlistenHotkeyTriggered?.();
      unlistenCounterTriggered?.();
    };
  }, [isNativeShell]);

  const runsById = useMemo(() => timerRunsById(bootstrap?.runs ?? []), [bootstrap?.runs]);
  const counterRunsByIdMap = useMemo(() => counterRunsById(bootstrap?.counterRuns ?? []), [bootstrap?.counterRuns]);
  const controlsDisabled = loading || !isNativeShell;

  const updateTimer = useCallback((id: string, value: Partial<TimerItemForm>) => {
    setForm((current) => {
      if (!current) { return current; }
      return {
        ...current,
        timers: current.timers.map((timer) => timer.id === id ? { ...timer, ...value } : timer),
      };
    });
  }, []);

  const updateTimerGroup = useCallback((id: string, value: Partial<TimerGroupForm>) => {
    setForm((current) => current ? {
      ...current,
      timerGroups: current.timerGroups.map((group) => group.id === id ? { ...group, ...value } : group),
    } : current);
  }, []);

  const updateTimerGroupDisplay = useCallback((id: string, value: Partial<TimerGroupForm["display"]>) => {
    setForm((current) => current ? {
      ...current,
      timerGroups: current.timerGroups.map((group) => group.id === id ? { ...group, display: { ...group.display, ...value } } : group),
      display: id === DEFAULT_TIMER_GROUP_ID ? { ...current.display, ...value } : current.display,
    } : current);
  }, []);

  const updateTimerGroupDisplayRect = useCallback((id: string, value: Partial<TimerGroupForm["display"]["rect"]>) => {
    setForm((current) => current ? {
      ...current,
      timerGroups: current.timerGroups.map((group) => group.id === id ? { ...group, display: { ...group.display, rect: { ...group.display.rect, ...value } } } : group),
      display: id === DEFAULT_TIMER_GROUP_ID ? { ...current.display, rect: { ...current.display.rect, ...value } } : current.display,
    } : current);
  }, []);

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
      counterDisplay: id === DEFAULT_COUNTER_GROUP_ID ? { ...current.counterDisplay, ...value } : current.counterDisplay,
    } : current);
  }, []);

  const updateCounterGroupDisplayRect = useCallback((id: string, value: Partial<TimerGroupForm["display"]["rect"]>) => {
    setForm((current) => current ? {
      ...current,
      counterGroups: current.counterGroups.map((group) => group.id === id ? { ...group, display: { ...group.display, rect: { ...group.display.rect, ...value } } } : group),
      counterDisplay: id === DEFAULT_COUNTER_GROUP_ID ? { ...current.counterDisplay, rect: { ...current.counterDisplay.rect, ...value } } : current.counterDisplay,
    } : current);
  }, []);

  const recorder = useHotkeyRecorder({
    formatKey: formatTimerHotkey,
    onCommit: (key) => {
      const target = recordingTargetRef.current;
      if (!target) return;
      setRecordingTarget(null);
      if (target.type === "timer") {
        updateTimer(target.id, { hotkey: key });
      } else {
        updateCounter(target.id, { hotkey: key });
      }
    },
    onCancel: (draft) => {
      const target = recordingTargetRef.current;
      if (!target) return;
      setRecordingTarget(null);
      setForm((current) => {
        if (!current) return current;
        if (target.type === "timer") {
          return { ...current, timers: current.timers.map((timer) => timer.id === target.id ? { ...timer, hotkey: draft } : timer) };
        }
        return { ...current, counters: current.counters.map((counter) => counter.id === target.id ? { ...counter, hotkey: draft } : counter) };
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
    setRecordingTarget({ type: "timer", id: timer.id });
    recorder.beginRecording(timer.hotkey);
    setStatusMessage(`正在录制 ${timer.name || "计时器"} 的快捷键，按下主键会保存；失焦会取消。`);
  }, [recorder]);

  const handleTimerHotkeyRecorderKeyDown = useCallback((timer: TimerItemForm, event: React.KeyboardEvent<HTMLButtonElement>) => {
    if (recordingTarget?.type !== "timer" || recordingTarget.id !== timer.id) {
      return;
    }
    recorder.handleKeyDown(event);
  }, [recordingTarget, recorder]);

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

  const addTimer = useCallback(() => {
    setForm((current) => current ? {
      ...current,
      timers: [...current.timers, (() => {
        const groupId = current.timerGroups[0]?.id ?? DEFAULT_TIMER_GROUP_ID;
        return { ...createTimerItem(current.timers.length, groupId), groupId, durationSeconds: "30", segmentCount: "" };
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
      counters: moveTimerItem(current.counters, activeId, overId),
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

  const beginPositionSelection = useCallback(async (target: TimerDisplayTarget, groupId?: string) => {
    if (!isNativeShell) {
      setStatusMessage("浏览器预览模式下不可设置透明窗口位置，请在桌面端使用。");
      return;
    }

    setStatusMessage("请在透明位置框中拖动窗口，按 Enter 保存，按 Esc 退出修改。透明窗口宽度可在上方调整。");

    try {
      const outcome = await invoke<TimerSelectionOutcome>("timer_begin_position_selection", { target, groupId });
      await syncBootstrap({ syncForm: true });
      const label = target === "timer" ? "计时器" : "计数器";
      if (outcome.kind === "selected") {
        setStatusMessage(`${label}透明窗口位置已保存。`);
      } else if (outcome.kind === "cancelled") {
        setStatusMessage(`${label}透明窗口位置修改已取消。`);
      } else {
        setStatusMessage(`${label}透明窗口位置设置窗口已关闭。`);
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
      const next = await invoke<TimerBootstrap>("timer_counter_reset", { counterId });
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
      const next = await invoke<TimerBootstrap>("timer_counter_adjust", { counterId, delta });
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
        code="S-01"
        title="SYNC BOARD"
        verticalLabel="同步"
        subtitle="计时器负责阶段节奏，计数器负责战局累加；透明窗口、定位窗口与快捷键保持双通道隔离。"
        badges={
          <>
            <Badge variant={form?.timerEnabled ? "default" : "secondary"}>计时通道{form?.timerEnabled ? "开启" : "关闭"}</Badge>
            <Badge variant={form?.counterEnabled ? "default" : "secondary"}>计数通道{form?.counterEnabled ? "开启" : "关闭"}</Badge>
            <SaveStateBadge dirty={isDirty} saving={saving} />
            {bootstrap?.hotkeyError ? <Badge variant="outline">快捷键异常</Badge> : null}
          </>
        }
        actions={
          <>
            <SignalTile
              label="计时矩阵"
              value={form?.timers.length ?? 0}
              detail={`${bootstrap?.runs.filter((run) => run.status === "running").length ?? 0} 个运行中`}
            />
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
          { id: "timer", state: form?.timerEnabled ? "active" : "idle", label: "计时通道" },
          { id: "counter", state: form?.counterEnabled ? "active" : "idle", label: "计数通道" },
          { id: "running", state: (bootstrap?.runs.filter((run) => run.status === "running").length ?? 0) > 0 ? "active" : "idle", label: "计时运行" },
          { id: "counted", state: (bootstrap?.counterRuns.filter((run) => run.value > 0).length ?? 0) > 0 ? "active" : "idle", label: "已计数" },
          { id: "hotkey", state: bootstrap?.hotkeyError ? "error" : (form?.timerEnabled || form?.counterEnabled) ? "valid" : "idle", label: "热键状态" },
          { id: "save", state: isDirty ? "warning" : "valid", label: "保存状态" },
          { id: "ready", state: (form?.timerEnabled || form?.counterEnabled) ? "valid" : "idle", label: "就绪状态" },
        ]} />
      </div>

      <TacticalCard className="col-span-12">
        <SectionHeader
          eyebrow="总控字段"
          icon={<RiSpeedUpLine />}
          title="同步总控"
          description="总开关分别控制计时器与计数器的透明窗口与快捷键是否生效。"
        />
        <CardBody className="grid gap-3">
          <div className="grid gap-px border-2 border-[var(--chalk)] bg-[var(--chalk)] xl:grid-cols-2">
            <ControlTile className="border-0 flex items-center gap-3 bg-[var(--slate)]">
              <Switch checked={Boolean(form?.timerEnabled)} disabled={controlsDisabled || !form} onCheckedChange={(checked) => updateForm("timerEnabled", checked)} />
              <div className="min-w-0">
                <p className="font-mono text-xs font-medium tracking-[0.12em] text-[var(--chalk)] uppercase">计时总开关</p>
                <p className="mt-1 text-xs text-muted-foreground">控制计时器快捷键与透明窗口输出。</p>
              </div>
            </ControlTile>
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

      {/* ── 计时器系统 ── */}
      <SectionHeader
        className="col-span-12"
        eyebrow="CHANNEL 01"
        icon={<RiTimerLine />}
        title="计时器系统"
        description="计时器负责阶段节奏。每张卡片配置独立计时方向、触发模式与快捷键。"
        actions={
          <Button type="button" variant="outline" size="sm" disabled={controlsDisabled || !form} onClick={addTimerGroup}>
            <RiAddLine data-icon="inline-start" />
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
        onPositionSelection={(groupId) => void beginPositionSelection("timer", groupId)}
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
        effectiveCount={(groupId) => timerEffectiveCountersByGroup(form, groupId).length}
        onGroupUpdate={updateCounterGroup}
        onGroupDelete={removeCounterGroup}
        onPositionSelection={(groupId) => void beginPositionSelection("counter", groupId)}
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

function TimerCard({ controlsDisabled, groupOptions, index, isDragging, isFavorite, isHighlighted, isRecording, onBeginHotkeyRecording, onDragOver, onDragStart, onHotkeyKeyDown, onHotkeyRecorderBlur, onRemove, onToggleFavorite, onUpdate, run, timer }: TimerCardProps) {
  const isMultiSegment = timer.segmentCount !== "" && Number.parseInt(timer.segmentCount, 10) >= 2;

  return (
    <TacticalCard active={isDragging} className={cn(timer.enabled ? "" : "opacity-80", isHighlighted ? "outline-4 outline-[var(--amber)]" : "", run?.status === "running" ? "border-l-4 border-l-[var(--amber)]" : run?.status === "finished" ? "border-l-4 border-l-[var(--valid-green)]" : "")} data-timer-card={timer.id} data-favorite-card={`timer:${timer.id}`} onPointerEnter={onDragOver}>
      <SectionHeader
        eyebrow="计时器"
        icon={<RiTimerLine />}
        title={(
          <Input
            className="h-auto w-full border-0 bg-transparent p-0 font-heading text-lg font-medium uppercase text-[var(--carbon)] placeholder:text-[var(--slate)] focus-visible:ring-0 focus-visible:ring-offset-0"
            placeholder="输入卡片名称"
            value={timer.name || "计时器"}
            disabled={controlsDisabled}
            onChange={(event) => onUpdate({ name: event.currentTarget.value })}
            aria-label="计时器名称"
          />
        )}
        description={run ? `${timerSignalChar(timer, run)} ${Math.floor(run.currentSeconds)}s` : (timer.enabled ? "▢ 等待触发" : "○ 已禁用")}
        actions={(
          <div className="flex items-center gap-1.5">
            <Select disabled={controlsDisabled} value={timer.groupId} onValueChange={(value) => onUpdate({ groupId: value })}>
              <SelectTrigger className="w-32 bg-[var(--carbon)]">
                <SelectValue placeholder="分组" />
              </SelectTrigger>
              <SelectContent>
                {groupOptions.map((group) => (
                  <SelectItem key={group.id} value={group.id}>{group.name}</SelectItem>
                ))}
              </SelectContent>
            </Select>
            <DragButton controlsDisabled={controlsDisabled} onDragStart={onDragStart} />
            <Button aria-label={isFavorite ? "取消收藏" : "加入收藏"} aria-pressed={isFavorite} className={cn(isFavorite ? "text-[var(--amber)]" : "text-muted-foreground")} disabled={controlsDisabled} onClick={onToggleFavorite} size="icon-sm" type="button" variant="outline">
              {isFavorite ? <RiStarFill /> : <RiStarLine />}
            </Button>
            <Switch checked={timer.enabled} disabled={controlsDisabled} aria-label="启用计时器" onCheckedChange={(checked) => onUpdate({ enabled: checked })} />
            <Button disabled={controlsDisabled} onClick={onRemove} size="icon-sm" type="button" variant="outline" aria-label="删除计时器">
              <RiDeleteBinLine />
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
              <Input id={`${timer.id}-duration`} disabled={controlsDisabled} inputMode="numeric" min="1" value={timer.durationSeconds} onChange={(event) => onUpdate({ durationSeconds: event.currentTarget.value })} />
            </FieldContent>
          </Field>
          <Field>
            <FieldLabel>计时方向</FieldLabel>
            <FieldContent>
              <SurfaceToggleGroup>
                <ToggleGroup className="w-full" disabled={controlsDisabled} type="single" value={timer.direction} variant="outline" onValueChange={(value) => value ? onUpdate({ direction: value as TimerItemForm["direction"] }) : undefined}>
                  <ToggleGroupItem className="min-w-24 flex-1 border-[var(--chalk)] font-mono text-sm font-black data-[state=on]:bg-[var(--chalk)] data-[state=on]:text-[var(--carbon)]" value="countup">正</ToggleGroupItem>
                  <ToggleGroupItem className="min-w-24 flex-1 border-[var(--chalk)] font-mono text-sm font-black data-[state=on]:bg-[var(--chalk)] data-[state=on]:text-[var(--carbon)]" value="countdown">反</ToggleGroupItem>
                </ToggleGroup>
              </SurfaceToggleGroup>
            </FieldContent>
          </Field>
          <Field>
            <FieldLabel>触发模式</FieldLabel>
            <FieldContent>
              <SurfaceToggleGroup>
                <ToggleGroup className="w-full" disabled={controlsDisabled} type="single" value={timer.triggerMode} variant="outline" onValueChange={(value) => value ? onUpdate({ triggerMode: value as TimerItemForm["triggerMode"] }) : undefined}>
                  <ToggleGroupItem className="min-w-24 flex-1 border-[var(--chalk)] font-mono text-sm font-black data-[state=on]:bg-[var(--chalk)] data-[state=on]:text-[var(--carbon)]" value="press">按下</ToggleGroupItem>
                  <ToggleGroupItem className="min-w-24 flex-1 border-[var(--chalk)] font-mono text-sm font-black data-[state=on]:bg-[var(--chalk)] data-[state=on]:text-[var(--carbon)]" value="release">释放</ToggleGroupItem>
                </ToggleGroup>
              </SurfaceToggleGroup>
            </FieldContent>
          </Field>
          <Field>
            <FieldLabel htmlFor={`${timer.id}-segment-count`}>多段数（留空=单段）</FieldLabel>
            <FieldContent>
              <Input id={`${timer.id}-segment-count`} disabled={controlsDisabled} inputMode="numeric" min="2" max="99" placeholder="留空为普通单段计时器" value={timer.segmentCount} onChange={(event) => onUpdate({ segmentCount: event.currentTarget.value })} />
            </FieldContent>
            {isMultiSegment ? (
              <p className="text-xs text-muted-foreground">总时长 {Number.parseInt(timer.durationSeconds, 10) * Number.parseInt(timer.segmentCount, 10)} 秒，每次触发减少 {timer.durationSeconds} 秒</p>
            ) : null}
          </Field>
          <ControlTile className="flex items-center gap-3 sm:col-span-2">
            <Switch
              checked={timer.ignoreRunning}
              disabled={controlsDisabled}
              onCheckedChange={(checked) => onUpdate({ ignoreRunning: checked })}
            />
            <div className="min-w-0">
              <p className="text-sm font-medium text-foreground">运行中忽略触发</p>
              <p className="mt-1 text-xs text-muted-foreground">开启后运行时快捷键无效；关闭后运行时触发会重置计时器。</p>
            </div>
          </ControlTile>
          <div className="sm:col-span-2">
            <HotkeyField controlsDisabled={controlsDisabled} id={`${timer.id}-hotkey`} isRecording={isRecording} hotkey={timer.hotkey} onBeginHotkeyRecording={onBeginHotkeyRecording} onHotkeyKeyDown={onHotkeyKeyDown} onHotkeyRecorderBlur={onHotkeyRecorderBlur} />
          </div>

        </FieldGroup>
      </CardBody>
    </TacticalCard>
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

function TimerDisplayOverlay({ groupId, isNativeShell }: { groupId: string; isNativeShell: boolean }) {
  const [bootstrap, setBootstrap] = useState<TimerBootstrap | null>(null);
  const [now, setNow] = useState(Date.now);

  useTimerOverlayBootstrap(isNativeShell, setBootstrap);

  useEffect(() => {
    let rafId: number;
    const tick = () => {
      setNow(Date.now());
      rafId = requestAnimationFrame(tick);
    };
    rafId = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(rafId);
  }, []);

  const runsById = useMemo(() => timerRunsById(bootstrap?.runs ?? []), [bootstrap?.runs]);
  const group = bootstrap?.settings.timerGroups?.find((item) => item.id === groupId);
  const opacity = group?.display.fontOpacity ?? bootstrap?.settings.display.fontOpacity ?? 0.92;

  function smoothProgress(run: TimerRunState | undefined): number {
    if (!run || !run.startedAtMs || run.status === "finished") {
      return timerProgressPercent(run, 0);
    }
    if (run.segmentCount != null && run.segmentCount >= 2) {
      const durationMs = run.durationSeconds * 1000;
      const poolMs = run.recoveryStartPool * 1000 + (now - run.startedAtMs);
      const cappedPoolMs = Math.max(0, Math.min(durationMs, poolMs));
      if (run.direction === "countdown") {
        return Math.max(0, Math.min(100, ((durationMs - cappedPoolMs) / durationMs) * 100));
      }
      return Math.max(0, Math.min(100, (cappedPoolMs / durationMs) * 100));
    }
    const durationMs = run.durationSeconds * 1000;
    if (run.direction === "countup") {
      return Math.max(0, Math.min(100, ((now - run.startedAtMs) / durationMs) * 100));
    }
    return Math.max(0, Math.min(100, ((run.startedAtMs + durationMs - now) / durationMs) * 100));
  }

  function smoothDisplayValue(run: TimerRunState | undefined): string {
    if (!run) {
      return "";
    }
    if (!run.startedAtMs || run.status === "finished") {
      if (run.segmentCount != null && run.segmentCount >= 2 && run.direction === "countdown") {
        return Math.floor(run.durationSeconds - run.currentSeconds).toString();
      }
      return Math.floor(run.currentSeconds).toString();
    }
    if (run.segmentCount != null && run.segmentCount >= 2) {
      return smoothSegmentDisplayValue(run);
    }
    const durationMs = run.durationSeconds * 1000;
    if (run.direction === "countdown") {
      const remainingMs = Math.max(0, run.startedAtMs + durationMs - now);
      return Math.ceil(remainingMs / 1000).toString();
    }
    const elapsedMs = Math.min(durationMs, now - run.startedAtMs);
    return Math.floor(elapsedMs / 1000).toString();
  }

  function smoothSegmentDisplayValue(run: TimerRunState): string {
    const durationMs = run.durationSeconds * 1000;
    const poolMs = Math.min(durationMs, run.recoveryStartPool * 1000 + (now - run.startedAtMs));
    if (run.direction === "countdown") {
      return Math.ceil((durationMs - poolMs) / 1000).toString();
    }
    return Math.floor(poolMs / 1000).toString();
  }

  return (
    <div className="flex h-screen w-screen items-start justify-start overflow-hidden bg-transparent p-2 font-mono text-white" style={{ opacity }}>
      <div className="h-full w-full overflow-hidden rounded-md border border-white/20 bg-black/20 px-3 py-2 backdrop-blur-[1px]">
        {bootstrap?.settings.timers.filter((t) => t.enabled && t.groupId === groupId && (group?.enabled ?? true)).map((timer) => {
          const run = runsById.get(timer.id);
          const finished = run?.status === "finished";
          const isActive = isTimerRunActive(run);
          const isMultiSegment = timer.segmentCount != null && timer.segmentCount >= 2;
          const progress = smoothProgress(run);

          let displayValue: string;
          if (!run) {
            if (isMultiSegment) {
              const total = timer.segmentCount! * timer.durationSeconds;
              displayValue = String(total);
            } else {
              displayValue = String(timer.durationSeconds);
            }
          } else {
            displayValue = smoothDisplayValue(run);
          }

          return (
            <div key={timer.id} className={cn("relative my-0.5 min-w-0 overflow-hidden rounded-md px-2 py-0.5 text-base font-semibold tracking-wide", isActive ? "bg-primary/20 ring-1 ring-primary/70" : "")}>
              {(run && !isMultiSegment) || isMultiSegment ? (
                <Progress aria-label={`${timer.name} 进度`} className="absolute inset-0 h-full rounded-md bg-white/20 [&_[data-slot=progress-indicator]]:bg-[var(--rust)]" value={progress} />
              ) : null}
              <div className="relative flex min-w-0 items-center justify-between gap-3">
                <span className="flex min-w-0 items-center gap-1.5">
                  {isActive ? <span aria-hidden="true" className="h-1.5 w-1.5 shrink-0 animate-pulse rounded-full bg-primary" /> : null}
              <span className={cn("min-w-0 truncate", finished && !isMultiSegment ? "text-primary-foreground italic" : "text-white")}>{timer.name}</span>
                  <span className={cn("shrink-0 font-mono text-xs", isActive ? "text-primary" : finished ? "text-[var(--amber)]" : "text-white/60")}>{timerSignalState(timer, run)}</span>
                </span>
                <span className={finished && !isMultiSegment ? "shrink-0 text-primary-foreground italic" : "shrink-0 text-white"}>{displayValue}</span>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

function CounterDisplayOverlay({ groupId, isNativeShell }: { groupId: string; isNativeShell: boolean }) {
  const [bootstrap, setBootstrap] = useState<TimerBootstrap | null>(null);

  useTimerOverlayBootstrap(isNativeShell, setBootstrap);

  const counterRunsByIdMap = useMemo(() => counterRunsById(bootstrap?.counterRuns ?? []), [bootstrap?.counterRuns]);
  const group = bootstrap?.settings.counterGroups?.find((item) => item.id === groupId);
  const opacity = group?.display.fontOpacity ?? bootstrap?.settings.counterDisplay.fontOpacity ?? 0.92;

  return (
    <div className="flex h-screen w-screen items-start justify-start overflow-hidden bg-transparent p-2 font-mono text-white" style={{ opacity }}>
      <div className="h-full w-full overflow-hidden rounded-md border border-white/20 bg-black/20 px-3 py-2 backdrop-blur-[1px]">
        {bootstrap?.settings.counters.filter((c) => c.enabled && c.groupId === groupId && (group?.enabled ?? true)).map((counter) => {
          const run = counterRunsByIdMap.get(counter.id);
          return (
            <div key={counter.id} className="flex min-w-0 items-center justify-between gap-3 py-0.5 text-base font-semibold tracking-wide">
              <span className="min-w-0 truncate text-white">{counter.name}</span>
              <span className="shrink-0 text-white">{run?.value ?? counter.startValue}</span>
            </div>
          );
        })}
      </div>
    </div>
  );
}

function TimerPositionOverlay({ isNativeShell }: { isNativeShell: boolean }) {
  return (
    <PositionOverlay
      isNativeShell={isNativeShell}
      label="计时器"
      commands={{
        commit: "timer_position_commit",
        cancel: "timer_position_cancel",
        moved: "timer_position_moved",
      }}
      initialStatusSuffix="关闭计时器总开关后透明窗口会隐藏并解绑快捷键。"
    />
  );
}

function CounterPositionOverlay({ isNativeShell }: { isNativeShell: boolean }) {
  return (
    <PositionOverlay
      isNativeShell={isNativeShell}
      label="计数器"
      commands={{
        commit: "timer_position_commit",
        cancel: "timer_position_cancel",
        moved: "timer_position_moved",
      }}
      initialStatusSuffix="关闭计数器总开关后透明窗口会隐藏并解绑快捷键。"
    />
  );
}

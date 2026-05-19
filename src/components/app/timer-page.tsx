import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  RiAddLine,
  RiDeleteBinLine,
  RiEyeLine,
  RiKeyboardLine,
  RiMapPinLine,
  RiResetLeftLine,
  RiSpeedUpLine,
  RiTimerLine,
} from "@remixicon/react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Field, FieldContent, FieldError, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Progress } from "@/components/ui/progress";
import { Slider } from "@/components/ui/slider";
import { Switch } from "@/components/ui/switch";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import type { CounterItemForm, CounterRunState, TimerBootstrap, TimerDisplayMode, TimerDisplayTarget, TimerItemForm, TimerRunState, TimerSelectionOutcome, TimerSettings, TimerSettingsForm } from "@/components/app/timer-types";
import { TIMER_AUTOSAVE_DELAY_MS } from "@/components/app/timer-types";
import { getErrorMessage } from "@/lib/error-utils";
import { useNativeShell } from "@/hooks/use-native-shell";
import { useTimeoutCleanup } from "@/hooks/use-timeout-cleanup";
import { cn } from "@/lib/utils";
import {
  counterRunsById,
  createCounterItem,
  createTimerItem,
  formatTimerHotkey,
  formatTimerRemaining,
  isTimerDirty,
  moveTimerItem,
  parseTimerSettingsForm,
  timerProgressPercent,
  timerRunsById,
  timerSettingsToForm,
} from "@/components/app/timer-utils";

export function TimerPage({ overlayMode }: { overlayMode?: TimerDisplayMode }) {
  const isNativeShell = useNativeShell();

  if (overlayMode === "display") {
    return <TimerDisplayOverlay isNativeShell={isNativeShell} />;
  }

  if (overlayMode === "counter-display") {
    return <CounterDisplayOverlay isNativeShell={isNativeShell} />;
  }

  if (overlayMode === "position") {
    return <TimerPositionOverlay isNativeShell={isNativeShell} target="timer" />;
  }

  if (overlayMode === "counter-position") {
    return <TimerPositionOverlay isNativeShell={isNativeShell} target="counter" />;
  }

  return <TimerWorkbench isNativeShell={isNativeShell} />;
}

function TimerWorkbench({ isNativeShell }: { isNativeShell: boolean }) {
  const [bootstrap, setBootstrap] = useState<TimerBootstrap | null>(null);
  const [form, setForm] = useState<TimerSettingsForm | null>(null);
  const [loading, setLoading] = useState(isNativeShell);
  const [saving, setSaving] = useState(false);
  const [recordingTarget, setRecordingTarget] = useState<{ type: "timer" | "counter"; id: string } | null>(null);
  const draggingTimerIdRef = useRef<string | null>(null);
  const draggingCounterIdRef = useRef<string | null>(null);
  const [draggingTimerId, setDraggingTimerId] = useState<string | null>(null);
  const [draggingCounterId, setDraggingCounterId] = useState<string | null>(null);
  const hotkeyDraftRef = useRef("");
  const [statusMessage, setStatusMessage] = useState(isNativeShell ? "正在加载计时/计数器..." : "浏览器预览模式：当前仅验证布局，原生命令请在桌面端运行。");
  const [pageError, setPageError] = useState<string | null>(null);
  const saveTimeoutRef = useTimeoutCleanup();
  const autosaveVersionRef = useRef(0);

  useEffect(() => {
    const handlePointerUp = () => {
      draggingTimerIdRef.current = null;
      draggingCounterIdRef.current = null;
      setDraggingTimerId(null);
      setDraggingCounterId(null);
    };

    window.addEventListener("pointerup", handlePointerUp);
    return () => window.removeEventListener("pointerup", handlePointerUp);
  }, []);

  const syncBootstrap = useCallback(async (syncForm = false) => {
    const next = await invoke<TimerBootstrap>("timer_get_bootstrap");
    setBootstrap(next);
    setForm((current) => (syncForm || current === null ? timerSettingsToForm(next.settings) : current));
    setPageError(null);
    return next;
  }, []);

  useEffect(() => {
    if (!isNativeShell) {
      return;
    }

    let disposed = false;

    const load = async () => {
      try {
        setLoading(true);
        const next = await syncBootstrap(true);
        if (!disposed) {
          setForm(timerSettingsToForm(next.settings));
          setStatusMessage("计时/计数器已就绪。两个总开关分别控制对应透明窗口与快捷键，配置会持续保留。");
        }
      } catch (error) {
        if (!disposed) {
          const message = getErrorMessage(error);
          setPageError(message);
          setStatusMessage(message);
        }
      } finally {
        if (!disposed) {
          setLoading(false);
        }
      }
    };

    void load();

    return () => {
      disposed = true;
    };
  }, [isNativeShell, syncBootstrap]);

  useEffect(() => {
    if (!isNativeShell) {
      return;
    }

    let disposed = false;
    let unlistenStateChanged: (() => void) | undefined;
    let unlistenHotkeyTriggered: (() => void) | undefined;
    let unlistenCounterTriggered: (() => void) | undefined;

    void listen<TimerBootstrap>("timer://state-changed", (event) => {
      if (disposed) {
        return;
      }
      setBootstrap(event.payload);
    }).then((dispose) => {
      unlistenStateChanged = dispose;
    });

    void listen<string[]>("timer://hotkey-triggered", (event) => {
      if (disposed) {
        return;
      }
      setStatusMessage(`快捷键已触发 ${event.payload.length} 个计时器。运行中的计时器会忽略重复触发。`);
    }).then((dispose) => {
      unlistenHotkeyTriggered = dispose;
    });

    void listen<string[]>("timer://counter-triggered", (event) => {
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

  const dirty = useMemo(() => isTimerDirty(bootstrap, form), [bootstrap, form]);
  const runsById = useMemo(() => timerRunsById(bootstrap?.runs ?? []), [bootstrap?.runs]);
  const counterRunsByIdMap = useMemo(() => counterRunsById(bootstrap?.counterRuns ?? []), [bootstrap?.counterRuns]);
  const controlsDisabled = loading || saving || !isNativeShell;

  const updateForm = useCallback(<K extends keyof TimerSettingsForm>(key: K, value: TimerSettingsForm[K]) => {
    setForm((current) => (current ? { ...current, [key]: value } : current));
  }, []);

  const updateDisplay = useCallback((target: TimerDisplayTarget, value: Partial<TimerSettingsForm["display"]>) => {
    setForm((current) => current ? {
      ...current,
      [target === "timer" ? "display" : "counterDisplay"]: {
        ...(target === "timer" ? current.display : current.counterDisplay),
        ...value,
      },
    } : current);
  }, []);

  const updateDisplayRect = useCallback((target: TimerDisplayTarget, value: Partial<TimerSettingsForm["display"]["rect"]>) => {
    setForm((current) => {
      if (!current) {
        return current;
      }
      const key = target === "timer" ? "display" : "counterDisplay";
      return {
        ...current,
        [key]: {
          ...current[key],
          rect: { ...current[key].rect, ...value },
        },
      };
    });
  }, []);

  const updateTimer = useCallback((id: string, value: Partial<TimerItemForm>) => {
    setForm((current) => current ? {
      ...current,
      timers: current.timers.map((timer) => timer.id === id ? { ...timer, ...value } : timer),
    } : current);
  }, []);

  const updateCounter = useCallback((id: string, value: Partial<CounterItemForm>) => {
    setForm((current) => current ? {
      ...current,
      counters: current.counters.map((counter) => counter.id === id ? { ...counter, ...value } : counter),
    } : current);
  }, []);

  const saveSettings = useCallback(async (settingsValue: TimerSettings, pendingVersion?: number) => {
    try {
      setSaving(true);
      const next = await invoke<TimerBootstrap>("timer_save_settings", { settingsValue });
      if (typeof pendingVersion === "number" && pendingVersion !== autosaveVersionRef.current) {
        return;
      }
      setBootstrap(next);
      setForm(timerSettingsToForm(next.settings));
      setPageError(null);
      setStatusMessage(next.settings.timerEnabled || next.settings.counterEnabled ? "计时/计数器设置已保存，已开启模块的快捷键已生效。" : "计时器和计数器均已关闭：透明窗口隐藏，快捷键已解绑，配置已保留。");
    } catch (error) {
      const message = getErrorMessage(error);
      setPageError(message);
      setStatusMessage(message);
    } finally {
      setSaving(false);
    }
  }, []);

  useEffect(() => {
    if (!isNativeShell || loading || !bootstrap || !form || recordingTarget) {
      return;
    }

    if (!dirty) {
      return;
    }

    const nextVersion = autosaveVersionRef.current + 1;
    autosaveVersionRef.current = nextVersion;
    const formSnapshot = form;

    saveTimeoutRef.current = window.setTimeout(() => {
      try {
        void saveSettings(parseTimerSettingsForm(formSnapshot), nextVersion);
      } catch (error) {
        if (nextVersion !== autosaveVersionRef.current) {
          return;
        }
        const message = getErrorMessage(error);
        setPageError(message);
        setStatusMessage(`保存失败：${message}`);
      }
    }, TIMER_AUTOSAVE_DELAY_MS);

    return () => {
      if (saveTimeoutRef.current !== null) {
        window.clearTimeout(saveTimeoutRef.current);
        saveTimeoutRef.current = null;
      }
    };
  }, [bootstrap, dirty, form, isNativeShell, loading, recordingTarget, saveSettings]);

  const beginTimerHotkeyRecording = useCallback((timer: TimerItemForm) => {
    hotkeyDraftRef.current = timer.hotkey;
    setRecordingTarget({ type: "timer", id: timer.id });
    setStatusMessage(`正在录制 ${timer.name || "计时器"} 的快捷键，按 Esc 取消。`);
  }, []);

  const beginCounterHotkeyRecording = useCallback((counter: CounterItemForm) => {
    hotkeyDraftRef.current = counter.hotkey;
    setRecordingTarget({ type: "counter", id: counter.id });
    setStatusMessage(`正在录制 ${counter.name || "计数器"} 的快捷键，按 Esc 取消。`);
  }, []);

  const handleTimerHotkeyRecorderKeyDown = useCallback((timer: TimerItemForm, event: React.KeyboardEvent<HTMLButtonElement>) => {
    if (recordingTarget?.type !== "timer" || recordingTarget.id !== timer.id) {
      return;
    }

    handleHotkeyRecorderKeyDown(event, () => updateTimer(timer.id, { hotkey: hotkeyDraftRef.current }), (hotkey) => updateTimer(timer.id, { hotkey }));
  }, [recordingTarget, updateTimer]);

  const handleCounterHotkeyRecorderKeyDown = useCallback((counter: CounterItemForm, event: React.KeyboardEvent<HTMLButtonElement>) => {
    if (recordingTarget?.type !== "counter" || recordingTarget.id !== counter.id) {
      return;
    }

    handleHotkeyRecorderKeyDown(event, () => updateCounter(counter.id, { hotkey: hotkeyDraftRef.current }), (hotkey) => updateCounter(counter.id, { hotkey }));
  }, [recordingTarget, updateCounter]);

  const handleHotkeyRecorderKeyDown = useCallback((event: React.KeyboardEvent<HTMLButtonElement>, cancel: () => void, commit: (hotkey: string) => void) => {
    if (event.key === "Tab") {
      return;
    }

    event.preventDefault();
    event.stopPropagation();

    if (event.key === "Escape") {
      cancel();
      setRecordingTarget(null);
      setStatusMessage("已取消快捷键录制。");
      return;
    }

    const nextHotkey = formatTimerHotkey(event);
    if (!nextHotkey) {
      setStatusMessage("请按下一个可识别的主键，支持字母、数字、功能键与常用导航键。");
      return;
    }

    commit(nextHotkey);
    setRecordingTarget(null);
    setStatusMessage(`新的快捷键已录制：${nextHotkey}`);
  }, []);

  const addTimer = useCallback(() => {
    setForm((current) => current ? {
      ...current,
      timers: [...current.timers, { ...createTimerItem(current.timers.length), durationSeconds: "30" }],
    } : current);
  }, []);

  const addCounter = useCallback(() => {
    setForm((current) => current ? {
      ...current,
      counters: [...current.counters, { ...createCounterItem(current.counters.length), startValue: "0" }],
    } : current);
  }, []);

  const removeTimer = useCallback((id: string) => {
    setForm((current) => current && current.timers.length > 1 ? {
      ...current,
      timers: current.timers.filter((timer) => timer.id !== id),
    } : current);
  }, []);

  const removeCounter = useCallback((id: string) => {
    setForm((current) => current && current.counters.length > 1 ? {
      ...current,
      counters: current.counters.filter((counter) => counter.id !== id),
    } : current);
  }, []);

  const moveTimer = useCallback((activeId: string, overId: string) => {
    setForm((current) => current ? {
      ...current,
      timers: moveTimerItem(current.timers, activeId, overId),
    } : current);
  }, []);

  const moveCounter = useCallback((activeId: string, overId: string) => {
    setForm((current) => current ? {
      ...current,
      counters: moveTimerItem(current.counters, activeId, overId),
    } : current);
  }, []);

  const beginTimerDrag = useCallback((id: string) => {
    draggingTimerIdRef.current = id;
    setDraggingTimerId(id);
  }, []);

  const beginCounterDrag = useCallback((id: string) => {
    draggingCounterIdRef.current = id;
    setDraggingCounterId(id);
  }, []);

  const moveDraggingTimerOver = useCallback((overId: string) => {
    const activeId = draggingTimerIdRef.current;
    if (!activeId || activeId === overId) {
      return;
    }
    moveTimer(activeId, overId);
  }, [moveTimer]);

  const moveDraggingCounterOver = useCallback((overId: string) => {
    const activeId = draggingCounterIdRef.current;
    if (!activeId || activeId === overId) {
      return;
    }
    moveCounter(activeId, overId);
  }, [moveCounter]);

  const beginPositionSelection = useCallback(async (target: TimerDisplayTarget) => {
    if (!isNativeShell) {
      setStatusMessage("浏览器预览模式下不可设置透明窗口位置，请在桌面端使用。");
      return;
    }

    setStatusMessage("请在透明位置框中拖动窗口，按 Enter 保存，按 Esc 退出修改。透明窗口宽度可在上方调整。");

    try {
      const outcome = await invoke<TimerSelectionOutcome>("timer_begin_position_selection", { target });
      await syncBootstrap(true);
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

  return (
    <Tabs defaultValue="timers" className="flex h-full min-h-0 flex-col gap-0 bg-background">
      <section className="border-b border-border/70 bg-card/95 px-4 py-4 shadow-sm backdrop-blur-sm xl:px-5">
        <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
          <div className="min-w-0 space-y-2">
            <div className="flex flex-wrap items-center gap-2">
              <Badge variant={form?.timerEnabled ? "default" : "secondary"}>计时{form?.timerEnabled ? "已开启" : "已关闭"}</Badge>
              <Badge variant={form?.counterEnabled ? "default" : "secondary"}>计数{form?.counterEnabled ? "已开启" : "已关闭"}</Badge>
              {saving ? <Badge variant="outline">保存中</Badge> : dirty ? <Badge variant="outline">待保存</Badge> : <Badge variant="outline">已保存</Badge>}
              {bootstrap?.hotkeyError ? <Badge variant="outline">快捷键异常</Badge> : null}
            </div>
            <div>
              <h1 className="text-lg font-semibold tracking-tight text-foreground">计时\计数器</h1>
              <p className="mt-1 text-sm text-muted-foreground">计时器支持正/反计时与进度背景；计数器通过快捷键累加，可单独显示并一键重置。</p>
            </div>
          </div>

          <div className="flex flex-col gap-3 rounded-lg border border-border/70 bg-background px-4 py-3">
            <TabsList className="h-11 w-full bg-border text-foreground shadow-inner">
              <TabsTrigger className="px-5 py-2 text-sm font-semibold text-muted-foreground data-active:bg-primary data-active:text-primary-foreground data-active:shadow-sm" value="timers">计时器</TabsTrigger>
              <TabsTrigger className="px-5 py-2 text-sm font-semibold text-muted-foreground data-active:bg-primary data-active:text-primary-foreground data-active:shadow-sm" value="counters">计数器</TabsTrigger>
            </TabsList>
            <div className="grid gap-3 sm:grid-cols-2">
              <div className="flex items-center gap-3 rounded-md border border-border bg-card px-3 py-2">
                <Switch checked={Boolean(form?.timerEnabled)} disabled={controlsDisabled || !form} onCheckedChange={(checked) => updateForm("timerEnabled", checked)} />
                <div>
                  <p className="text-sm font-medium text-foreground">计时器总开关</p>
                  <p className="mt-1 text-xs text-muted-foreground">控制计时器快捷键和透明窗口。</p>
                </div>
              </div>
              <div className="flex items-center gap-3 rounded-md border border-border bg-card px-3 py-2">
                <Switch checked={Boolean(form?.counterEnabled)} disabled={controlsDisabled || !form} onCheckedChange={(checked) => updateForm("counterEnabled", checked)} />
                <div>
                  <p className="text-sm font-medium text-foreground">计数器总开关</p>
                  <p className="mt-1 text-xs text-muted-foreground">控制计数器快捷键和透明窗口。</p>
                </div>
              </div>
            </div>
          </div>
        </div>
      </section>

      <div className="flex-1 overflow-y-auto px-4 py-4 xl:px-5">
        <div className="mx-auto flex w-full max-w-6xl flex-col gap-4">
          {pageError ? <FieldError>{pageError}</FieldError> : null}

          <TabsContent value="timers" className="flex flex-col gap-4">
            <DisplaySettingsCard
              controlsDisabled={controlsDisabled || !form?.timerEnabled}
              description="宽度可调，每个计时器一行；倒计时进度条会作为文本背景随剩余时间减少。"
              display={form?.display}
              statusMessage={statusMessage}
              target="timer"
              title="计时器透明窗口"
              onPositionSelection={() => void beginPositionSelection("timer")}
              onUpdate={(value) => updateDisplay("timer", value)}
              onUpdateRect={(value) => updateDisplayRect("timer", value)}
            />

            <div className="grid gap-4 xl:grid-cols-3">
              {form?.timers.map((timer, index) => (
                <TimerCard
                  key={timer.id}
                  controlsDisabled={controlsDisabled}
                  index={index}
                  isRecording={recordingTarget?.type === "timer" && recordingTarget.id === timer.id}
                  isDragging={draggingTimerId === timer.id}
                  run={runsById.get(timer.id)}
                  timer={timer}
                  canRemove={form.timers.length > 1}
                  onDragOver={() => moveDraggingTimerOver(timer.id)}
                  onDragStart={() => beginTimerDrag(timer.id)}
                  onBeginHotkeyRecording={() => beginTimerHotkeyRecording(timer)}
                  onHotkeyKeyDown={(event) => handleTimerHotkeyRecorderKeyDown(timer, event)}
                  onRemove={() => removeTimer(timer.id)}
                  onUpdate={(value) => updateTimer(timer.id, value)}
                />
              ))}

              <AddCard controlsDisabled={controlsDisabled || !form} title="添加计时器" description="名称、秒数、计时方向、快捷键均可自定义。" onClick={addTimer} />
            </div>
          </TabsContent>
          <TabsContent value="counters" className="flex flex-col gap-4">
            <DisplaySettingsCard
              controlsDisabled={controlsDisabled || !form?.counterEnabled}
              description="计数器拥有独立透明窗口；按快捷键累加，重置会回到设置的起始数。"
              display={form?.counterDisplay}
              statusMessage={statusMessage}
              target="counter"
              title="计数器透明窗口"
              onPositionSelection={() => void beginPositionSelection("counter")}
              onUpdate={(value) => updateDisplay("counter", value)}
              onUpdateRect={(value) => updateDisplayRect("counter", value)}
            />

            <div className="grid gap-4 xl:grid-cols-3">
              {form?.counters.map((counter, index) => (
                <CounterCard
                  key={counter.id}
                  controlsDisabled={controlsDisabled}
                  counter={counter}
                  canRemove={form.counters.length > 1}
                  index={index}
                  isDragging={draggingCounterId === counter.id}
                  isRecording={recordingTarget?.type === "counter" && recordingTarget.id === counter.id}
                  run={counterRunsByIdMap.get(counter.id)}
                  onBeginHotkeyRecording={() => beginCounterHotkeyRecording(counter)}
                  onDragOver={() => moveDraggingCounterOver(counter.id)}
                  onDragStart={() => beginCounterDrag(counter.id)}
                  onHotkeyKeyDown={(event) => handleCounterHotkeyRecorderKeyDown(counter, event)}
                  onRemove={() => removeCounter(counter.id)}
                  onReset={() => void resetCounter(counter.id)}
                  resetDisabled={controlsDisabled || !form?.counterEnabled}
                  onUpdate={(value) => updateCounter(counter.id, value)}
                />
              ))}

              <AddCard controlsDisabled={controlsDisabled || !form} title="添加计数器" description="名称、起始数、快捷键均可自定义。" onClick={addCounter} />
            </div>
          </TabsContent>
        </div>
      </div>
    </Tabs>
  );
}

type DisplaySettingsCardProps = {
  controlsDisabled: boolean;
  description: string;
  display: TimerSettingsForm["display"] | undefined;
  statusMessage: string;
  target: TimerDisplayTarget;
  title: string;
  onPositionSelection: () => void;
  onUpdate: (value: Partial<TimerSettingsForm["display"]>) => void;
  onUpdateRect: (value: Partial<TimerSettingsForm["display"]["rect"]>) => void;
};

function DisplaySettingsCard({ controlsDisabled, description, display, statusMessage, target, title, onPositionSelection, onUpdate, onUpdateRect }: DisplaySettingsCardProps) {
  return (
    <Card size="sm" className="border-border shadow-sm">
      <CardHeader className="border-b border-border/70">
        <div className="flex items-center gap-2">
          <RiEyeLine className="text-muted-foreground" />
          <div>
            <CardTitle>{title}</CardTitle>
            <CardDescription>{description}</CardDescription>
          </div>
        </div>
      </CardHeader>
      <CardContent className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_auto]">
        <FieldGroup className="gap-4">
          <Field>
            <FieldLabel>字体透明度</FieldLabel>
            <FieldContent>
              <div className="flex items-center gap-4">
                <Slider disabled={controlsDisabled || !display} min={0.1} max={1} step={0.05} value={[Number.parseFloat(display?.fontOpacity ?? "0.9")]} onValueChange={([value]) => onUpdate({ fontOpacity: value.toFixed(2) })} />
                <span className="w-12 text-right font-mono text-sm text-muted-foreground">{display?.fontOpacity ?? "--"}</span>
              </div>
            </FieldContent>
          </Field>
          <Field>
            <FieldLabel>{target === "timer" ? "计时器" : "计数器"}透明窗口显示宽度</FieldLabel>
            <FieldContent>
              <Input disabled={controlsDisabled || !display} inputMode="numeric" min="320" value={display?.rect.width ?? 320} onChange={(event) => onUpdateRect({ width: Number.parseInt(event.currentTarget.value, 10) || 320 })} />
            </FieldContent>
          </Field>
          <div className="rounded-lg border border-border bg-muted/30 px-3 py-3 text-xs text-muted-foreground">{statusMessage}</div>
        </FieldGroup>
        <Button disabled={controlsDisabled} onClick={onPositionSelection} type="button" variant="outline">
          <RiMapPinLine data-icon="inline-start" />
          设置透明窗口位置
        </Button>
      </CardContent>
    </Card>
  );
}

type TimerCardProps = {
  canRemove: boolean;
  controlsDisabled: boolean;
  index: number;
  isDragging: boolean;
  isRecording: boolean;
  run: TimerRunState | undefined;
  timer: TimerItemForm;
  onBeginHotkeyRecording: () => void;
  onDragOver: () => void;
  onDragStart: () => void;
  onHotkeyKeyDown: (event: React.KeyboardEvent<HTMLButtonElement>) => void;
  onRemove: () => void;
  onUpdate: (value: Partial<TimerItemForm>) => void;
};

function TimerCard({ canRemove, controlsDisabled, index, isDragging, isRecording, onBeginHotkeyRecording, onDragOver, onDragStart, onHotkeyKeyDown, onRemove, onUpdate, run, timer }: TimerCardProps) {
  return (
    <Card size="sm" className={isDragging ? "border-primary shadow-sm" : "border-border shadow-sm"} onPointerEnter={onDragOver}>
      <CardHeader className="border-b border-border/70">
        <div className="flex items-start justify-between gap-3">
          <div className="flex min-w-0 items-center gap-2">
            <DragButton controlsDisabled={controlsDisabled} onDragStart={onDragStart} />
            <div className="flex size-6 items-center justify-center rounded-full bg-primary text-xs font-bold text-primary-foreground">{index + 1}</div>
            <RiTimerLine className="text-muted-foreground" />
            <div className="min-w-0">
              <CardTitle>{timer.name || `计时器 ${index + 1}`}</CardTitle>
              <CardDescription>{run ? `${run.status === "finished" ? "已结束" : "运行中"} · ${formatTimerRemaining(run.currentSeconds)}` : "等待快捷键触发"}</CardDescription>
            </div>
          </div>
          <Button disabled={controlsDisabled || !canRemove} onClick={onRemove} size="icon-sm" type="button" variant="ghost">
            <RiDeleteBinLine />
          </Button>
        </div>
      </CardHeader>
      <CardContent>
        <FieldGroup className="gap-4">
          <Field>
            <FieldLabel htmlFor={`${timer.id}-name`}>名称</FieldLabel>
            <FieldContent>
              <Input id={`${timer.id}-name`} disabled={controlsDisabled} value={timer.name} onChange={(event) => onUpdate({ name: event.currentTarget.value })} />
            </FieldContent>
          </Field>
          <Field>
            <FieldLabel htmlFor={`${timer.id}-duration`}>计时秒数</FieldLabel>
            <FieldContent>
              <Input id={`${timer.id}-duration`} disabled={controlsDisabled} inputMode="numeric" min="1" value={timer.durationSeconds} onChange={(event) => onUpdate({ durationSeconds: event.currentTarget.value })} />
            </FieldContent>
          </Field>
          <Field>
            <FieldLabel>计时方向</FieldLabel>
            <FieldContent>
              <ToggleGroup className="w-full" disabled={controlsDisabled} type="single" value={timer.direction} variant="outline" onValueChange={(value) => value ? onUpdate({ direction: value as TimerItemForm["direction"] }) : undefined}>
                <ToggleGroupItem className="min-w-24 flex-1 border-border text-sm font-semibold data-[state=on]:bg-primary data-[state=on]:text-primary-foreground data-[state=on]:shadow-sm" value="countup">正</ToggleGroupItem>
                <ToggleGroupItem className="min-w-24 flex-1 border-border text-sm font-semibold data-[state=on]:bg-primary data-[state=on]:text-primary-foreground data-[state=on]:shadow-sm" value="countdown">反</ToggleGroupItem>
              </ToggleGroup>
            </FieldContent>
          </Field>
          <HotkeyField controlsDisabled={controlsDisabled} id={`${timer.id}-hotkey`} isRecording={isRecording} hotkey={timer.hotkey} onBeginHotkeyRecording={onBeginHotkeyRecording} onHotkeyKeyDown={onHotkeyKeyDown} />
          <div className="rounded-lg border border-border bg-muted/30 px-3 py-3">
            <div className="flex items-center gap-2 text-sm text-foreground">
              <RiKeyboardLine className="text-muted-foreground" />
              <span>{timer.hotkey || "未设置快捷键"}</span>
            </div>
          </div>
        </FieldGroup>
      </CardContent>
    </Card>
  );
}

type CounterCardProps = {
  canRemove: boolean;
  controlsDisabled: boolean;
  counter: CounterItemForm;
  index: number;
  isDragging: boolean;
  isRecording: boolean;
  run: CounterRunState | undefined;
  onBeginHotkeyRecording: () => void;
  onDragOver: () => void;
  onDragStart: () => void;
  onHotkeyKeyDown: (event: React.KeyboardEvent<HTMLButtonElement>) => void;
  onRemove: () => void;
  onReset: () => void;
  resetDisabled: boolean;
  onUpdate: (value: Partial<CounterItemForm>) => void;
};

function CounterCard({ canRemove, controlsDisabled, counter, index, isDragging, isRecording, onBeginHotkeyRecording, onDragOver, onDragStart, onHotkeyKeyDown, onRemove, onReset, onUpdate, resetDisabled, run }: CounterCardProps) {
  return (
    <Card size="sm" className={isDragging ? "border-primary shadow-sm" : "border-border shadow-sm"} onPointerEnter={onDragOver}>
      <CardHeader className="border-b border-border/70">
        <div className="flex items-start justify-between gap-3">
          <div className="flex min-w-0 items-center gap-2">
            <DragButton controlsDisabled={controlsDisabled} onDragStart={onDragStart} />
            <div className="flex size-6 items-center justify-center rounded-full bg-primary text-xs font-bold text-primary-foreground">{index + 1}</div>
            <RiSpeedUpLine className="text-muted-foreground" />
            <div className="min-w-0">
              <CardTitle>{counter.name || `计数器 ${index + 1}`}</CardTitle>
              <CardDescription>当前计数 · {run?.value ?? counter.startValue}</CardDescription>
            </div>
          </div>
          <Button disabled={controlsDisabled || !canRemove} onClick={onRemove} size="icon-sm" type="button" variant="ghost">
            <RiDeleteBinLine />
          </Button>
        </div>
      </CardHeader>
      <CardContent>
        <FieldGroup className="gap-4">
          <Field>
            <FieldLabel htmlFor={`${counter.id}-name`}>名称</FieldLabel>
            <FieldContent>
              <Input id={`${counter.id}-name`} disabled={controlsDisabled} value={counter.name} onChange={(event) => onUpdate({ name: event.currentTarget.value })} />
            </FieldContent>
          </Field>
          <Field>
            <FieldLabel htmlFor={`${counter.id}-start`}>起始数</FieldLabel>
            <FieldContent>
              <Input id={`${counter.id}-start`} disabled={controlsDisabled} inputMode="numeric" value={counter.startValue} onChange={(event) => onUpdate({ startValue: event.currentTarget.value })} />
            </FieldContent>
          </Field>
          <HotkeyField controlsDisabled={controlsDisabled} id={`${counter.id}-hotkey`} isRecording={isRecording} hotkey={counter.hotkey} onBeginHotkeyRecording={onBeginHotkeyRecording} onHotkeyKeyDown={onHotkeyKeyDown} />
          <Button disabled={resetDisabled} onClick={onReset} type="button" variant="outline">
            <RiResetLeftLine data-icon="inline-start" />
            重置为起始数
          </Button>
          <div className="rounded-lg border border-border bg-muted/30 px-3 py-3">
            <div className="flex items-center gap-2 text-sm text-foreground">
              <RiKeyboardLine className="text-muted-foreground" />
              <span>{counter.hotkey || "未设置快捷键"}</span>
            </div>
          </div>
        </FieldGroup>
      </CardContent>
    </Card>
  );
}

function DragButton({ controlsDisabled, onDragStart }: { controlsDisabled: boolean; onDragStart: () => void }) {
  return (
    <Button aria-label="拖动排序" className="cursor-grab active:cursor-grabbing" disabled={controlsDisabled} onPointerDown={(event) => { event.preventDefault(); onDragStart(); }} size="icon-sm" type="button" variant="ghost">
      <span className="text-xs font-bold">↕</span>
    </Button>
  );
}

function HotkeyField({ controlsDisabled, hotkey, id, isRecording, onBeginHotkeyRecording, onHotkeyKeyDown }: { controlsDisabled: boolean; hotkey: string; id: string; isRecording: boolean; onBeginHotkeyRecording: () => void; onHotkeyKeyDown: (event: React.KeyboardEvent<HTMLButtonElement>) => void }) {
  return (
    <Field>
      <FieldLabel htmlFor={id}>快捷键</FieldLabel>
      <FieldContent>
        <Button className="h-auto w-full justify-between gap-4 py-2 font-mono" disabled={controlsDisabled} id={id} onClick={onBeginHotkeyRecording} onKeyDown={onHotkeyKeyDown} type="button" variant="outline">
          <span>{isRecording ? "正在录制，按下快捷键..." : hotkey || "点击录制快捷键"}</span>
          <span className="text-[0.6875rem] text-muted-foreground">{isRecording ? "Esc 取消" : "点击录制"}</span>
        </Button>
      </FieldContent>
    </Field>
  );
}

function AddCard({ controlsDisabled, description, onClick, title }: { controlsDisabled: boolean; description: string; onClick: () => void; title: string }) {
  return (
    <button
      className="flex min-h-64 flex-col items-center justify-center rounded-xl border border-dashed border-border bg-muted/20 p-6 text-center transition-colors hover:bg-muted/35 disabled:cursor-not-allowed disabled:opacity-50"
      disabled={controlsDisabled}
      onClick={onClick}
      type="button"
    >
      <RiAddLine className="mb-3 text-muted-foreground" />
      <span className="text-sm font-medium text-foreground">{title}</span>
      <span className="mt-1 text-xs text-muted-foreground">{description}</span>
    </button>
  );
}

function TimerDisplayOverlay({ isNativeShell }: { isNativeShell: boolean }) {
  const [bootstrap, setBootstrap] = useState<TimerBootstrap | null>(null);

  useOverlayBootstrap(isNativeShell, setBootstrap);

  const runsById = useMemo(() => timerRunsById(bootstrap?.runs ?? []), [bootstrap?.runs]);
  const opacity = bootstrap?.settings.display.fontOpacity ?? 0.92;

  return (
    <div className="flex h-screen w-screen items-start justify-start overflow-hidden bg-transparent p-2 font-mono text-white" style={{ opacity }}>
      <div className="h-full w-full overflow-hidden rounded-xl border border-white/20 bg-black/20 px-3 py-2 shadow-[0_0_24px_rgba(0,0,0,0.35)] backdrop-blur-[1px]">
        {bootstrap?.settings.timers.map((timer) => {
          const run = runsById.get(timer.id);
          const finished = run?.status === "finished";
          const progress = timerProgressPercent(run, timer.durationSeconds);
          return (
            <div key={timer.id} className="relative my-0.5 min-w-0 overflow-hidden rounded-md px-2 py-0.5 text-base font-semibold tracking-wide">
              <Progress aria-label={`${timer.name} 进度`} className="absolute inset-0 h-full rounded-md bg-white/5 [&_[data-slot=progress-indicator]]:bg-primary/45" value={progress} />
              <div className="relative flex min-w-0 items-center justify-between gap-3">
                <span className={cn("min-w-0 truncate", finished ? "text-primary italic" : "text-white")}>{timer.name}</span>
                <span className={finished ? "shrink-0 text-primary italic" : "shrink-0 text-white"}>{formatTimerRemaining(run?.currentSeconds ?? (timer.direction === "countup" ? 0 : timer.durationSeconds))}</span>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

function CounterDisplayOverlay({ isNativeShell }: { isNativeShell: boolean }) {
  const [bootstrap, setBootstrap] = useState<TimerBootstrap | null>(null);

  useOverlayBootstrap(isNativeShell, setBootstrap);

  const counterRunsByIdMap = useMemo(() => counterRunsById(bootstrap?.counterRuns ?? []), [bootstrap?.counterRuns]);
  const opacity = bootstrap?.settings.counterDisplay.fontOpacity ?? 0.92;

  return (
    <div className="flex h-screen w-screen items-start justify-start overflow-hidden bg-transparent p-2 font-mono text-white" style={{ opacity }}>
      <div className="h-full w-full overflow-hidden rounded-xl border border-white/20 bg-black/20 px-3 py-2 shadow-[0_0_24px_rgba(0,0,0,0.35)] backdrop-blur-[1px]">
        {bootstrap?.settings.counters.map((counter) => {
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

function useOverlayBootstrap(isNativeShell: boolean, setBootstrap: (value: TimerBootstrap) => void) {
  useEffect(() => {
    document.body.dataset.overlayMode = "true";
    return () => {
      delete document.body.dataset.overlayMode;
    };
  }, []);

  useEffect(() => {
    if (!isNativeShell) {
      return;
    }

    let disposed = false;
    let unlistenStateChanged: (() => void) | undefined;

    void invoke<TimerBootstrap>("timer_get_bootstrap").then((next) => {
      if (!disposed) {
        setBootstrap(next);
      }
    });

    void listen<TimerBootstrap>("timer://state-changed", (event) => {
      if (!disposed) {
        setBootstrap(event.payload);
      }
    }).then((dispose) => {
      unlistenStateChanged = dispose;
    });

    return () => {
      disposed = true;
      unlistenStateChanged?.();
    };
  }, [isNativeShell, setBootstrap]);
}

function TimerPositionOverlay({ isNativeShell, target }: { isNativeShell: boolean; target: TimerDisplayTarget }) {
  const label = target === "timer" ? "计时器" : "计数器";
  const [statusMessage, setStatusMessage] = useState(`拖动此固定大小框到目标位置，按 Enter 保存，按 Esc 退出修改。关闭${label}总开关后对应透明窗口会隐藏并解绑快捷键。`);
  const [dragStart, setDragStart] = useState<{ mouseX: number; mouseY: number; x: number; y: number } | null>(null);
  const [position, setPosition] = useState({ x: window.screenX, y: window.screenY, width: window.innerWidth });

  useEffect(() => {
    document.body.dataset.overlayMode = "true";
    return () => {
      delete document.body.dataset.overlayMode;
    };
  }, []);

  const commit = useCallback(async () => {
    if (!isNativeShell) {
      return;
    }
    setStatusMessage(`正在保存${label}透明窗口位置...`);
    try {
      await invoke("timer_position_commit");
    } catch (error) {
      setStatusMessage(getErrorMessage(error));
    }
  }, [isNativeShell, label]);

  const cancel = useCallback(async () => {
    if (!isNativeShell) {
      return;
    }
    setStatusMessage(`正在退出${label}透明窗口位置设置...`);
    try {
      await invoke("timer_position_cancel");
    } catch (error) {
      setStatusMessage(getErrorMessage(error));
    }
  }, [isNativeShell, label]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Enter") {
        event.preventDefault();
        void commit();
      }
      if (event.key === "Escape") {
        event.preventDefault();
        void cancel();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [cancel, commit]);

  const moveTo = useCallback(async (x: number, y: number) => {
    setPosition((current) => ({ ...current, x, y }));
    if (!isNativeShell) {
      return;
    }
    try {
      await invoke("timer_position_moved", { x, y });
    } catch (error) {
      setStatusMessage(getErrorMessage(error));
    }
  }, [isNativeShell]);

  return (
    <div
      className="flex h-screen w-screen cursor-move select-none items-center justify-center rounded-xl border-2 border-primary bg-background/82 px-4 py-4 text-foreground shadow-2xl backdrop-blur-md"
      onMouseDown={(event) => {
        if (event.button !== 0) {
          return;
        }
        setDragStart({ mouseX: event.screenX, mouseY: event.screenY, x: position.x, y: position.y });
      }}
      onMouseMove={(event) => {
        if (!dragStart) {
          return;
        }
        void moveTo(dragStart.x + event.screenX - dragStart.mouseX, dragStart.y + event.screenY - dragStart.mouseY);
      }}
      onMouseUp={() => setDragStart(null)}
    >
      <div className="text-center">
        <Badge variant="secondary">{label}透明窗口位置</Badge>
        <p className="mt-3 text-sm font-medium">{statusMessage}</p>
        <p className="mt-2 font-mono text-xs text-muted-foreground">X {position.x} · Y {position.y} · W {position.width}</p>
      </div>
    </div>
  );
}

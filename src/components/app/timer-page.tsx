import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  RiAddLine,
  RiDeleteBinLine,
  RiEyeLine,
  RiKeyboardLine,
  RiMapPinLine,
  RiTimerLine,
} from "@remixicon/react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Field, FieldContent, FieldError, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Slider } from "@/components/ui/slider";
import { Switch } from "@/components/ui/switch";
import type { TimerBootstrap, TimerDisplayMode, TimerItemForm, TimerRunState, TimerSelectionOutcome, TimerSettings, TimerSettingsForm } from "@/components/app/timer-types";
import { TIMER_AUTOSAVE_DELAY_MS } from "@/components/app/timer-types";
import { getErrorMessage } from "@/components/app/morse-utils";
import {
  createTimerItem,
  formatTimerHotkey,
  formatTimerRemaining,
  isTimerDirty,
  parseTimerSettingsForm,
  timerRunsById,
  timerSettingsToForm,
} from "@/components/app/timer-utils";

export function TimerPage({ overlayMode }: { overlayMode?: TimerDisplayMode }) {
  const isNativeShell = useMemo(() => {
    const tauriWindow = window as Window & { __TAURI_INTERNALS__?: unknown };
    return Boolean(tauriWindow.__TAURI_INTERNALS__);
  }, []);

  if (overlayMode === "display") {
    return <TimerDisplayOverlay isNativeShell={isNativeShell} />;
  }

  if (overlayMode === "position") {
    return <TimerPositionOverlay isNativeShell={isNativeShell} />;
  }

  return <TimerWorkbench isNativeShell={isNativeShell} />;
}

function TimerWorkbench({ isNativeShell }: { isNativeShell: boolean }) {
  const [bootstrap, setBootstrap] = useState<TimerBootstrap | null>(null);
  const [form, setForm] = useState<TimerSettingsForm | null>(null);
  const formRef = useRef<TimerSettingsForm | null>(null);
  const [loading, setLoading] = useState(isNativeShell);
  const [saving, setSaving] = useState(false);
  const [recordingTimerId, setRecordingTimerId] = useState<string | null>(null);
  const hotkeyDraftRef = useRef("");
  const [statusMessage, setStatusMessage] = useState(isNativeShell ? "正在加载计时器..." : "浏览器预览模式：当前仅验证布局，原生命令请在桌面端运行。");
  const [pageError, setPageError] = useState<string | null>(null);
  const saveTimeoutRef = useRef<number | null>(null);
  const autosaveVersionRef = useRef(0);

  useEffect(() => {
    formRef.current = form;
  }, [form]);

  useEffect(() => {
    return () => {
      if (saveTimeoutRef.current !== null) {
        window.clearTimeout(saveTimeoutRef.current);
      }
    };
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
          setStatusMessage("计时器已就绪。关闭总开关会隐藏透明窗口并解绑快捷键，配置仍会保留。");
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
      setStatusMessage(`快捷键已触发 ${event.payload.length} 个计时器。`);
    }).then((dispose) => {
      unlistenHotkeyTriggered = dispose;
    });

    return () => {
      disposed = true;
      unlistenStateChanged?.();
      unlistenHotkeyTriggered?.();
    };
  }, [isNativeShell]);

  const dirty = useMemo(() => isTimerDirty(bootstrap, form), [bootstrap, form]);
  const runsById = useMemo(() => timerRunsById(bootstrap?.runs ?? []), [bootstrap?.runs]);
  const controlsDisabled = loading || saving || !isNativeShell;

  const updateForm = useCallback(<K extends keyof TimerSettingsForm>(key: K, value: TimerSettingsForm[K]) => {
    setForm((current) => (current ? { ...current, [key]: value } : current));
  }, []);

  const updateDisplay = useCallback((value: Partial<TimerSettingsForm["display"]>) => {
    setForm((current) => current ? { ...current, display: { ...current.display, ...value } } : current);
  }, []);

  const updateTimer = useCallback((id: string, value: Partial<TimerItemForm>) => {
    setForm((current) => current ? {
      ...current,
      timers: current.timers.map((timer) => timer.id === id ? { ...timer, ...value } : timer),
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
      setStatusMessage(next.settings.enabled ? "计时器设置已保存，快捷键已生效。" : "总开关已关闭：透明窗口隐藏，快捷键已解绑，配置已保留。");
    } catch (error) {
      const message = getErrorMessage(error);
      setPageError(message);
      setStatusMessage(message);
    } finally {
      setSaving(false);
    }
  }, []);

  useEffect(() => {
    if (!isNativeShell || loading || !bootstrap || !form || recordingTimerId) {
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
  }, [bootstrap, dirty, form, isNativeShell, loading, recordingTimerId, saveSettings]);

  const beginHotkeyRecording = useCallback((timer: TimerItemForm) => {
    hotkeyDraftRef.current = timer.hotkey;
    setRecordingTimerId(timer.id);
    setStatusMessage(`正在录制 ${timer.name || "计时器"} 的快捷键，按 Esc 取消。`);
  }, []);

  const handleHotkeyRecorderKeyDown = useCallback((timer: TimerItemForm, event: React.KeyboardEvent<HTMLButtonElement>) => {
    if (recordingTimerId !== timer.id) {
      return;
    }

    if (event.key === "Tab") {
      return;
    }

    event.preventDefault();
    event.stopPropagation();

    if (event.key === "Escape") {
      updateTimer(timer.id, { hotkey: hotkeyDraftRef.current });
      setRecordingTimerId(null);
      setStatusMessage("已取消快捷键录制。");
      return;
    }

    const nextHotkey = formatTimerHotkey(event);
    if (!nextHotkey) {
      setStatusMessage("请按下一个可识别的主键，支持字母、数字、功能键与常用导航键。");
      return;
    }

    updateTimer(timer.id, { hotkey: nextHotkey });
    setRecordingTimerId(null);
    setStatusMessage(`新的快捷键已录制：${nextHotkey}`);
  }, [recordingTimerId, updateTimer]);

  const addTimer = useCallback(() => {
    setForm((current) => current ? {
      ...current,
      timers: [...current.timers, { ...createTimerItem(current.timers.length), durationSeconds: "30" }],
    } : current);
  }, []);

  const removeTimer = useCallback((id: string) => {
    setForm((current) => current && current.timers.length > 1 ? {
      ...current,
      timers: current.timers.filter((timer) => timer.id !== id),
    } : current);
  }, []);

  const beginPositionSelection = useCallback(async () => {
    if (!isNativeShell) {
      setStatusMessage("浏览器预览模式下不可设置透明窗口位置，请在桌面端使用。");
      return;
    }

    setStatusMessage("请在透明位置框中拖动窗口，按 Enter 保存，按 Esc 退出修改。透明窗口为固定宽度 320px。");

    try {
      const outcome = await invoke<TimerSelectionOutcome>("timer_begin_position_selection");
      await syncBootstrap(true);
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
    <div className="flex h-full min-h-0 flex-col bg-background">
      <section className="border-b border-border/70 bg-card/95 px-4 py-4 shadow-sm backdrop-blur-sm xl:px-5">
        <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
          <div className="min-w-0 space-y-2">
            <div className="flex flex-wrap items-center gap-2">
              <Badge variant={form?.enabled ? "default" : "secondary"}>{form?.enabled ? "已开启" : "已关闭"}</Badge>
              {saving ? <Badge variant="outline">保存中</Badge> : dirty ? <Badge variant="outline">待保存</Badge> : <Badge variant="outline">已保存</Badge>}
              {bootstrap?.hotkeyError ? <Badge variant="outline">快捷键异常</Badge> : null}
            </div>
            <div>
              <h1 className="text-lg font-semibold tracking-tight text-foreground">计时器</h1>
              <p className="mt-1 text-sm text-muted-foreground">多个计时器可共享快捷键；关闭总开关会隐藏透明窗口并解绑所有快捷键，但配置会持久化保留。</p>
            </div>
          </div>

          <div className="rounded-lg border border-border/70 bg-background px-4 py-3">
            <div className="flex items-center gap-3">
              <Switch checked={Boolean(form?.enabled)} disabled={controlsDisabled || !form} onCheckedChange={(checked) => updateForm("enabled", checked)} />
              <div>
                <p className="text-sm font-medium text-foreground">计时器总开关</p>
                <p className="mt-1 text-xs text-muted-foreground">关闭后所有计时框功能不可用，快捷键解绑。</p>
              </div>
            </div>
          </div>
        </div>
      </section>

      <div className="flex-1 overflow-y-auto px-4 py-4 xl:px-5">
        <div className="mx-auto flex w-full max-w-6xl flex-col gap-4">
          {pageError ? <FieldError>{pageError}</FieldError> : null}

          <Card size="sm" className="border-border shadow-sm">
            <CardHeader className="border-b border-border/70">
              <div className="flex items-center gap-2">
                <RiEyeLine className="text-muted-foreground" />
                <div>
              <CardTitle>透明窗口</CardTitle>
                  <CardDescription>固定宽度 320px，每个计时器一行；按 Enter 保存位置，按 Esc 退出修改；倒计时仅显示秒数。</CardDescription>
                </div>
              </div>
            </CardHeader>
            <CardContent className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_auto]">
              <FieldGroup className="gap-4">
                <Field>
                  <FieldLabel>字体透明度</FieldLabel>
                  <FieldContent>
                    <div className="flex items-center gap-4">
                      <Slider disabled={controlsDisabled || !form} min={0.1} max={1} step={0.05} value={[Number.parseFloat(form?.display.fontOpacity ?? "0.9")]} onValueChange={([value]) => updateDisplay({ fontOpacity: value.toFixed(2) })} />
                      <span className="w-12 text-right font-mono text-sm text-muted-foreground">{form?.display.fontOpacity ?? "--"}</span>
                    </div>
                  </FieldContent>
                </Field>
                <div className="rounded-lg border border-border bg-muted/30 px-3 py-3 text-xs text-muted-foreground">{statusMessage}</div>
              </FieldGroup>
              <Button disabled={controlsDisabled} onClick={() => void beginPositionSelection()} type="button" variant="outline">
                <RiMapPinLine data-icon="inline-start" />
                设置透明窗口位置
              </Button>
            </CardContent>
          </Card>

          <div className="grid gap-4 xl:grid-cols-2">
            {form?.timers.map((timer, index) => (
              <TimerCard
                key={timer.id}
                controlsDisabled={controlsDisabled}
                index={index}
                isRecording={recordingTimerId === timer.id}
                run={runsById.get(timer.id)}
                timer={timer}
                canRemove={form.timers.length > 1}
                onBeginHotkeyRecording={() => beginHotkeyRecording(timer)}
                onHotkeyKeyDown={(event) => handleHotkeyRecorderKeyDown(timer, event)}
                onRemove={() => removeTimer(timer.id)}
                onUpdate={(value) => updateTimer(timer.id, value)}
              />
            ))}

            <button
              className="flex min-h-64 flex-col items-center justify-center rounded-xl border border-dashed border-border bg-muted/20 p-6 text-center transition-colors hover:bg-muted/35 disabled:cursor-not-allowed disabled:opacity-50"
              disabled={controlsDisabled || !form}
              onClick={addTimer}
              type="button"
            >
              <RiAddLine className="mb-3 text-muted-foreground" />
              <span className="text-sm font-medium text-foreground">添加计时器</span>
              <span className="mt-1 text-xs text-muted-foreground">名称、秒数、快捷键均可自定义。</span>
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

type TimerCardProps = {
  canRemove: boolean;
  controlsDisabled: boolean;
  index: number;
  isRecording: boolean;
  run: TimerRunState | undefined;
  timer: TimerItemForm;
  onBeginHotkeyRecording: () => void;
  onHotkeyKeyDown: (event: React.KeyboardEvent<HTMLButtonElement>) => void;
  onRemove: () => void;
  onUpdate: (value: Partial<TimerItemForm>) => void;
};

function TimerCard({ canRemove, controlsDisabled, index, isRecording, onBeginHotkeyRecording, onHotkeyKeyDown, onRemove, onUpdate, run, timer }: TimerCardProps) {
  return (
    <Card size="sm" className="border-border shadow-sm">
      <CardHeader className="border-b border-border/70">
        <div className="flex items-start justify-between gap-3">
          <div className="flex min-w-0 items-center gap-2">
            <div className="flex h-6 w-6 items-center justify-center rounded-full bg-primary text-xs font-bold text-primary-foreground">{index + 1}</div>
            <RiTimerLine className="text-muted-foreground" />
            <div className="min-w-0">
              <CardTitle>{timer.name || `计时器 ${index + 1}`}</CardTitle>
              <CardDescription>{run ? `${run.status === "finished" ? "已结束" : "运行中"} · ${formatTimerRemaining(run.remainingSeconds)}` : "等待快捷键触发"}</CardDescription>
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
            <FieldLabel htmlFor={`${timer.id}-duration`}>倒计时秒数</FieldLabel>
            <FieldContent>
              <Input id={`${timer.id}-duration`} disabled={controlsDisabled} inputMode="numeric" min="1" value={timer.durationSeconds} onChange={(event) => onUpdate({ durationSeconds: event.currentTarget.value })} />
            </FieldContent>
          </Field>
          <Field>
            <FieldLabel htmlFor={`${timer.id}-hotkey`}>快捷键</FieldLabel>
            <FieldContent>
              <Button className="h-auto w-full justify-between gap-4 py-2 font-mono" disabled={controlsDisabled} id={`${timer.id}-hotkey`} onClick={onBeginHotkeyRecording} onKeyDown={onHotkeyKeyDown} type="button" variant="outline">
                <span>{isRecording ? "正在录制，按下快捷键..." : timer.hotkey || "点击录制快捷键"}</span>
                <span className="text-[0.6875rem] text-muted-foreground">{isRecording ? "Esc 取消" : "点击录制"}</span>
              </Button>
            </FieldContent>
          </Field>
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

function TimerDisplayOverlay({ isNativeShell }: { isNativeShell: boolean }) {
  const [bootstrap, setBootstrap] = useState<TimerBootstrap | null>(null);

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
  }, [isNativeShell]);

  const runsById = useMemo(() => timerRunsById(bootstrap?.runs ?? []), [bootstrap?.runs]);
  const opacity = bootstrap?.settings.display.fontOpacity ?? 0.92;

  return (
    <div className="flex min-h-screen w-screen items-start justify-start bg-transparent p-3 font-mono text-white" style={{ opacity }}>
      <div className="w-full rounded-xl border border-white/20 bg-black/20 px-4 py-3 shadow-[0_0_24px_rgba(0,0,0,0.35)] backdrop-blur-[1px]">
        {bootstrap?.settings.timers.map((timer) => {
          const run = runsById.get(timer.id);
          const finished = run?.status === "finished";
          return (
            <div key={timer.id} className="flex items-center justify-between gap-3 py-1.5 text-lg font-semibold tracking-wide">
              <span className={finished ? "text-primary italic" : "text-white"}>{timer.name}</span>
              <span className={finished ? "text-primary italic" : "text-white"}>{formatTimerRemaining(run?.remainingSeconds ?? timer.durationSeconds)}</span>
            </div>
          );
        })}
      </div>
    </div>
  );
}

function TimerPositionOverlay({ isNativeShell }: { isNativeShell: boolean }) {
  const [statusMessage, setStatusMessage] = useState("拖动此固定大小框到目标位置，按 Enter 保存，按 Esc 退出修改。关闭总开关后透明窗口会隐藏并解绑快捷键。");
  const [dragStart, setDragStart] = useState<{ mouseX: number; mouseY: number; x: number; y: number } | null>(null);
  const [position, setPosition] = useState({ x: window.screenX, y: window.screenY });

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
    setStatusMessage("正在保存计时器透明窗口位置...");
    try {
      await invoke("timer_position_commit");
    } catch (error) {
      setStatusMessage(getErrorMessage(error));
    }
  }, [isNativeShell]);

  const cancel = useCallback(async () => {
    if (!isNativeShell) {
      return;
    }
    setStatusMessage("正在退出计时器透明窗口位置设置...");
    try {
      await invoke("timer_position_cancel");
    } catch (error) {
      setStatusMessage(getErrorMessage(error));
    }
  }, [isNativeShell]);

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
    setPosition({ x, y });
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
        <Badge variant="secondary">计时器透明窗口位置</Badge>
        <p className="mt-3 text-sm font-medium">{statusMessage}</p>
        <p className="mt-2 font-mono text-xs text-muted-foreground">X {position.x} · Y {position.y} · W 320</p>
      </div>
    </div>
  );
}

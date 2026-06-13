import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  RiAddLine,
  RiDeleteBinLine,
  RiSpeedUpLine,
  RiStarFill,
  RiStarLine,
  RiTimerLine,
} from "@remixicon/react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Field, FieldContent, FieldError, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Progress } from "@/components/ui/progress";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { PositionOverlay } from "@/components/ui/position-overlay";
import {
  AddCardButton,
  AppPage,
  CardBody,
  ControlTile,
  DisplaySettingsInline,
  DragButton,
  HotkeyField,
  InlineControl,
  PageHero,
  PagePreviewBanner,
  SaveStateBadge,
  SectionHeader,
  SignalTile,
  TacticalCard,
  SurfaceToggleGroup,
} from "@/components/app/app-ui";
import type { TimerBootstrap, TimerDisplayMode, TimerGroupForm, TimerItemForm, TimerRunState, TimerSelectionOutcome, TimerSettings, TimerSettingsForm, TimerDisplayTarget } from "@/components/app/timer-types";
import { DEFAULT_TIMER_GROUP_ID, TIMER_AUTOSAVE_DELAY_MS } from "@/components/app/timer-types";
import { getErrorMessage } from "@/lib/error-utils";
import { useNativeShell } from "@/hooks/use-native-shell";
import { useAutosave } from "@/hooks/use-autosave";
import { useBootstrapForm } from "@/hooks/use-bootstrap-form";

const TIMER_BOOTSTRAP_SPEC = {
  getBootstrapCommand: "timer_get_bootstrap",
  saveSettingsCommand: "timer_save_settings",
  settingsToForm: timerSettingsToForm,
  parseSettingsForm: parseTimerSettingsForm,
};

import { useHotkeyRecorder } from "@/hooks/use-hotkey-recorder";
import { cn } from "@/lib/utils";
import {
  createTimerGroup,
  createTimerItem,
  formatTimerHotkey,
  isTimerRunActive,
  moveTimerItem,
  parseTimerSettingsForm,
  timerEffectiveTimersByGroup,
  timerProgressPercent,
  timerRunsById,
  timerSettingsToForm,
  useTimerOverlayBootstrap,
} from "@/components/app/timer-utils";
import { useFavorites } from "@/hooks/use-favorites";

export type TimerHighlightTarget = {
  kind: "timer";
  cardId: string;
  /** nonce 用于强制重触发高亮动画（用户重复点击同一卡片） */
  nonce: number;
};

type TimerPageProps = {
  overlayMode?: TimerDisplayMode;
  highlightCardId?: TimerHighlightTarget | null;
};

export function TimerPage({ overlayMode, highlightCardId }: TimerPageProps) {
  const isNativeShell = useNativeShell();
  const overlayGroupId = new URLSearchParams(window.location.search).get("groupId");

  if (overlayMode === "display") {
    return <TimerDisplayOverlay groupId={overlayGroupId ?? DEFAULT_TIMER_GROUP_ID} isNativeShell={isNativeShell} />;
  }

  if (overlayMode === "position") {
    return <TimerPositionOverlay isNativeShell={isNativeShell} />;
  }

  return <TimerWorkbench highlightCardId={highlightCardId ?? null} isNativeShell={isNativeShell} />;
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

function TimerWorkbench({ highlightCardId, isNativeShell }: { highlightCardId: TimerHighlightTarget | null; isNativeShell: boolean }) {
  const bf = useBootstrapForm<TimerBootstrap, TimerSettings, TimerSettingsForm>({
    spec: TIMER_BOOTSTRAP_SPEC,
    isNativeShell,
    loadStatusMessage: "正在加载计时器...",
    readyStatusMessage: "计时器已就绪。总开关控制计时器透明窗口与快捷键，配置会持续保留。",
    previewStatusMessage: "浏览器预览模式：当前仅验证布局，原生命令请在桌面端运行。",
    saveSuccessMessage: (next) => next.settings.timerEnabled
      ? "计时器设置已保存，快捷键已生效。"
      : "计时器已关闭：透明窗口隐藏，快捷键已解绑，配置已保留。",
  });

  const { bootstrap, setBootstrap, form, setForm, isDirty, updateForm, saveSettings, syncBootstrap, loading, saving, pageError, setPageError, statusMessage, setStatusMessage, autosaveVersionRef } = bf;

  const [recordingTarget, setRecordingTarget] = useState<{ type: "timer"; id: string } | null>(null);
  const draggingTimerIdRef = useRef<string | null>(null);
  const [draggingTimerId, setDraggingTimerId] = useState<string | null>(null);
  const favorites = useFavorites();
  const recordingTargetRef = useRef<{} | null>(null);
  recordingTargetRef.current = recordingTarget;

  // 高亮跳转：从收藏页跳过来时滚动到目标卡片并加 1.5s 高亮动画
  useEffect(() => {
    if (!highlightCardId) {
      return;
    }
    const selector = `[data-favorite-card="${highlightCardId.kind}:${highlightCardId.cardId}"]`;
    const handle = window.setTimeout(() => {
      const element = document.querySelector<HTMLElement>(selector);
      if (!element) {
        return;
      }
      element.classList.remove("favorite-highlight");
      // 强制 reflow 重新触发动画
      void element.offsetWidth;
      element.classList.add("favorite-highlight");
      element.scrollIntoView({ behavior: "smooth", block: "center" });
    }, 80);
    return () => {
      window.clearTimeout(handle);
    };
  }, [highlightCardId]);

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
      setStatusMessage(`快捷键已触发 ${event.payload.length} 个计时器。运行中的计时器会忽略重复触发。`);
    }).then((dispose) => {
      unlistenHotkeyTriggered = dispose;
    });

    return () => {
      disposed = true;
      unlistenStateChanged?.();
      unlistenHotkeyTriggered?.();
    };
  }, [isNativeShell]);

  const runsById = useMemo(() => timerRunsById(bootstrap?.runs ?? []), [bootstrap?.runs]);
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

  const recorder = useHotkeyRecorder({
    formatKey: formatTimerHotkey,
    onCommit: (key) => {
      const target = recordingTargetRef.current as { type: "timer"; id: string } | null;
      if (!target) return;
      setRecordingTarget(null);
      updateTimer(target.id, { hotkey: key });
    },
    onCancel: (draft) => {
      const target = recordingTargetRef.current as { type: "timer"; id: string } | null;
      if (!target) return;
      setRecordingTarget(null);
      setForm((current) => {
        if (!current) return current;
        return { ...current, timers: current.timers.map((timer) => timer.id === target.id ? { ...timer, hotkey: draft } : timer) };
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

  return (
      <AppPage className="auto-rows-max">
        <PageHero
          eyebrow="02 / TIMER"
          title="任务时序板"
          description="计时器负责阶段节奏，计数器负责战局累加；透明窗口、定位窗口与快捷键保持双通道隔离。"
          badges={
            <>
              <Badge variant={form?.timerEnabled ? "default" : "secondary"}>计时通道{form?.timerEnabled ? "开启" : "关闭"}</Badge>
              <SaveStateBadge dirty={isDirty} saving={saving} />
              {bootstrap?.hotkeyError ? <Badge variant="outline">快捷键异常</Badge> : null}
            </>
          }
          stats={
            <>
              <SignalTile
                label="计时矩阵"
                value={form?.timers.length ?? 0}
                detail={`${bootstrap?.runs.filter((run) => run.status === "running").length ?? 0} 个运行中`}
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


        <TacticalCard className="col-span-12">
          <SectionHeader
            eyebrow="总控字段"
            icon={<RiSpeedUpLine />}
            title="计时总控"
            description="总开关控制计时器透明窗口与快捷键是否生效。"
          />
          <CardBody className="grid gap-3">
            <div className="grid gap-px border-2 border-[var(--chalk)] bg-[var(--chalk)]">
              <ControlTile className="border-0 flex items-center gap-3 bg-[var(--slate)]">
                <Switch checked={Boolean(form?.timerEnabled)} disabled={controlsDisabled || !form} onCheckedChange={(checked) => updateForm("timerEnabled", checked)} />
                <div className="min-w-0">
                  <p className="font-mono text-xs font-medium tracking-[0.12em] text-[var(--chalk)] uppercase">计时总开关</p>
                  <p className="mt-1 text-xs text-muted-foreground">控制计时器快捷键与透明窗口输出。</p>
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
        <div className="col-span-12 flex flex-col gap-2">
          {form?.timerGroups.map((group) => (
            <DisplaySettingsInline
              key={group.id}
              controlsDisabled={controlsDisabled || !form?.timerEnabled}
              display={group.display}
              group={group}
              canDelete={Boolean(form && form.timerGroups.length > 1 && !form.timers.some((timer) => timer.groupId === group.id))}
              statusMessage={`${group.enabled ? "分组已启用" : "分组已关闭"} · ${timerEffectiveTimersByGroup(form, group.id).length} 张有效卡片`}
              targetLabel="计时器"
              onGroupUpdate={(value) => updateTimerGroup(group.id, value)}
              onGroupDelete={() => removeTimerGroup(group.id)}
              onPositionSelection={() => void beginPositionSelection("timer", group.id)}
              onUpdate={(value) => updateTimerGroupDisplay(group.id, value)}
              onUpdateRect={(value) => updateTimerGroupDisplayRect(group.id, value)}
            />
          ))}
        </div>

        <section className="@container col-span-12 grid min-h-0 gap-3 @xl:grid-cols-2">
          {form?.timers.map((timer, index) => (
            <TimerCard
              key={timer.id}
              controlsDisabled={controlsDisabled}
              index={index}
              isFavorite={favorites.isFavorite("timer", timer.id)}
              isHighlighted={Boolean(highlightCardId && highlightCardId.kind === "timer" && highlightCardId.cardId === timer.id)}
              isRecording={recordingTarget?.type === "timer" && recordingTarget.id === timer.id}
              isDragging={draggingTimerId === timer.id}
              groupOptions={form.timerGroups}
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
          ))}

          <AddCardButton
            className="min-h-36"
            disabled={controlsDisabled || !form}
            title="添加计时器"
            description="名称、秒数、计时方向、快捷键均可自定义。"
            onClick={addTimer}
          />
        </section>
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
        eyebrow={`T-${String(index + 1).padStart(2, "0")} · 计时器`}
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
        // 倒计时多段：进度 = 已消耗 / 总时长
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
        // 多段倒计时结束：已消耗 = 总时长 - 当前池子 (= 总时长 - 总时长 = 0)
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
      // 倒计时：显示已消耗的时间 = 总时长 - 当前池子
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

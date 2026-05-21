import type React from "react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  RiAddLine,
  RiArrowDownLine,
  RiArrowUpLine,
  RiDeleteBinLine,
  RiKeyboardLine,
  RiMapPinLine,
  RiPulseLine,
  RiStopLine,
} from "@remixicon/react";

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogMedia,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Field, FieldContent, FieldDescription, FieldGroup, FieldLabel, FieldTitle } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Kbd } from "@/components/ui/kbd";
import { Separator } from "@/components/ui/separator";
import { Switch } from "@/components/ui/switch";
import type {
  RapidfireBootstrap,
  RapidfireCardForm,
  RapidfireRunState,
  RapidfireSettings,
  RapidfireSettingsForm,
} from "@/components/app/rapidfire-types";
import {
  RAPIDFIRE_AUTOSAVE_DELAY_MS,
  RAPIDFIRE_DISPLAY_MAX_WIDTH,
  RAPIDFIRE_DISPLAY_MIN_WIDTH,
  RAPIDFIRE_MIN_INTERVAL_MS,
  createRapidfireCard,
  formatTriggerKey,
  isRapidfireDirty,
  moveRapidfireCard,
  parseRapidfireSettingsForm,
  rapidfireEnabledCards,
  rapidfireRunsById,
  rapidfireSettingsToForm,
  rapidfireStatusLabel,
  rapidfireStatusVariant,
} from "@/components/app/rapidfire-types";
import { getErrorMessage } from "@/lib/error-utils";
import { useNativeShell } from "@/hooks/use-native-shell";
import { useTimeoutCleanup } from "@/hooks/use-timeout-cleanup";
import { cn } from "@/lib/utils";

type RapidfireDisplayMode = "display" | "position";
type RecordingTarget = { cardId: string; field: "triggerKey" | "targetKey" } | null;

function rapidfireCardId(): string {
  const suffix = `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
  return `rapidfire-${suffix}`;
}

export function RapidfirePage({ overlayMode }: { overlayMode?: RapidfireDisplayMode }) {
  const isNativeShell = useNativeShell();

  if (overlayMode === "display") {
    return <RapidfireDisplayOverlay isNativeShell={isNativeShell} />;
  }

  if (overlayMode === "position") {
    return <RapidfirePositionOverlay isNativeShell={isNativeShell} />;
  }

  return <RapidfireWorkbench isNativeShell={isNativeShell} />;
}

function RapidfireWorkbench({ isNativeShell }: { isNativeShell: boolean }) {
  const [bootstrap, setBootstrap] = useState<RapidfireBootstrap | null>(null);
  const [form, setForm] = useState<RapidfireSettingsForm | null>(null);
  const [loading, setLoading] = useState(isNativeShell);
  const [saving, setSaving] = useState(false);
  const [recordingTarget, setRecordingTarget] = useState<RecordingTarget>(null);
  const [pendingDeleteCard, setPendingDeleteCard] = useState<RapidfireCardForm | null>(null);
  const keyDraftRef = useRef("");
  const [statusMessage, setStatusMessage] = useState(
    isNativeShell ? "正在加载连发器..." : "浏览器预览模式：只显示界面，原生命令请在桌面端运行。",
  );
  const [pageError, setPageError] = useState<string | null>(null);
  const saveTimeoutRef = useTimeoutCleanup();
  const autosaveVersionRef = useRef(0);

  useEffect(() => {
    if (isNativeShell) return;
    setForm({
      rapidfireEnabled: false,
      showOverlay: true,
      overlayWidth: "400",
      overlayPosition: null,
      cards: [createRapidfireCard(rapidfireCardId(), 0)],
    });
  }, [isNativeShell]);

  const syncBootstrap = useCallback(async (syncForm = false) => {
    const next = await invoke<RapidfireBootstrap>("rapidfire_get_bootstrap");
    setBootstrap(next);
    setForm((current) => (syncForm || current === null ? rapidfireSettingsToForm(next.settings) : current));
    setPageError(null);
    return next;
  }, []);

  useEffect(() => {
    if (!isNativeShell) return;

    let disposed = false;
    const load = async () => {
      try {
        setLoading(true);
        const next = await syncBootstrap(true);
        if (!disposed) {
          setForm(rapidfireSettingsToForm(next.settings));
          setStatusMessage("连发器已就绪。按住触发键开始，松开后自动补齐奇数次数。");
        }
      } catch (error) {
        if (!disposed) {
          const message = getErrorMessage(error);
          setPageError(message);
          setStatusMessage(message);
        }
      } finally {
        if (!disposed) setLoading(false);
      }
    };
    void load();
    return () => {
      disposed = true;
    };
  }, [isNativeShell, syncBootstrap]);

  useEffect(() => {
    if (!isNativeShell) return;

    let disposed = false;
    let unlistenStateChanged: (() => void) | undefined;
    let unlistenHotkeyError: (() => void) | undefined;

    void listen<RapidfireBootstrap>("rapidfire://state-changed", (event) => {
      if (disposed) return;
      setBootstrap(event.payload);
    }).then((dispose) => {
      unlistenStateChanged = dispose;
    });

    void listen<string>("rapidfire://hotkey-error", (event) => {
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

  const dirty = useMemo(() => isRapidfireDirty(bootstrap, form), [bootstrap, form]);
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

  const clearStaleConfigError = useCallback(() => {
    if (!pageError) return;
    setPageError(null);
    setStatusMessage("配置已更新，等待自动保存...");
  }, [pageError]);

  const updateForm = useCallback(<K extends keyof RapidfireSettingsForm>(key: K, value: RapidfireSettingsForm[K]) => {
    clearStaleConfigError();
    setForm((current) => (current ? { ...current, [key]: value } : current));
  }, [clearStaleConfigError]);

  const updateCard = useCallback((id: string, value: Partial<RapidfireCardForm>) => {
    clearStaleConfigError();
    setForm((current) =>
      current
        ? {
            ...current,
            cards: current.cards.map((card) => (card.id === id ? { ...card, ...value } : card)),
          }
        : current,
    );
  }, [clearStaleConfigError]);

  const saveSettings = useCallback(async (settingsValue: RapidfireSettings, pendingVersion?: number) => {
    const isStaleSave = () => typeof pendingVersion === "number" && pendingVersion !== autosaveVersionRef.current;

    try {
      setSaving(true);
      const next = await invoke<RapidfireBootstrap>("rapidfire_save_settings", { settingsValue });
      if (isStaleSave()) return;

      setBootstrap(next);
      setForm(rapidfireSettingsToForm(next.settings));
      setPageError(null);
      setStatusMessage(
        next.settings.rapidfireEnabled
          ? "连发器设置已保存，触发键已生效。"
          : "连发器已关闭：触发键已解绑，透明窗口已隐藏，配置已保留。",
      );
    } catch (error) {
      if (isStaleSave()) return;
      const message = getErrorMessage(error);
      setPageError(message);
      setStatusMessage(message);
    } finally {
      setSaving(false);
    }
  }, []);

  useEffect(() => {
    if (!isNativeShell || loading || !bootstrap || !form || recordingTarget) return;
    if (!dirty) return;

    const nextVersion = autosaveVersionRef.current + 1;
    autosaveVersionRef.current = nextVersion;
    const formSnapshot = form;

    saveTimeoutRef.current = window.setTimeout(() => {
      try {
        const settingsValue = parseRapidfireSettingsForm(formSnapshot);
        void saveSettings(settingsValue, nextVersion);
      } catch (error) {
        if (nextVersion !== autosaveVersionRef.current) return;
        const message = getErrorMessage(error);
        setPageError(message);
        setStatusMessage(`保存失败：${message}`);
      }
    }, RAPIDFIRE_AUTOSAVE_DELAY_MS);

    return () => {
      if (saveTimeoutRef.current !== null) {
        window.clearTimeout(saveTimeoutRef.current);
        saveTimeoutRef.current = null;
      }
    };
  }, [bootstrap, dirty, form, isNativeShell, loading, recordingTarget, saveSettings]);

  const beginRecording = useCallback((card: RapidfireCardForm, field: "triggerKey" | "targetKey") => {
    keyDraftRef.current = field === "triggerKey" ? card.triggerKey : card.targetKey;
    setRecordingTarget({ cardId: card.id, field });
    setStatusMessage(`正在录制 ${card.name || "连发器"} 的${field === "triggerKey" ? "触发键" : "目标键"}，按 Esc 取消。`);
  }, []);

  const handleRecorderKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLButtonElement>) => {
      if (!recordingTarget) return;
      if (event.key === "Tab") return;

      event.preventDefault();
      event.stopPropagation();

      if (event.key === "Escape") {
        updateCard(recordingTarget.cardId, { [recordingTarget.field]: keyDraftRef.current });
        setRecordingTarget(null);
        setStatusMessage("已取消按键录制。");
        return;
      }

      const nextKey = formatTriggerKey(event.key);
      if (!nextKey || nextKey.includes("+")) {
        setStatusMessage("请按下一个可识别的单键。");
        return;
      }

      updateCard(recordingTarget.cardId, { [recordingTarget.field]: nextKey });
      setRecordingTarget(null);
      setStatusMessage(`新的按键已录制：${nextKey}`);
    },
    [recordingTarget, updateCard],
  );

  const addCard = useCallback(() => {
    clearStaleConfigError();
    setForm((current) =>
      current
        ? {
            ...current,
            cards: [...current.cards, createRapidfireCard(rapidfireCardId(), current.cards.length)],
          }
        : current,
    );
  }, [clearStaleConfigError]);

  const removeCard = useCallback((id: string) => {
    clearStaleConfigError();
    setForm((current) =>
      current && current.cards.length > 1
        ? {
            ...current,
            cards: current.cards.filter((card) => card.id !== id),
          }
        : current,
    );
    setPendingDeleteCard(null);
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

  const beginPositionSelection = useCallback(async () => {
    if (!isNativeShell) return;
    try {
      setStatusMessage("正在打开连发器透明窗口位置设置...");
      await invoke("rapidfire_begin_position_selection");
      await syncBootstrap();
      setStatusMessage("位置设置已结束。");
    } catch (error) {
      const message = getErrorMessage(error);
      setPageError(message);
      setStatusMessage(message);
    }
  }, [isNativeShell, syncBootstrap]);

  if (!form) {
    return (
      <Empty className="min-h-[360px] border">
        <EmptyMedia variant="icon">
          <RiPulseLine />
        </EmptyMedia>
        <EmptyHeader>
          <EmptyTitle>连发器准备中</EmptyTitle>
          <EmptyDescription>{statusMessage}</EmptyDescription>
        </EmptyHeader>
      </Empty>
    );
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-4">
      {pageError && (
        <Alert variant="destructive">
          <RiPulseLine />
          <AlertTitle>连发器配置未生效</AlertTitle>
          <AlertDescription>{pageError}</AlertDescription>
        </Alert>
      )}

      <section className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_320px]">
        <Card className="border-primary/15 bg-card/95">
          <CardHeader>
            <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
              <div className="min-w-0">
                <div className="flex flex-wrap items-center gap-2">
                  <Badge variant={form.rapidfireEnabled ? "default" : "outline"}>
                    {form.rapidfireEnabled ? "已启用" : "未启用"}
                  </Badge>
                  <Badge variant="secondary">{enabledCount} 张卡片可触发</Badge>
                  {dirty && <Badge variant="outline">待保存</Badge>}
                </div>
                <CardTitle className="mt-3 text-xl">连发器控制台</CardTitle>
                <CardDescription className="mt-1">
                  按住触发键持续触发目标键；松开后如果次数为奇数，会等待一个间隔补齐触发。
                </CardDescription>
              </div>
              <div className="flex flex-wrap items-center gap-2">
                <Button variant="outline" size="sm" disabled={controlsDisabled} onClick={beginPositionSelection}>
                  <RiMapPinLine data-icon="inline-start" />
                  设置位置
                </Button>
                <Button variant="outline" size="sm" disabled={controlsDisabled} onClick={stopAll}>
                  <RiStopLine data-icon="inline-start" />
                  全部停止
                </Button>
                <Button variant="default" size="sm" disabled={controlsDisabled} onClick={addCard}>
                  <RiAddLine data-icon="inline-start" />
                  添加卡片
                </Button>
              </div>
            </div>
          </CardHeader>
          <CardContent>
            <FieldGroup className="grid gap-3 md:grid-cols-3">
              <Field orientation="horizontal" className="rounded-lg border bg-background/60 p-3">
                <Switch
                  id="rapidfireEnabled"
                  checked={form.rapidfireEnabled}
                  disabled={controlsDisabled}
                  onCheckedChange={(checked) => updateForm("rapidfireEnabled", checked)}
                />
                <FieldContent>
                  <FieldLabel htmlFor="rapidfireEnabled">连发器总开关</FieldLabel>
                  <FieldDescription>关闭后解绑触发键并隐藏透明窗口。</FieldDescription>
                </FieldContent>
              </Field>
              <Field orientation="horizontal" className="rounded-lg border bg-background/60 p-3">
                <Switch
                  id="showOverlay"
                  checked={form.showOverlay}
                  disabled={controlsDisabled}
                  onCheckedChange={(checked) => updateForm("showOverlay", checked)}
                />
                <FieldContent>
                  <FieldLabel htmlFor="showOverlay">透明窗口</FieldLabel>
                  <FieldDescription>游戏内只显示启用卡片和运行次数。</FieldDescription>
                </FieldContent>
              </Field>
              <Field className="rounded-lg border bg-background/60 p-3">
                <FieldLabel htmlFor="overlayWidth">透明窗口宽度</FieldLabel>
                <Input
                  id="overlayWidth"
                  className="max-w-32 font-mono"
                  type="number"
                  min={RAPIDFIRE_DISPLAY_MIN_WIDTH}
                  max={RAPIDFIRE_DISPLAY_MAX_WIDTH}
                  value={form.overlayWidth}
                  disabled={controlsDisabled}
                  onChange={(event) => updateForm("overlayWidth", event.target.value)}
                />
                <FieldDescription>{RAPIDFIRE_DISPLAY_MIN_WIDTH}-{RAPIDFIRE_DISPLAY_MAX_WIDTH}px。</FieldDescription>
              </Field>
            </FieldGroup>
          </CardContent>
        </Card>

        <Card className="bg-muted/35">
          <CardHeader>
            <CardTitle>运行态</CardTitle>
            <CardDescription>透明窗口与主界面共享同一状态。</CardDescription>
          </CardHeader>
          <CardContent className="flex flex-col gap-4">
            <div className="grid grid-cols-3 gap-2 text-center">
              <StatusTile label="运行中" value={activeRunCount} />
              <StatusTile label="已启用" value={enabledCount} />
              <StatusTile label="本轮次数" value={totalFireCount} />
            </div>
            <Separator />
            <div className="flex flex-col gap-2">
              <div className="flex items-center justify-between text-xs text-muted-foreground">
                <span>保存状态</span>
                <span>{saving ? "保存中" : dirty ? "待保存" : "已保存"}</span>
              </div>
            </div>
            <p className="text-xs/relaxed text-muted-foreground">{statusMessage}</p>
          </CardContent>
        </Card>
      </section>

      <section className="grid min-h-0 gap-3 xl:grid-cols-2">
        {form.cards.map((card, index) => {
          const run = runsById.get(card.id);
          const isRecording = recordingTarget?.cardId === card.id;
          return (
            <RapidfireCardEditor
              key={card.id}
              card={card}
              index={index}
              total={form.cards.length}
              previousId={form.cards[index - 1]?.id ?? null}
              nextId={form.cards[index + 1]?.id ?? null}
              run={run}
              disabled={controlsDisabled}
              isRecording={isRecording}
              recordingField={recordingTarget?.field}
              onUpdate={updateCard}
              onRecord={beginRecording}
              onRecorderKeyDown={handleRecorderKeyDown}
              onMove={moveCard}
              onDelete={() => setPendingDeleteCard(card)}
            />
          );
        })}
      </section>

      <AlertDialog open={pendingDeleteCard !== null} onOpenChange={(open) => !open && setPendingDeleteCard(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogMedia>
              <RiDeleteBinLine />
            </AlertDialogMedia>
            <AlertDialogTitle>删除连发器卡片？</AlertDialogTitle>
            <AlertDialogDescription>
              将删除「{pendingDeleteCard?.name || "未命名连发器"}」并停止它的运行状态。至少会保留一张卡片。
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>取消</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              onClick={() => pendingDeleteCard && removeCard(pendingDeleteCard.id)}
            >
              删除
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}

function StatusTile({ label, value }: { label: string; value: number }) {
  return (
    <div className="rounded-lg border bg-background/70 px-3 py-2">
      <div className="font-heading text-lg font-semibold tabular-nums">{value}</div>
      <div className="text-xs text-muted-foreground">{label}</div>
    </div>
  );
}

interface RapidfireCardEditorProps {
  card: RapidfireCardForm;
  index: number;
  total: number;
  previousId: string | null;
  nextId: string | null;
  run: RapidfireRunState | undefined;
  disabled: boolean;
  isRecording: boolean;
  recordingField: "triggerKey" | "targetKey" | undefined;
  onUpdate: (id: string, value: Partial<RapidfireCardForm>) => void;
  onRecord: (card: RapidfireCardForm, field: "triggerKey" | "targetKey") => void;
  onRecorderKeyDown: (event: React.KeyboardEvent<HTMLButtonElement>) => void;
  onMove: (activeId: string, overId: string) => void;
  onDelete: () => void;
}

function RapidfireCardEditor({
  card,
  index,
  total,
  previousId,
  nextId,
  run,
  disabled,
  isRecording,
  recordingField,
  onUpdate,
  onRecord,
  onRecorderKeyDown,
  onMove,
  onDelete,
}: RapidfireCardEditorProps) {
  const isRunning = run?.status === "firing";
  const isPending = run?.status === "pendingCompensation";

  return (
    <Card className={cn("transition-colors", isRunning && "border-primary/60 bg-primary/5", isPending && "border-border bg-muted/50")}>
      <CardHeader>
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0 flex-1">
            <div className="flex flex-wrap items-center gap-2">
              <Badge variant={rapidfireStatusVariant(run?.status ?? "idle")}>
                {rapidfireStatusLabel(run?.status ?? "idle")}
                {run && run.status !== "idle" ? ` · ${run.count}` : ""}
              </Badge>
              {!card.enabled && <Badge variant="outline">未启用</Badge>}
            </div>
            <Input
              className="mt-3 max-w-72 font-medium"
              placeholder="卡片名称"
              value={card.name}
              disabled={disabled}
              onChange={(event) => onUpdate(card.id, { name: event.target.value })}
            />
          </div>
          <div className="flex shrink-0 items-center gap-1">
            <Button
              variant="ghost"
              size="icon-sm"
              disabled={disabled || index === 0}
              aria-label="上移卡片"
              onClick={() => previousId && onMove(card.id, previousId)}
            >
              <RiArrowUpLine />
            </Button>
            <Button
              variant="ghost"
              size="icon-sm"
              disabled={disabled || index >= total - 1}
              aria-label="下移卡片"
              onClick={() => nextId && onMove(card.id, nextId)}
            >
              <RiArrowDownLine />
            </Button>
            <Switch
              checked={card.enabled}
              disabled={disabled}
              aria-label="启用卡片"
              onCheckedChange={(checked) => onUpdate(card.id, { enabled: checked })}
            />
            <Button variant="ghost" size="icon-sm" disabled={disabled || total <= 1} onClick={onDelete} aria-label="删除卡片">
              <RiDeleteBinLine />
            </Button>
          </div>
        </div>
      </CardHeader>
      <CardContent>
        <FieldGroup className="grid gap-3 md:grid-cols-3">
          <Field>
            <FieldLabel>触发键</FieldLabel>
            <KeyRecorderButton
              value={card.triggerKey}
              active={isRecording && recordingField === "triggerKey"}
              disabled={disabled}
              onClick={() => onRecord(card, "triggerKey")}
              onKeyDown={onRecorderKeyDown}
            />
            <FieldDescription>按住此键开始连发。</FieldDescription>
          </Field>
          <Field>
            <FieldLabel>目标键</FieldLabel>
            <KeyRecorderButton
              value={card.targetKey}
              active={isRecording && recordingField === "targetKey"}
              disabled={disabled}
              onClick={() => onRecord(card, "targetKey")}
              onKeyDown={onRecorderKeyDown}
            />
            <FieldDescription>连发时触发此键。</FieldDescription>
          </Field>
          <Field>
            <FieldLabel htmlFor={`${card.id}-interval`}>连发间隔</FieldLabel>
            <div className="flex items-center gap-2">
              <Input
                id={`${card.id}-interval`}
                className="max-w-28 font-mono"
                type="number"
                min={RAPIDFIRE_MIN_INTERVAL_MS}
                value={card.intervalMs}
                disabled={disabled}
                onChange={(event) => onUpdate(card.id, { intervalMs: event.target.value })}
              />
              <FieldTitle>ms</FieldTitle>
            </div>
            <FieldDescription>最小 {RAPIDFIRE_MIN_INTERVAL_MS}ms。</FieldDescription>
          </Field>
        </FieldGroup>
      </CardContent>
    </Card>
  );
}

function KeyRecorderButton({
  value,
  active,
  disabled,
  onClick,
  onKeyDown,
}: {
  value: string;
  active: boolean;
  disabled: boolean;
  onClick: () => void;
  onKeyDown: (event: React.KeyboardEvent<HTMLButtonElement>) => void;
}) {
  return (
    <Button
      type="button"
      variant={active ? "default" : "outline"}
      size="sm"
      disabled={disabled}
      className="w-full justify-start font-mono"
      onClick={onClick}
      onKeyDown={onKeyDown}
    >
      <RiKeyboardLine data-icon="inline-start" />
      <span className="truncate">{active ? "按任意键..." : value || "点击录制"}</span>
    </Button>
  );
}

function RapidfireDisplayOverlay({ isNativeShell }: { isNativeShell: boolean }) {
  const [bootstrap, setBootstrap] = useState<RapidfireBootstrap | null>(null);

  useRapidfireOverlayBootstrap(isNativeShell, setBootstrap);

  const runsById = useMemo(() => rapidfireRunsById(bootstrap?.runs ?? []), [bootstrap?.runs]);
  const enabledCards = bootstrap?.settings.cards.filter((card) => card.enabled) ?? [];

  return (
    <div className="flex h-screen w-screen items-start justify-start overflow-hidden bg-transparent p-1 font-mono text-white">
      <div className="h-full w-full overflow-hidden rounded-lg border border-white/15 bg-black/20 px-2.5 py-1.5 shadow-[0_0_20px_rgba(0,0,0,0.32)] backdrop-blur-[1px]">
        {enabledCards.length === 0 ? (
          <div className="flex h-full items-center justify-center text-xs font-semibold text-white/60">连发器未启用</div>
        ) : (
          enabledCards.map((card) => {
            const run = runsById.get(card.id);
            const statusText = run ? rapidfireStatusLabel(run.status) : "空闲";
            const countText = run && run.status !== "idle" ? ` ×${run.count}` : "";

            return (
              <div key={card.id} className="flex min-w-0 items-center justify-between gap-2 py-0.5 text-sm font-semibold tracking-wide">
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

function RapidfirePositionOverlay({ isNativeShell }: { isNativeShell: boolean }) {
  const [statusMessage, setStatusMessage] = useState("拖动此固定大小框到目标位置，按 Enter 保存，按 Esc 退出修改。");
  const [dragStart, setDragStart] = useState<{ mouseX: number; mouseY: number; x: number; y: number } | null>(null);
  const [position, setPosition] = useState({ x: window.screenX, y: window.screenY, width: window.innerWidth });

  useEffect(() => {
    document.body.dataset.overlayMode = "true";
    return () => {
      delete document.body.dataset.overlayMode;
    };
  }, []);

  const commit = useCallback(async () => {
    if (!isNativeShell) return;
    setStatusMessage("正在保存连发器透明窗口位置...");
    try {
      await invoke("rapidfire_position_commit");
    } catch (error) {
      setStatusMessage(getErrorMessage(error));
    }
  }, [isNativeShell]);

  const cancel = useCallback(async () => {
    if (!isNativeShell) return;
    setStatusMessage("正在退出连发器透明窗口位置设置...");
    try {
      await invoke("rapidfire_position_cancel");
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

  const moveTo = useCallback(
    async (x: number, y: number) => {
      setPosition((current) => ({ ...current, x, y }));
      if (!isNativeShell) return;
      try {
        await invoke("rapidfire_position_moved", { x, y });
      } catch (error) {
        setStatusMessage(getErrorMessage(error));
      }
    },
    [isNativeShell],
  );

  return (
    <div
      className="flex h-screen w-screen cursor-move select-none items-center justify-center rounded-xl border-2 border-primary bg-background/82 px-4 py-4 text-foreground shadow-2xl backdrop-blur-md"
      onMouseDown={(event) => {
        if (event.button !== 0) return;
        setDragStart({ mouseX: event.screenX, mouseY: event.screenY, x: position.x, y: position.y });
      }}
      onMouseMove={(event) => {
        if (!dragStart) return;
        void moveTo(dragStart.x + event.screenX - dragStart.mouseX, dragStart.y + event.screenY - dragStart.mouseY);
      }}
      onMouseUp={() => setDragStart(null)}
    >
      <div className="text-center">
        <Badge variant="secondary">连发器透明窗口位置</Badge>
        <p className="mt-3 text-sm font-medium">{statusMessage}</p>
        <p className="mt-2 font-mono text-xs text-muted-foreground">X {position.x} · Y {position.y} · W {position.width}</p>
      </div>
    </div>
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

    void listen<RapidfireBootstrap>("rapidfire://state-changed", (event) => {
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

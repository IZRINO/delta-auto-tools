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
import { toast } from "sonner";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { CardHeader } from "@/components/ui/card";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Field, FieldContent, FieldDescription, FieldGroup, FieldLabel, FieldTitle } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Kbd } from "@/components/ui/kbd";
import { Switch } from "@/components/ui/switch";
import {
  AppPage,
  CardBody,
  ControlTile,
  PageHero,
  SaveStateBadge,
  SectionHeader,
  SignalTile,
  TacticalCard,
} from "@/components/app/app-ui";
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
  rapidfireCardError,
  rapidfireCardStatus,
  rapidfireEnabledCards,
  rapidfireRunsById,
  rapidfireSettingsToForm,
  rapidfireStatusLabel,
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
      if (!nextKey || nextKey.includes("+") || (event.ctrlKey && event.key !== "Control") || (event.metaKey && event.key !== "Meta")) {
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
      <Empty className="min-h-[360px] rounded-xl border border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-card-strong),var(--surface-tile))] backdrop-blur-md">
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
    <AppPage>
      {pageError && (
        <Alert variant="destructive">
          <RiPulseLine />
          <AlertTitle>连发器配置未生效</AlertTitle>
          <AlertDescription>{pageError}</AlertDescription>
        </Alert>
      )}

      <PageHero
        eyebrow="Rapidfire Control"
        title="连发器控制台"
        description="按住触发键持续触发目标键；松开后如果次数为奇数，立即补发一次使次数为偶数。"
        badges={
          <>
            <Badge variant={form.rapidfireEnabled ? "default" : "outline"}>{form.rapidfireEnabled ? "已启用" : "未启用"}</Badge>
            <Badge variant="secondary">{enabledCount} 张卡片可触发</Badge>
            <SaveStateBadge dirty={dirty} saving={saving} />
          </>
        }
        actions={
          <>
            <Button variant="outline" size="sm" disabled={controlsDisabled} onClick={beginPositionSelection}>
              <RiMapPinLine data-icon="inline-start" />
              设置位置
            </Button>
            <Button variant="outline" size="sm" disabled={controlsDisabled} onClick={stopAll}>
              <RiStopLine data-icon="inline-start" />
              全部停止
            </Button>
          </>
        }
        stats={
          <>
            <SignalTile label="运行中" value={activeRunCount} detail="非空闲卡片数量" />
            <SignalTile label="已启用" value={enabledCount} detail="参与触发键监听" />
            <SignalTile label="本轮次数" value={totalFireCount} detail={statusMessage} />
          </>
        }
      />

      <TacticalCard>
        <SectionHeader
          eyebrow="Overlay & Hotkeys"
          icon={<RiPulseLine />}
          title="全局控制"
          description="总开关、透明窗口和显示宽度会自动保存。"
        />
        <CardBody>
          <FieldGroup className="grid gap-3 md:grid-cols-3">
            <ControlTile>
              <Field orientation="horizontal">
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
            </ControlTile>
            <ControlTile>
              <Field orientation="horizontal">
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
            </ControlTile>
            <ControlTile>
              <Field>
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
            </ControlTile>
          </FieldGroup>
        </CardBody>
      </TacticalCard>

      <section className="grid min-h-0 gap-4 xl:grid-cols-2">
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
              isRecording={isRecording}
              recordingField={recordingTarget?.field}
              onUpdate={updateCard}
              onRecord={beginRecording}
              onRecorderKeyDown={handleRecorderKeyDown}
              onMove={moveCard}
              onDelete={() => removeCard(card.id)}
            />
          );
        })}
        <RapidfireAddCard controlsDisabled={controlsDisabled} onClick={addCard} />
      </section>
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
  cardError,
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
  const status = rapidfireCardStatus(card, run, cardError);

  return (
    <TacticalCard
      active={status.active || isRunning || isPending}
      className={cn(
        !card.enabled && !status.error && "opacity-80",
        status.error && "border-destructive/65 bg-[linear-gradient(145deg,color-mix(in_oklch,var(--destructive)_9%,var(--surface-card-strong)),var(--surface-card-strong))] ring-1 ring-destructive/25 hover:border-destructive/75",
      )}
    >
      <SectionHeader
        eyebrow={`Rapid ${String(index + 1).padStart(2, "0")}`}
        icon={<RiPulseLine />}
        title={card.name || `连发器 ${index + 1}`}
        description={`${card.triggerKey || "--"} → ${card.targetKey || "--"} · ${card.intervalMs || "--"}ms`}
        badge={
          <Badge variant={status.variant}>
            {status.label}
          </Badge>
        }
        className={cn(status.error && "border-destructive/30 bg-destructive/10")}
      />
      <CardHeader className="border-b border-[var(--surface-border)] bg-[linear-gradient(180deg,var(--surface-muted),transparent)] pt-0">
        <div className="flex flex-col gap-3">
          <div className="flex items-center justify-between gap-3">
            <Input
              className="max-w-80 bg-[linear-gradient(145deg,color-mix(in_oklch,var(--card)_58%,transparent),var(--surface-tile))] font-medium"
              placeholder="卡片名称"
              value={card.name}
              disabled={disabled}
              onChange={(event) => onUpdate(card.id, { name: event.target.value })}
            />
            <div className="flex shrink-0 items-center gap-1.5">
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
              <Button variant="ghost" size="icon-sm" disabled={disabled} onClick={onDelete} aria-label="删除卡片">
                <RiDeleteBinLine />
              </Button>
            </div>
          </div>
        </div>
      </CardHeader>
      <CardBody>
        {cardError ? (
          <Alert
            variant="destructive"
            className="mb-3 border-destructive/45 bg-[color-mix(in_oklch,var(--destructive)_8%,transparent)]"
          >
            <RiPulseLine />
            <AlertTitle>这张卡片的配置未生效</AlertTitle>
            <AlertDescription>{cardError}</AlertDescription>
          </Alert>
        ) : null}
        <FieldGroup className="grid gap-3 md:grid-cols-3">
          <ControlTile>
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
          </ControlTile>
          <ControlTile>
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
          </ControlTile>
          <ControlTile>
            <Field>
              <FieldLabel htmlFor={`${card.id}-interval`}>连发间隔</FieldLabel>
              <div className="flex items-center gap-2">
                <Input
                  id={`${card.id}-interval`}
                  className="max-w-28 bg-[linear-gradient(145deg,color-mix(in_oklch,var(--card)_58%,transparent),var(--surface-tile))] font-mono"
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
          </ControlTile>
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
      size="default"
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

function RapidfireAddCard({ controlsDisabled, onClick }: { controlsDisabled: boolean; onClick: () => void }) {
  return (
    <button
      type="button"
      disabled={controlsDisabled}
      onClick={onClick}
      className={cn(
        "group flex min-h-72 flex-col items-center justify-center rounded-xl border border-dashed border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-tile),color-mix(in_oklch,var(--card)_38%,transparent))] p-6 text-center transition-all",
        "hover:border-primary/35 hover:bg-[var(--surface-hover)]",
        "disabled:cursor-not-allowed disabled:opacity-50",
      )}
    >
      <span className="mb-4 flex size-11 items-center justify-center rounded-lg border border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-card-strong),var(--surface-tile))] text-primary transition-colors group-hover:border-primary/35 group-hover:bg-primary/5">
        <RiAddLine />
      </span>
      <span className="text-sm font-semibold text-foreground">添加连发器</span>
      <span className="mt-1 max-w-52 text-xs/relaxed text-muted-foreground">
        创建新的触发键、目标键和间隔配置。
      </span>
    </button>
  );
}

function RapidfireDisplayOverlay({ isNativeShell }: { isNativeShell: boolean }) {
  const [bootstrap, setBootstrap] = useState<RapidfireBootstrap | null>(null);

  useRapidfireOverlayBootstrap(isNativeShell, setBootstrap);

  const runsById = useMemo(() => rapidfireRunsById(bootstrap?.runs ?? []), [bootstrap?.runs]);
  const enabledCards = bootstrap?.settings.cards.filter((card) => card.enabled) ?? [];

  return (
    <div className="flex h-screen w-screen items-start justify-start overflow-hidden bg-transparent p-1 font-mono text-white">
      <div className="h-full w-full overflow-hidden rounded-lg border border-white/15 bg-black/20 px-2.5 py-1.5 backdrop-blur-[1px]">
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
      className="flex h-screen w-screen cursor-move select-none items-center justify-center rounded-xl border-2 border-primary bg-background/82 px-4 py-4 text-foreground backdrop-blur-md"
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

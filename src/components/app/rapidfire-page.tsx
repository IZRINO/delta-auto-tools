import type React from "react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
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
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { CardHeader } from "@/components/ui/card";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { Field, FieldContent, FieldDescription, FieldGroup, FieldLabel, FieldTitle } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Kbd } from "@/components/ui/kbd";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import {
  AddCardButton,
  AppPage,
  CardBody,
  ControlTile,
  InlineNotice,
  PageHero,
  SaveStateBadge,
  SectionHeader,
  SignalTile,
  TacticalCard,
} from "@/components/app/app-ui";
import type {
  RapidfireBootstrap,
  RapidfireCardForm,
  RapidfireGroupForm,
  RapidfireRunState,
  RapidfireSettings,
  RapidfireSelectionOutcome,
  RapidfireSettingsForm,
} from "@/components/app/rapidfire-types";
import {
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
  createRapidfireCard,
  createRapidfireGroup,
  DEFAULT_RAPIDFIRE_GROUP_ID,
  formatTriggerKey,
  formatTriggerHotkey,
  isRapidfireDirty,
  moveRapidfireCard,
  parseRapidfireSettingsForm,
  rapidfireEffectiveCardsByGroup,
  rapidfireCardError,
  rapidfireCardStatus,
  rapidfireEnabledCards,
  rapidfireRunsById,
  rapidfireSettingsToForm,
  rapidfireStatusLabel,
} from "@/components/app/rapidfire-types";
import { getErrorMessage } from "@/lib/error-utils";
import { useFavorites } from "@/hooks/use-favorites";
import { useNativeShell } from "@/hooks/use-native-shell";
import { useTimeoutCleanup } from "@/hooks/use-timeout-cleanup";
import { cn } from "@/lib/utils";

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

export function RapidfirePage({ highlightCardId, overlayMode }: RapidfirePageProps) {
  const isNativeShell = useNativeShell();
  const overlayGroupId = new URLSearchParams(window.location.search).get("groupId") ?? DEFAULT_RAPIDFIRE_GROUP_ID;

  if (overlayMode === "display") {
    return <RapidfireDisplayOverlay groupId={overlayGroupId} isNativeShell={isNativeShell} />;
  }

  if (overlayMode === "position") {
    return <RapidfirePositionOverlay isNativeShell={isNativeShell} />;
  }

  return <RapidfireWorkbench highlightCardId={highlightCardId ?? null} isNativeShell={isNativeShell} />;
}

function RapidfireWorkbench({ highlightCardId, isNativeShell }: { highlightCardId: RapidfireHighlightTarget | null; isNativeShell: boolean }) {
  const [bootstrap, setBootstrap] = useState<RapidfireBootstrap | null>(null);
  const [form, setForm] = useState<RapidfireSettingsForm | null>(null);
  const [loading, setLoading] = useState(isNativeShell);
  const [saving, setSaving] = useState(false);
  const [recordingTarget, setRecordingTarget] = useState<RecordingTarget>(null);
  const keyDraftRef = useRef("");
  const draggingCardIdRef = useRef<string | null>(null);
  const [draggingCardId, setDraggingCardId] = useState<string | null>(null);
  const [statusMessage, setStatusMessage] = useState(
    isNativeShell ? "正在加载连发器..." : "浏览器预览模式：只显示界面，原生命令请在桌面端运行。",
  );
  const [pageError, setPageError] = useState<string | null>(null);
  const saveTimeoutRef = useTimeoutCleanup();
  const autosaveVersionRef = useRef(0);
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
        element.scrollIntoView({ behavior: "smooth", block: "center" });
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
          setStatusMessage("连发器已就绪。按住触发键开始；未开启不追加的卡片会在松开后自动补齐奇数次数。");
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

  const updateGroup = useCallback((id: string, value: Partial<RapidfireGroupForm>) => {
    clearStaleConfigError();
    setForm((current) =>
      current
        ? {
            ...current,
            groups: current.groups.map((group) => (group.id === id ? { ...group, ...value } : group)),
            showOverlay: id === DEFAULT_RAPIDFIRE_GROUP_ID && typeof value.showOverlay === "boolean" ? value.showOverlay : current.showOverlay,
            overlayPosition: id === DEFAULT_RAPIDFIRE_GROUP_ID && "overlayPosition" in value ? value.overlayPosition ?? null : current.overlayPosition,
            overlayWidth: id === DEFAULT_RAPIDFIRE_GROUP_ID && typeof value.overlayWidth === "string" ? value.overlayWidth : current.overlayWidth,
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
    setStatusMessage(`正在录制 ${card.name || "连发器"} 的${field === "triggerKey" ? "触发键" : "目标键"}，按下主键会保存；失焦会取消。触发键可按住 Ctrl/Alt/Shift/Win 录制组合键。`);
  }, []);

  const handleRecorderKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLButtonElement>) => {
      if (!recordingTarget) return;
      if (event.key === "Tab") return;

      event.preventDefault();
      event.stopPropagation();


      const nextKey = recordingTarget.field === "triggerKey" ? formatTriggerHotkey(event) : formatTriggerKey(event.key);
      const modifierOnly = ["Control", "Alt", "Shift", "Meta"].includes(event.key);
      if (!nextKey || modifierOnly || (recordingTarget.field === "targetKey" && nextKey.includes("+"))) {
        setStatusMessage(recordingTarget.field === "triggerKey" ? "请按下组合键的主键。" : "目标键必须是单键。");
        return;
      }

      updateCard(recordingTarget.cardId, { [recordingTarget.field]: nextKey });
      setRecordingTarget(null);
      setStatusMessage(`新的按键已录制：${nextKey}`);
    },
    [recordingTarget, updateCard],
  );

  const handleRecorderBlur = useCallback(() => {
    if (!recordingTarget) return;
    const target = recordingTarget;
    setRecordingTarget(null);
    updateCard(target.cardId, { [target.field]: keyDraftRef.current });
    setStatusMessage("已取消按键录制。");
  }, [recordingTarget, updateCard]);

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
      const outcome = await invoke<RapidfireSelectionOutcome>("rapidfire_begin_position_selection", { groupId });
      await syncBootstrap(true);
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
      <AddCardButton
        className="min-h-[360px]"
        disabled
        title="连发器准备中"
        description={statusMessage}
        onClick={() => undefined}
      />
    );
  }

  return (
    <AppPage>
      {pageError && (
        <InlineNotice title="连发器配置未生效">
          {pageError}
        </InlineNotice>
      )}

      <PageHero
        eyebrow="连发控制"
        title="连发器控制台"
        description="按住触发键持续触发目标键；松开后默认补齐奇数次数，也可为单张卡片开启不追加。"
        badges={
          <>
            <Badge variant={form.rapidfireEnabled ? "default" : "outline"}>{form.rapidfireEnabled ? "已启用" : "未启用"}</Badge>
            <Badge variant="secondary">{enabledCount} 张卡片可触发</Badge>
            <SaveStateBadge dirty={dirty} saving={saving} />
          </>
        }
        actions={
          <>
            <Button variant="outline" size="sm" disabled={controlsDisabled} onClick={() => void beginPositionSelection(DEFAULT_RAPIDFIRE_GROUP_ID)}>
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
          eyebrow="快捷键与叠加层"
          icon={<RiPulseLine />}
          title="全局控制"
          description="总开关、透明窗口和补齐延迟会自动保存；按键间距与启动抖动在每张卡片内独立配置。"
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
          <FieldGroup className="mt-3 grid gap-3 md:grid-cols-2">
            <ControlTile>
              <Field>
                <FieldLabel htmlFor="compensationDelayMinMs">补齐延迟下限</FieldLabel>
                <div className="flex items-center gap-2">
                  <Input
                    id="compensationDelayMinMs"
                    className="w-28 bg-[linear-gradient(145deg,color-mix(in_oklch,var(--card)_58%,transparent),var(--surface-tile))] font-mono"
                    type="number"
                    min={RAPIDFIRE_GLOBAL_DELAY_MIN_MS}
                    max={RAPIDFIRE_GLOBAL_DELAY_MAX_MS}
                    value={form.compensationDelayMinMs}
                    disabled={controlsDisabled}
                    onChange={(event) => updateForm("compensationDelayMinMs", event.target.value)}
                  />
                  <FieldTitle>ms</FieldTitle>
                </div>
                <FieldDescription>奇数补齐前随机等待下限。</FieldDescription>
              </Field>
            </ControlTile>
            <ControlTile>
              <Field>
                <FieldLabel htmlFor="compensationDelayMaxMs">补齐延迟上限</FieldLabel>
                <div className="flex items-center gap-2">
                  <Input
                    id="compensationDelayMaxMs"
                    className="w-28 bg-[linear-gradient(145deg,color-mix(in_oklch,var(--card)_58%,transparent),var(--surface-tile))] font-mono"
                    type="number"
                    min={RAPIDFIRE_GLOBAL_DELAY_MIN_MS}
                    max={RAPIDFIRE_GLOBAL_DELAY_MAX_MS}
                    value={form.compensationDelayMaxMs}
                    disabled={controlsDisabled}
                    onChange={(event) => updateForm("compensationDelayMaxMs", event.target.value)}
                  />
                  <FieldTitle>ms</FieldTitle>
                </div>
                <FieldDescription>下限不能大于上限。</FieldDescription>
              </Field>
            </ControlTile>
          </FieldGroup>
        </CardBody>
      </TacticalCard>

      <TacticalCard>
        <SectionHeader
          eyebrow="连发分组"
          icon={<RiPulseLine />}
          title="连发分组"
          description="每个分组拥有独立透明窗口；总开关、分组开关、卡片开关同时开启才会响应触发键。"
          actions={
            <Button type="button" variant="outline" size="sm" disabled={controlsDisabled} onClick={addGroup}>
              <RiAddLine data-icon="inline-start" />
              新增分组
            </Button>
          }
        />
        <CardBody className="grid gap-3 lg:grid-cols-2">
          {form.groups.map((group) => (
            <ControlTile key={group.id} className="flex flex-col gap-3">
              <div className="flex items-start justify-between gap-3">
                <div className="min-w-0">
                  <p className="text-sm font-semibold text-foreground">{group.name}</p>
                  <p className="mt-1 text-xs text-muted-foreground">
                    {rapidfireEffectiveCardsByGroup(form, group.id).length} 张有效卡片
                  </p>
                </div>
                <Switch checked={group.enabled} disabled={controlsDisabled} onCheckedChange={(checked) => updateGroup(group.id, { enabled: checked })} />
              </div>
              <FieldGroup className="grid gap-3 md:grid-cols-2">
                <Field>
                  <FieldLabel>分组名称</FieldLabel>
                  <Input disabled={controlsDisabled} value={group.name} onChange={(event) => updateGroup(group.id, { name: event.currentTarget.value })} />
                </Field>
                <Field>
                  <FieldLabel>透明窗口宽度</FieldLabel>
                  <Input
                    className="font-mono"
                    disabled={controlsDisabled || !group.enabled}
                    max={RAPIDFIRE_DISPLAY_MAX_WIDTH}
                    min={RAPIDFIRE_DISPLAY_MIN_WIDTH}
                    onChange={(event) => updateGroup(group.id, { overlayWidth: event.currentTarget.value })}
                    type="number"
                    value={group.overlayWidth}
                  />
                </Field>
              </FieldGroup>
              <div className="flex flex-wrap items-center gap-2">
                <ControlTile className="flex items-center gap-2 px-2 py-1.5">
                  <Switch checked={group.showOverlay} disabled={controlsDisabled || !group.enabled} onCheckedChange={(checked) => updateGroup(group.id, { showOverlay: checked })} />
                  <span className="text-xs text-muted-foreground">透明窗口</span>
                </ControlTile>
                <Button type="button" variant="outline" size="sm" disabled={controlsDisabled || !group.enabled} onClick={() => void beginPositionSelection(group.id)}>
                  <RiMapPinLine data-icon="inline-start" />
                  设置位置
                </Button>
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  disabled={controlsDisabled || form.groups.length <= 1 || form.cards.some((card) => card.groupId === group.id)}
                  onClick={() => removeGroup(group.id)}
                >
                  <RiDeleteBinLine data-icon="inline-start" />
                  删除空分组
                </Button>
              </div>
            </ControlTile>
          ))}
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
              groupOptions={form.groups}
              isFavorite={favorites.isFavorite("rapidfire", card.id)}
              isHighlighted={Boolean(highlightCardId && highlightCardId.kind === "rapidfire" && highlightCardId.cardId === card.id)}
              isRecording={isRecording}
              isDragging={draggingCardId === card.id}
              recordingField={recordingTarget?.field}
              onUpdate={updateCard}
              onRecord={beginRecording}
              onRecorderKeyDown={handleRecorderKeyDown}
              onRecorderBlur={handleRecorderBlur}
              onMove={moveCard}
              onDragStart={() => beginCardDrag(card.id)}
              onDragOver={() => moveDraggingCardOver(card.id)}
              onDelete={() => removeCard(card.id)}
              onToggleFavorite={() => favorites.toggleFavorite("rapidfire", card.id)}
            />
          );
        })}
        <AddCardButton className="min-h-72" disabled={controlsDisabled} title="添加连发器" description="创建新的触发键、目标键和间隔配置。" onClick={addCard} />
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
        !card.enabled && !status.error && "opacity-80",
        status.error && "border-destructive/65 bg-[linear-gradient(145deg,color-mix(in_oklch,var(--destructive)_9%,var(--surface-card-strong)),var(--surface-card-strong))] ring-1 ring-destructive/25 hover:border-destructive/75",
        isDragging && "ring-2 ring-primary/55",
        isHighlighted && "ring-2 ring-primary/70",
      )}
    >
      <SectionHeader
        eyebrow="连发卡片"
        icon={<RiPulseLine />}
        title={card.name || `连发器 ${index + 1}`}
        description={`${card.triggerKey || "--"} → ${card.targetKey || "--"} · 间隔 ${card.intervalMs || "--"}ms · ${card.skipCompensation ? "不追加" : "自动补齐"}`}
        badge={
          <><Badge variant="outline">{String(index + 1).padStart(2, "0")}</Badge><Badge variant={status.variant}>{status.label}</Badge></>
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
              onChange={(event) => onUpdate(card.id, { name: event.target.value })}
            />
            <Select disabled={disabled} value={card.groupId} onValueChange={(value) => onUpdate(card.id, { groupId: value })}>
              <SelectTrigger className="max-w-44">
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
            <div className="flex shrink-0 items-center gap-1.5">
              <RapidfireCardDragHandle disabled={disabled} onDragStart={onDragStart} />

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
                <RiArrowDownSLine />
              </Button>
              <Switch
                checked={card.enabled}
                disabled={disabled}
                aria-label="启用卡片"
                onCheckedChange={(checked) => onUpdate(card.id, { enabled: checked })}
              />
              <Button
                aria-label={isFavorite ? "取消收藏" : "加入收藏"}
                aria-pressed={isFavorite}
                className={cn(isFavorite ? "text-amber-500" : "text-muted-foreground")}
                data-icon="inline-start"
                disabled={disabled}
                onClick={onToggleFavorite}
                size="icon-sm"
                type="button"
                variant="ghost"
              >
                {isFavorite ? <RiStarFill /> : <RiStarLine />}
              </Button>
              <Button variant="ghost" size="icon-sm" disabled={disabled} onClick={onDelete} aria-label="删除卡片">
                <RiDeleteBinLine />
              </Button>
            </div>
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
          <ControlTile>
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
              <FieldDescription>按住此键开始连发；支持 Shift+- 这类组合键。</FieldDescription>
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
                onBlur={onRecorderBlur}
              />
              <FieldDescription>连发时触发此键。</FieldDescription>
            </Field>
          </ControlTile>
          <ControlTile>
            <Field orientation="horizontal">
              <Switch
                id={`${card.id}-skip-compensation`}
                checked={card.skipCompensation}
                disabled={disabled}
                onCheckedChange={(checked) => onUpdate(card.id, { skipCompensation: checked })}
              />
              <FieldContent>
                <FieldLabel htmlFor={`${card.id}-skip-compensation`}>不追加补齐</FieldLabel>
                <FieldDescription>开启后松开触发键时不再补发，单数次数保持单数。</FieldDescription>
              </FieldContent>
            </Field>
          </ControlTile>
          <ControlTile>
            <Field>
              <FieldLabel htmlFor={`${card.id}-interval`}>连发间隔</FieldLabel>
              <div className="flex items-center gap-2">
                <Input
                  id={`${card.id}-interval`}
                  className="w-28 bg-[linear-gradient(145deg,color-mix(in_oklch,var(--card)_58%,transparent),var(--surface-tile))] font-mono"
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
          <Collapsible defaultOpen={Boolean(cardError)} className="md:col-span-2">
            <ControlTile className="p-0">
              <CollapsibleTrigger asChild>
                <Button className="w-full justify-between px-3" type="button" variant="ghost">
                  高级参数
                  <RiArrowDownSLine className="size-4" />
                </Button>
              </CollapsibleTrigger>
              <CollapsibleContent className="border-t border-[var(--surface-border)] px-3 py-3">
                <FieldGroup className="grid gap-3 md:grid-cols-2">
                  <ControlTile>
                    <Field>
                      <FieldLabel>触发抖动</FieldLabel>
                      <div className="grid grid-cols-[minmax(4.75rem,1fr)_auto_minmax(4.75rem,1fr)_auto] items-center gap-2">
                        <Input
                          id={`${card.id}-jitter-min`}
                          className="min-w-0 bg-[linear-gradient(145deg,color-mix(in_oklch,var(--card)_58%,transparent),var(--surface-tile))] font-mono"
                          type="number"
                          min={RAPIDFIRE_PRESS_JITTER_MIN_MS}
                          max={RAPIDFIRE_PRESS_JITTER_MAX_MS}
                          value={card.pressJitterMinMs}
                          disabled={disabled}
                          aria-label="触发抖动最小值"
                          onChange={(event) => onUpdate(card.id, { pressJitterMinMs: event.target.value })}
                        />
                        <span className="text-xs text-muted-foreground">至</span>
                        <Input
                          id={`${card.id}-jitter-max`}
                          className="min-w-0 bg-[linear-gradient(145deg,color-mix(in_oklch,var(--card)_58%,transparent),var(--surface-tile))] font-mono"
                          type="number"
                          min={RAPIDFIRE_PRESS_JITTER_MIN_MS}
                          max={RAPIDFIRE_PRESS_JITTER_MAX_MS}
                          value={card.pressJitterMaxMs}
                          disabled={disabled}
                          aria-label="触发抖动最大值"
                          onChange={(event) => onUpdate(card.id, { pressJitterMaxMs: event.target.value })}
                        />
                        <FieldTitle>ms</FieldTitle>
                      </div>
                      <FieldDescription>目标键按下保持时间。</FieldDescription>
                    </Field>
                  </ControlTile>
                  <ControlTile>
                    <Field>
                      <FieldLabel htmlFor={`${card.id}-min-spacing`}>当前卡片按键最小间距</FieldLabel>
                      <div className="flex items-center gap-2">
                        <Input
                          id={`${card.id}-min-spacing`}
                          className="w-28 bg-[linear-gradient(145deg,color-mix(in_oklch,var(--card)_58%,transparent),var(--surface-tile))] font-mono"
                          type="number"
                          min={RAPIDFIRE_GLOBAL_DELAY_MIN_MS}
                          max={RAPIDFIRE_GLOBAL_DELAY_MAX_MS}
                          value={card.minPressSpacingMs}
                          disabled={disabled}
                          onChange={(event) => onUpdate(card.id, { minPressSpacingMs: event.target.value })}
                        />
                        <FieldTitle>ms</FieldTitle>
                      </div>
                      <FieldDescription>仅限制这张卡片的目标键触发间距，不拖慢其他卡片。</FieldDescription>
                    </Field>
                  </ControlTile>
                  <ControlTile>
                    <Field>
                      <FieldLabel htmlFor={`${card.id}-trigger-jitter`}>当前卡片启动抖动上限</FieldLabel>
                      <div className="flex items-center gap-2">
                        <Input
                          id={`${card.id}-trigger-jitter`}
                          className="w-28 bg-[linear-gradient(145deg,color-mix(in_oklch,var(--card)_58%,transparent),var(--surface-tile))] font-mono"
                          type="number"
                          min={0}
                          max={1000}
                          value={card.triggerJitterMaxMs}
                          disabled={disabled}
                          onChange={(event) => onUpdate(card.id, { triggerJitterMaxMs: event.target.value })}
                        />
                        <FieldTitle>ms（0=关闭）</FieldTitle>
                      </div>
                      <FieldDescription>按下这张卡片的触发键后，最多等待此时长再开始连发。</FieldDescription>
                    </Field>
                  </ControlTile>
                  <ControlTile>
                    <Field orientation="horizontal">
                      <Switch
                        id={`${card.id}-cancel-jitter`}
                        checked={card.cancelJitterOnRelease}
                        disabled={disabled}
                        onCheckedChange={(checked) => onUpdate(card.id, { cancelJitterOnRelease: checked })}
                      />
                      <FieldContent>
                        <FieldLabel htmlFor={`${card.id}-cancel-jitter`}>抖动期间松手立即触发</FieldLabel>
                        <FieldDescription>仅作用于这张卡片；松开后立即触发一次并进入奇数补齐判断。</FieldDescription>
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
      className="w-full justify-start font-mono"
      onClick={onClick}
      onBlur={onBlur}
      onKeyDown={onKeyDown}
    >
      <RiKeyboardLine data-icon="inline-start" />
      <span className="truncate">{active ? "按任意键..." : value || "点击录制"}</span>
    </Button>
  );
}



function RapidfireDisplayOverlay({ groupId, isNativeShell }: { groupId: string; isNativeShell: boolean }) {
  const [bootstrap, setBootstrap] = useState<RapidfireBootstrap | null>(null);

  useRapidfireOverlayBootstrap(isNativeShell, setBootstrap);

  const runsById = useMemo(() => rapidfireRunsById(bootstrap?.runs ?? []), [bootstrap?.runs]);
  const group = bootstrap?.settings.groups?.find((item) => item.id === groupId);
  const enabledCards = bootstrap?.settings.cards.filter((card) => card.enabled && card.groupId === groupId && (group?.enabled ?? true)) ?? [];

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

function RapidfireCardDragHandle({ disabled, onDragStart }: { disabled: boolean; onDragStart: () => void }) {
  return (
    <Button
      aria-label="拖动排序"
      className="cursor-grab active:cursor-grabbing"
      disabled={disabled}
      onPointerDown={(event) => {
        event.preventDefault();
        onDragStart();
      }}
      size="icon-sm"
      type="button"
      variant="ghost"
    >
      <span aria-hidden className="font-mono text-xs font-bold leading-none">↕</span>
    </Button>
  );
}

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  RiAddLine,
  RiDeleteBinLine,
  RiKeyboardLine,
  RiMapPinLine,
  RiStopLine,
} from "@remixicon/react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { Field, FieldContent, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import type {
  RapidfireBootstrap,
  RapidfireCardForm,
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
  parseRapidfireSettingsForm,
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

// ---- 主工作台 ----

function RapidfireWorkbench({ isNativeShell }: { isNativeShell: boolean }) {
  const [bootstrap, setBootstrap] = useState<RapidfireBootstrap | null>(null);
  const [form, setForm] = useState<RapidfireSettingsForm | null>(null);
  const [loading, setLoading] = useState(isNativeShell);
  const [saving, setSaving] = useState(false);
  const [recordingTarget, setRecordingTarget] = useState<{ cardId: string; field: "triggerKey" | "targetKey" } | null>(null);
  const keyDraftRef = useRef("");
  const [statusMessage, setStatusMessage] = useState(
    isNativeShell ? "正在加载连发器..." : "浏览器预览模式：当前仅验证布局，原生命令请在桌面端运行。",
  );
  const [pageError, setPageError] = useState<string | null>(null);
  const saveTimeoutRef = useTimeoutCleanup();
  const autosaveVersionRef = useRef(0);

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
          setStatusMessage("连发器已就绪。总开关控制快捷键注册与透明窗口显示，配置会持续保留。");
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
    return () => { disposed = true; };
  }, [isNativeShell, syncBootstrap]);

  useEffect(() => {
    if (!isNativeShell) return;

    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen<RapidfireBootstrap>("rapidfire://state-changed", (event) => {
      if (disposed) return;
      setBootstrap(event.payload);
    }).then((d) => { unlisten = d; });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [isNativeShell]);

  const dirty = useMemo(() => isRapidfireDirty(bootstrap, form), [bootstrap, form]);
  const runsById = useMemo(() => rapidfireRunsById(bootstrap?.runs ?? []), [bootstrap?.runs]);
  const controlsDisabled = loading || saving || !isNativeShell;

  const updateForm = useCallback(<K extends keyof RapidfireSettingsForm>(key: K, value: RapidfireSettingsForm[K]) => {
    setForm((current) => (current ? { ...current, [key]: value } : current));
  }, []);

  const updateCard = useCallback((id: string, value: Partial<RapidfireCardForm>) => {
    setForm((current) => current ? {
      ...current,
      cards: current.cards.map((c) => (c.id === id ? { ...c, ...value } : c)),
    } : current);
  }, []);

  const saveSettings = useCallback(async (settingsValue: RapidfireSettings, pendingVersion?: number) => {
    try {
      setSaving(true);
      const next = await invoke<RapidfireBootstrap>("rapidfire_save_settings", { settingsValue });
      if (typeof pendingVersion === "number" && pendingVersion !== autosaveVersionRef.current) return;
      setBootstrap(next);
      setForm(rapidfireSettingsToForm(next.settings));
      setPageError(null);
      setStatusMessage(next.settings.rapidfireEnabled ? "连发器设置已保存，快捷键已生效。" : "连发器已关闭：快捷键已解绑，透明窗口已隐藏，配置已保留。");
    } catch (error) {
      const message = getErrorMessage(error);
      setPageError(message);
      setStatusMessage(message);
    } finally {
      setSaving(false);
    }
  }, []);

  // Autosave
  useEffect(() => {
    if (!isNativeShell || loading || !bootstrap || !form || recordingTarget) return;
    if (!dirty) return;

    const nextVersion = autosaveVersionRef.current + 1;
    autosaveVersionRef.current = nextVersion;
    const formSnapshot = form;

    saveTimeoutRef.current = window.setTimeout(() => {
      try {
        void saveSettings(parseRapidfireSettingsForm(formSnapshot), nextVersion);
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

  // Hotkey recording
  const beginRecording = useCallback((card: RapidfireCardForm, field: "triggerKey" | "targetKey") => {
    keyDraftRef.current = field === "triggerKey" ? card.triggerKey : card.targetKey;
    setRecordingTarget({ cardId: card.id, field });
    setStatusMessage(`正在录制 ${card.name || "连发器"} 的${field === "triggerKey" ? "触发键" : "目标键"}，按 Esc 取消。`);
  }, []);

  const handleRecorderKeyDown = useCallback((event: React.KeyboardEvent<HTMLButtonElement>) => {
    if (!recordingTarget) return;
    if (event.key === "Tab") return;

    event.preventDefault();
    event.stopPropagation();

    if (event.key === "Escape") {
      updateCard(recordingTarget.cardId, { [recordingTarget.field]: keyDraftRef.current });
      setRecordingTarget(null);
      setStatusMessage("已取消键录制。");
      return;
    }

    const nextKey = formatTriggerKey(event.key.length === 1 ? event.key.toUpperCase() : event.key);
    if (!nextKey) {
      setStatusMessage("请按下一个可识别的按键。");
      return;
    }

    updateCard(recordingTarget.cardId, { [recordingTarget.field]: nextKey });
    setRecordingTarget(null);
    setStatusMessage(`新的按键已录制：${nextKey}`);
  }, [recordingTarget, updateCard]);

  const addCard = useCallback(() => {
    setForm((current) => current ? {
      ...current,
      cards: [...current.cards, createRapidfireCard(rapidfireCardId())],
    } : current);
  }, []);

  const removeCard = useCallback((id: string) => {
    setForm((current) => current && current.cards.length > 1 ? {
      ...current,
      cards: current.cards.filter((c) => c.id !== id),
    } : current);
  }, []);

  const stopAll = useCallback(async () => {
    if (!isNativeShell) return;
    try {
      setStatusMessage("正在停止所有连发...");
      const next = await invoke<RapidfireBootstrap>("rapidfire_stop");
      setBootstrap(next);
      setStatusMessage("已停止所有连发。");
    } catch (error) {
      setStatusMessage(getErrorMessage(error));
    }
  }, [isNativeShell]);

  const beginPositionSelection = useCallback(async () => {
    if (!isNativeShell) return;
    try {
      setStatusMessage("正在打开位置设置窗口...");
      await invoke("rapidfire_begin_position_selection");
      setStatusMessage("位置设置窗口已关闭。");
    } catch (error) {
      setStatusMessage(getErrorMessage(error));
    }
  }, [isNativeShell]);

  const triggerKeyLabel = "触发键 (按下此键启动连发)";
  const targetKeyLabel = "目标键 (连发时触发的按键)";
  const intervalLabel = "连发间隔 (ms)";

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-5">
      {pageError && (
        <div className="rounded-lg border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{pageError}</div>
      )}

      {/* 总开关行 */}
      <div className="flex flex-wrap items-center gap-4 rounded-lg border border-border bg-background px-4 py-3">
        <div className="flex items-center gap-2">
          <Switch
            id="rapidfireEnabled"
            checked={form?.rapidfireEnabled ?? false}
            disabled={controlsDisabled}
            onCheckedChange={(checked) => updateForm("rapidfireEnabled", checked)}
          />
          <label htmlFor="rapidfireEnabled" className="text-sm font-medium cursor-pointer select-none">连发器总开关</label>
        </div>
        <div className="flex items-center gap-2">
          <Switch
            id="showOverlay"
            checked={form?.showOverlay ?? false}
            disabled={controlsDisabled}
            onCheckedChange={(checked) => updateForm("showOverlay", checked)}
          />
          <label htmlFor="showOverlay" className="text-sm font-medium cursor-pointer select-none">透明窗口</label>
        </div>
        <Button variant="outline" size="sm" disabled={controlsDisabled} onClick={stopAll}>
          <RiStopLine />
          全部停止
        </Button>
        <Button variant="outline" size="sm" disabled={controlsDisabled} onClick={beginPositionSelection}>
          <RiMapPinLine />
          设置位置
        </Button>
      </div>

      {/* 卡片列表 */}
      <div className="flex min-h-0 flex-1 flex-col gap-3">
        {form?.cards.map((card) => {
          const run = runsById.get(card.id);
          const isRecording = recordingTarget?.cardId === card.id;
          const recordingField = recordingTarget?.field;

          return (
            <Card key={card.id} className={cn(run?.status === "firing" && "border-primary/50")}>
              <CardHeader className="pb-2">
                <div className="flex items-center justify-between gap-3">
                  <div className="flex items-center gap-2 min-w-0">
                    <Input
                      className="max-w-[160px]"
                      placeholder="卡片名称"
                      value={card.name}
                      disabled={controlsDisabled}
                      onChange={(e) => updateCard(card.id, { name: e.target.value })}
                    />
                    {run && (
                      <Badge variant={rapidfireStatusVariant(run.status)}>
                        {rapidfireStatusLabel(run.status)}
                        {run.status !== "idle" && ` · ${run.count}`}
                      </Badge>
                    )}
                  </div>
                  <div className="flex items-center gap-1 shrink-0">
                    <Switch
                      checked={card.enabled}
                      disabled={controlsDisabled}
                      onCheckedChange={(checked) => updateCard(card.id, { enabled: checked })}
                    />
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      disabled={controlsDisabled || form.cards.length <= 1}
                      onClick={() => removeCard(card.id)}
                    >
                      <RiDeleteBinLine />
                    </Button>
                  </div>
                </div>
              </CardHeader>
              <CardContent className="pt-0">
                <FieldGroup className="!gap-3">
                  <Field>
                    <FieldLabel>{triggerKeyLabel}</FieldLabel>
                    <FieldContent>
                      <Button
                        variant={isRecording && recordingField === "triggerKey" ? "default" : "outline"}
                        size="sm"
                        disabled={controlsDisabled}
                        className="min-w-[140px] justify-start font-mono"
                        onKeyDown={handleRecorderKeyDown}
                        onClick={() => beginRecording(card, "triggerKey")}
                      >
                        <RiKeyboardLine />
                        <span className="truncate">{isRecording && recordingField === "triggerKey" ? "按任意键..." : (card.triggerKey || "点击录制")}</span>
                      </Button>
                    </FieldContent>
                  </Field>
                  <Field>
                    <FieldLabel>{targetKeyLabel}</FieldLabel>
                    <FieldContent>
                      <Button
                        variant={isRecording && recordingField === "targetKey" ? "default" : "outline"}
                        size="sm"
                        disabled={controlsDisabled}
                        className="min-w-[140px] justify-start font-mono"
                        onKeyDown={handleRecorderKeyDown}
                        onClick={() => beginRecording(card, "targetKey")}
                      >
                        <RiKeyboardLine />
                        <span className="truncate">{isRecording && recordingField === "targetKey" ? "按任意键..." : (card.targetKey || "点击录制")}</span>
                      </Button>
                    </FieldContent>
                  </Field>
                  <Field>
                    <FieldLabel>{intervalLabel}</FieldLabel>
                    <FieldContent>
                      <Input
                        className="max-w-[120px] font-mono"
                        type="number"
                        min={RAPIDFIRE_MIN_INTERVAL_MS}
                        value={card.intervalMs}
                        disabled={controlsDisabled}
                        onChange={(e) => updateCard(card.id, { intervalMs: e.target.value })}
                      />
                    </FieldContent>
                  </Field>
                </FieldGroup>
              </CardContent>
            </Card>
          );
        })}
      </div>

      {/* 宽度设置 + 添加 */}
      <div className="flex flex-wrap items-center gap-4 rounded-lg border border-border bg-background px-4 py-3">
        <Field className="!flex-row !items-center !gap-2">
          <FieldLabel className="!text-sm shrink-0">透明窗口宽度</FieldLabel>
          <Input
            className="max-w-[90px] font-mono"
            type="number"
            min={RAPIDFIRE_DISPLAY_MIN_WIDTH}
            max={RAPIDFIRE_DISPLAY_MAX_WIDTH}
            value={form?.overlayWidth ?? "400"}
            disabled={controlsDisabled}
            onChange={(e) => updateForm("overlayWidth", e.target.value)}
          />
        </Field>
        <Button variant="outline" size="sm" disabled={controlsDisabled} onClick={addCard}>
          <RiAddLine />
          添加卡片
        </Button>
      </div>

      {/* 状态栏 */}
      <div className="rounded-lg border border-border bg-muted/50 px-4 py-2.5">
        <p className="text-xs text-muted-foreground leading-relaxed">{statusMessage}</p>
      </div>
    </div>
  );
}

// ---- 透明窗口 ----

function RapidfireDisplayOverlay({ isNativeShell }: { isNativeShell: boolean }) {
  const [bootstrap, setBootstrap] = useState<RapidfireBootstrap | null>(null);

  useRapidfireOverlayBootstrap(isNativeShell, setBootstrap);

  const runsById = useMemo(() => rapidfireRunsById(bootstrap?.runs ?? []), [bootstrap?.runs]);

  return (
    <div className="flex h-screen w-screen items-start justify-start overflow-hidden bg-transparent p-1.5 font-mono text-white">
      <div className="h-full w-full overflow-hidden rounded-lg border border-white/15 bg-black/15 px-2.5 py-1.5 shadow-[0_0_20px_rgba(0,0,0,0.3)] backdrop-blur-[1px]">
        {bootstrap?.settings.cards.filter((c) => c.enabled).map((card) => {
          const run = runsById.get(card.id);
          const statusText = run ? rapidfireStatusLabel(run.status) : "空闲";
          const countText = run && run.status !== "idle" ? ` ×${run.count}` : "";

          return (
            <div key={card.id} className="flex min-w-0 items-center justify-between gap-2 py-0.5 text-sm font-semibold tracking-wide">
              <span className="min-w-0 truncate text-white/90">{card.triggerKey} → {card.targetKey}</span>
              <span className={cn(
                "shrink-0",
                run?.status === "firing" && "text-green-400",
                run?.status === "pendingCompensation" && "text-yellow-400",
                (!run || run.status === "idle") && "text-white/60",
              )}>
                {statusText}{countText}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}

// ---- 位置设置窗口 ----

function RapidfirePositionOverlay({ isNativeShell }: { isNativeShell: boolean }) {
  const [statusMessage, setStatusMessage] = useState("拖动此固定大小框到目标位置，按 Enter 保存，按 Esc 退出修改。");
  const [dragStart, setDragStart] = useState<{ mouseX: number; mouseY: number; x: number; y: number } | null>(null);
  const [position, setPosition] = useState({ x: window.screenX, y: window.screenY, width: window.innerWidth });

  useEffect(() => {
    document.body.dataset.overlayMode = "true";
    return () => { delete document.body.dataset.overlayMode; };
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
      if (event.key === "Enter") { event.preventDefault(); void commit(); }
      if (event.key === "Escape") { event.preventDefault(); void cancel(); }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [cancel, commit]);

  const moveTo = useCallback(async (x: number, y: number) => {
    setPosition((current) => ({ ...current, x, y }));
    if (!isNativeShell) return;
    try {
      await invoke("rapidfire_position_moved", { x, y });
    } catch (error) {
      setStatusMessage(getErrorMessage(error));
    }
  }, [isNativeShell]);

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
    return () => { delete document.body.dataset.overlayMode; };
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
    }).then((dispose) => { unlistenStateChanged = dispose; });

    return () => {
      disposed = true;
      unlistenStateChanged?.();
    };
  }, [isNativeShell, setBootstrap]);
}

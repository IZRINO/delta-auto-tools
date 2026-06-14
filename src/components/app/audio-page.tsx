import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listenEvent, AUDIO_EVENTS } from "@/lib/tauri-events";
import {
  RiAddLine,
  RiDeleteBinLine,
  RiPlayLine,
  RiVolumeUpLine,
  RiCloseLine,
  RiCheckLine,
} from "@remixicon/react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Field, FieldContent, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import {
  AppPage,
  CardBody,
  ControlTile,
  MacroHeader,
  PagePreviewBanner,
  SaveStateBadge,
  SectionHeader,
  SignalTile,
  TacticalCard,
} from "@/components/app/app-ui";
import type { AudioBootstrap, AudioCard, AudioSettings, AudioSettingsForm } from "@/components/app/audio-types";
import { AUDIO_AUTOSAVE_DELAY_MS } from "@/components/app/audio-types";
import { createEmptyAudioCard, parseSettingsForm, settingsToForm } from "@/components/app/audio-utils";
import { getErrorMessage, getSelectionRect } from "@/components/app/morse-utils";
import type { Point } from "@/components/app/morse-types";
import { useNativeShell } from "@/hooks/use-native-shell";
import { useBootstrapForm } from "@/hooks/use-bootstrap-form";
import { useAutosave } from "@/hooks/use-autosave";

const AUDIO_BOOTSTRAP_SPEC = {
  getBootstrapCommand: "audio_get_bootstrap",
  saveSettingsCommand: "audio_save_settings",
  settingsToForm,
  parseSettingsForm,
};

export function AudioPage() {
  const isNativeShell = useNativeShell();
  return <AudioWorkbench isNativeShell={isNativeShell} />;
}

function AudioWorkbench({ isNativeShell }: { isNativeShell: boolean }) {
  const {
    form,
    setForm,
    isDirty,
    updateForm,
    saveSettings,
    syncBootstrap,
    loading,
    saving,
    pageError,
    statusMessage,
    autosaveVersionRef,
  } = useBootstrapForm<AudioBootstrap, AudioSettings, AudioSettingsForm>({
    spec: AUDIO_BOOTSTRAP_SPEC,
    isNativeShell,
    loadStatusMessage: "正在加载音频设置...",
    readyStatusMessage: "音频模块就绪。",
  });

  useAutosave<AudioSettingsForm>({
    form,
    isDirty,
    disabled: !isNativeShell || loading || !form,
    onSave: (formSnapshot, nextVersion) => saveSettings(parseSettingsForm(formSnapshot), nextVersion),
    onError: (message) => {
      toast.error(`保存失败：${message}`);
    },
    delay: AUDIO_AUTOSAVE_DELAY_MS,
    autosaveVersionRef,
  });

  useEffect(() => {
    if (!isNativeShell) return;
    const unlisten = listenEvent(AUDIO_EVENTS.stateChanged, () => {
      void syncBootstrap({ syncForm: false });
    });
    return () => {
      void unlisten.then((f) => f());
    };
  }, [isNativeShell, syncBootstrap]);

  const handleAddCard = useCallback(() => {
    setForm((current) => {
      if (!current) return current;
      const newCard = cardToForm(createEmptyAudioCard());
      return { ...current, cards: [...current.cards, newCard] };
    });
  }, [setForm]);

  const handleRemoveCard = useCallback((index: number) => {
    setForm((current) => {
      if (!current) return current;
      return { ...current, cards: current.cards.filter((_, i) => i !== index) };
    });
  }, [setForm]);

  const handleUpdateCard = useCallback(
    (index: number, patch: Partial<AudioSettingsForm["cards"][number]>) => {
      setForm((current) => {
        if (!current) return current;
        const nextCards = current.cards.map((card, i) => (i === index ? { ...card, ...patch } : card));
        return { ...current, cards: nextCards };
      });
    },
    [setForm],
  );

  const handleTestPlay = useCallback(
    async (cardId: string) => {
      if (!isNativeShell) return;
      try {
        await invoke("audio_test_play", { cardId });
        toast.success("播放测试已触发");
      } catch (error) {
        toast.error(getErrorMessage(error));
      }
    },
    [isNativeShell],
  );

  const handleBeginRegionSelection = useCallback(
    async (cardId: string) => {
      if (!isNativeShell) return;
      try {
        await invoke("audio_begin_region_selection", { cardId });
      } catch (error) {
        toast.error(getErrorMessage(error));
      }
    },
    [isNativeShell],
  );

  const enabled = form?.audioEnabled ?? false;
  const cardCount = form?.cards.length ?? 0;
  const activeCards = form?.cards.filter((c) => c.enabled).length ?? 0;

  return (
    <AppPage>
      <MacroHeader
        code="A-04"
        title="AUDIO / 音频"
        verticalLabel="音频"
        subtitle="快捷键触发或区域监听+图像匹配触发音频播放。"
        badges={
          <>
            <Badge variant={enabled ? "default" : "outline"}>{enabled ? "已启用" : "已禁用"}</Badge>
            <Badge variant="secondary">{activeCards} 卡片激活</Badge>
            <SaveStateBadge dirty={isDirty} saving={saving} />
          </>
        }
        actions={
          <>
            <SignalTile label="总开关" value={enabled ? "ON" : "OFF"} detail={statusMessage} />
            <SignalTile label="卡片数" value={cardCount} detail="已配置" />
          </>
        }
      />

      {pageError && (
        <div className="mb-3 border-2 border-[var(--alert-red)] bg-[var(--alert-red)]/10 px-3 py-2 font-mono text-xs font-black tracking-[0.12em] text-[var(--alert-red)] uppercase">
          [ 错误 ] {pageError}
        </div>
      )}

      {!isNativeShell && <PagePreviewBanner />}

      <TacticalCard className="mt-3">
        <SectionHeader eyebrow="全局设置" title="全局设置" />
        <CardBody>
          <ControlTile>
            <div className="flex items-center gap-3">
              <Switch
                checked={enabled}
                onCheckedChange={(v) => updateForm("audioEnabled", v)}
                aria-label="音频总开关"
              />
              <span className="font-mono text-xs font-black tracking-[0.14em] uppercase text-[var(--chalk)]">
                {enabled ? "已启用" : "已禁用"}
              </span>
            </div>
          </ControlTile>
        </CardBody>
      </TacticalCard>

      <TacticalCard className="mt-3">
        <SectionHeader
          eyebrow="音频卡片"
          title="音频卡片"
          actions={
            <Button variant="secondary" size="sm" onClick={handleAddCard} data-icon="inline-start">
              <RiAddLine className="size-4" aria-hidden="true" />
              新增卡片
            </Button>
          }
        />
        <CardBody>
          {form?.cards.length === 0 && (
            <div className="py-8 text-center font-mono text-xs font-black tracking-[0.14em] text-[var(--zinc)] uppercase">
              [ 无音频卡片 ] 点击上方按钮添加
            </div>
          )}
          <div className="space-y-3">
            {form?.cards.map((card, index) => (
              <AudioCardEditor
                key={card.id}
                card={card}
                index={index}
                onUpdate={(patch) => handleUpdateCard(index, patch)}
                onRemove={() => handleRemoveCard(index)}
                onTestPlay={() => handleTestPlay(card.id)}
                onBeginRegionSelection={() => handleBeginRegionSelection(card.id)}
              />
            ))}
          </div>
        </CardBody>
      </TacticalCard>
    </AppPage>
  );
}

function AudioCardEditor({
  card,
  index,
  onUpdate,
  onRemove,
  onTestPlay,
  onBeginRegionSelection,
}: {
  card: AudioSettingsForm["cards"][number];
  index: number;
  onUpdate: (patch: Partial<AudioSettingsForm["cards"][number]>) => void;
  onRemove: () => void;
  onTestPlay: () => void;
  onBeginRegionSelection: () => void;
}) {
  const isHotkey = card.triggerMode === "hotkey";
  const isRegion = card.triggerMode === "regionWatch";

  return (
    <div className="border-2 border-[var(--chalk)] bg-[var(--slate)]">
      <div className="flex items-center justify-between border-b-2 border-[var(--chalk)] bg-[var(--carbon)] px-3 py-2">
        <div className="flex items-center gap-2">
          <span className="font-mono text-xs font-black text-[var(--amber)]">A-{String(index + 1).padStart(2, "0")}</span>
          <Switch
            checked={card.enabled}
            onCheckedChange={(v) => onUpdate({ enabled: v })}
            aria-label={`卡片 ${index + 1} 启用开关`}
          />
          <span className="font-mono text-xs font-bold tracking-[0.12em] uppercase text-[var(--chalk)]">
            {card.enabled ? "启用" : "禁用"}
          </span>
        </div>
        <div className="flex items-center gap-1">
          <Button variant="ghost" size="sm" onClick={onTestPlay} title="测试播放" data-icon="inline-start">
            <RiPlayLine className="size-4" aria-hidden="true" />
            测试
          </Button>
          <Button variant="ghost" size="sm" onClick={onRemove} title="删除卡片" data-icon="inline-start">
            <RiDeleteBinLine className="size-4 text-[var(--alert-red)]" aria-hidden="true" />
          </Button>
        </div>
      </div>

      <div className="space-y-3 p-3">
        <FieldGroup>
          <Field>
            <FieldLabel>卡片名称</FieldLabel>
            <FieldContent>
              <Input
                value={card.name}
                onChange={(e) => onUpdate({ name: e.target.value })}
                placeholder="输入卡片名称..."
              />
            </FieldContent>
          </Field>

          <Field>
            <FieldLabel>触发模式</FieldLabel>
            <FieldContent>
              <Select value={card.triggerMode} onValueChange={(v) => onUpdate({ triggerMode: v as "hotkey" | "regionWatch" })}>
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="hotkey">快捷键触发</SelectItem>
                  <SelectItem value="regionWatch">区域监听+图像匹配</SelectItem>
                </SelectContent>
              </Select>
            </FieldContent>
          </Field>
        </FieldGroup>

        {isHotkey && (
          <FieldGroup>
            <Field>
              <FieldLabel>快捷键</FieldLabel>
              <FieldContent>
                <Input
                  value={card.hotkey}
                  onChange={(e) => onUpdate({ hotkey: e.target.value })}
                  placeholder="输入快捷键，如 Ctrl+F1..."
                />
              </FieldContent>
            </Field>
          </FieldGroup>
        )}

        {isRegion && (
          <FieldGroup>
            <Field>
              <FieldLabel>监听区域</FieldLabel>
              <FieldContent>
                <div className="flex items-center gap-2">
                  <Button variant="secondary" size="sm" onClick={onBeginRegionSelection} data-icon="inline-start">
                    <RiVolumeUpLine className="size-4" aria-hidden="true" />
                    {card.watchRegion ? "重新框选" : "框选区域"}
                  </Button>
                  {card.watchRegion && (
                    <Badge variant="outline" className="font-mono text-xs">
                      {card.watchRegion.x},{card.watchRegion.y} / {card.watchRegion.width}x{card.watchRegion.height}
                    </Badge>
                  )}
                </div>
              </FieldContent>
            </Field>
            <Field>
              <FieldLabel>参考图像路径</FieldLabel>
              <FieldContent>
                <Input
                  value={card.watchReferenceImagePath}
                  onChange={(e) => onUpdate({ watchReferenceImagePath: e.target.value })}
                  placeholder="参考图像文件路径..."
                />
              </FieldContent>
            </Field>
            <Field>
              <FieldLabel>匹配阈值</FieldLabel>
              <FieldContent>
                <Input
                  type="number"
                  min={0}
                  max={1}
                  step={0.01}
                  value={card.watchMatchThreshold}
                  onChange={(e) => onUpdate({ watchMatchThreshold: e.target.value })}
                />
              </FieldContent>
            </Field>
            <Field>
              <FieldLabel>轮询间隔 (ms)</FieldLabel>
              <FieldContent>
                <Input
                  type="number"
                  min={100}
                  max={10000}
                  step={100}
                  value={card.watchPollIntervalMs}
                  onChange={(e) => onUpdate({ watchPollIntervalMs: e.target.value })}
                />
              </FieldContent>
            </Field>
          </FieldGroup>
        )}

        <FieldGroup>
          <Field>
            <FieldLabel>音频文件路径</FieldLabel>
            <FieldContent>
              <Input
                value={card.audioFilePath}
                onChange={(e) => onUpdate({ audioFilePath: e.target.value })}
                placeholder="音频文件绝对路径..."
              />
            </FieldContent>
          </Field>
          <Field>
            <FieldLabel>音量</FieldLabel>
            <FieldContent>
              <Input
                type="number"
                min={0}
                max={1}
                step={0.1}
                value={card.volume}
                onChange={(e) => onUpdate({ volume: e.target.value })}
              />
            </FieldContent>
          </Field>
          <Field>
            <FieldLabel>冷却时间 (ms)</FieldLabel>
            <FieldContent>
              <Input
                type="number"
                min={0}
                max={60000}
                step={100}
                value={card.cooldownMs}
                onChange={(e) => onUpdate({ cooldownMs: e.target.value })}
              />
            </FieldContent>
          </Field>
        </FieldGroup>
      </div>
    </div>
  );
}

function cardToForm(card: AudioCard): AudioSettingsForm["cards"][number] {
  return {
    id: card.id,
    name: card.name,
    enabled: card.enabled,
    triggerMode: card.triggerMode,
    hotkey: card.hotkey ?? "",
    watchRegion: card.watchRegion,
    watchReferenceImagePath: card.watchReferenceImagePath ?? "",
    watchMatchThreshold: String(card.watchMatchThreshold),
    watchPollIntervalMs: String(card.watchPollIntervalMs),
    audioFilePath: card.audioFilePath,
    volume: String(card.volume),
    cooldownMs: String(card.cooldownMs),
  };
}

export function AudioRegionOverlay() {
  const params = useMemo(() => new URLSearchParams(window.location.search), []);
  const cardId = params.get("audio_card") ?? "";

  const [dragStart, setDragStart] = useState<Point | null>(null);
  const [dragCurrent, setDragCurrent] = useState<Point | null>(null);
  const [statusMessage, setStatusMessage] = useState("拖拽鼠标框选监听区域，Enter 确认，Esc 取消。");
  const [submitting, setSubmitting] = useState(false);

  const currentRect = useMemo(() => {
    if (!dragStart || !dragCurrent) return null;
    return getSelectionRect(dragStart, dragCurrent);
  }, [dragStart, dragCurrent]);

  const cancelSelection = useCallback(async () => {
    if (submitting) return;
    setSubmitting(true);
    setStatusMessage("正在取消...");
    try {
      await invoke("audio_overlay_cancel_selection", { cardId });
    } catch (error) {
      setStatusMessage(getErrorMessage(error));
      setSubmitting(false);
    }
  }, [cardId, submitting]);

  const submitSelection = useCallback(async () => {
    if (!currentRect || submitting) return;
    setSubmitting(true);
    setStatusMessage("正在提交...");
    try {
      await invoke("audio_overlay_submit_selection", { cardId, region: currentRect });
    } catch (error) {
      setStatusMessage(getErrorMessage(error));
      setSubmitting(false);
    }
  }, [cardId, currentRect, submitting]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        void cancelSelection();
      } else if (event.key === "Enter" && currentRect && !submitting) {
        event.preventDefault();
        void submitSelection();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [cancelSelection, currentRect, submitSelection, submitting]);

  const handleMouseDown = (event: React.MouseEvent<HTMLDivElement>) => {
    if (submitting || event.button !== 0) return;
    const point = { x: event.clientX, y: event.clientY };
    setDragStart(point);
    setDragCurrent(point);
    setStatusMessage("正在框选...");
  };

  const handleMouseMove = (event: React.MouseEvent<HTMLDivElement>) => {
    if (!dragStart || submitting) return;
    setDragCurrent({ x: event.clientX, y: event.clientY });
  };

  const handleMouseUp = () => {
    if (!dragStart || submitting) return;
    setStatusMessage("区域已框选，按 Enter 确认或 Esc 取消。");
  };

  return (
    <div
      className="fixed inset-0 cursor-crosshair select-none text-white"
      onContextMenu={(event) => {
        event.preventDefault();
        void cancelSelection();
      }}
      onMouseDown={handleMouseDown}
      onMouseMove={handleMouseMove}
      onMouseUp={handleMouseUp}
    >
      {currentRect && (
        <div
          className="pointer-events-none absolute border-2 border-[var(--amber)] bg-[var(--amber)]/16"
          style={{
            left: currentRect.x,
            top: currentRect.y,
            width: currentRect.width,
            height: currentRect.height,
          }}
        />
      )}

      <div className="pointer-events-none absolute left-6 top-6 max-w-md border-2 border-white/40 bg-[var(--carbon)]/88 px-4 py-4 text-[var(--chalk)] backdrop-blur-md">
        <h1 className="text-lg font-semibold text-[var(--chalk)]">音频区域选择</h1>
        <p className="mt-2 text-sm text-[var(--zinc)]">{statusMessage}</p>
        {currentRect && (
          <p className="mt-3 border border-[var(--seam)] bg-[var(--slate)]/80 px-3 py-2 font-mono text-xs text-[var(--zinc)]">
            {`X ${currentRect.x} · Y ${currentRect.y} · W ${currentRect.width} · H ${currentRect.height}`}
          </p>
        )}
      </div>

      <div className="absolute right-6 top-6 flex items-center gap-2 border-2 border-white/30 bg-[var(--carbon)]/80 px-3 py-3 backdrop-blur-md">
        <Button
          disabled={!currentRect || submitting}
          onClick={() => void submitSelection()}
          type="button"
          variant="secondary"
          data-icon="inline-start"
        >
          <RiCheckLine className="size-4" aria-hidden="true" />
          确认
        </Button>
        <Button
          disabled={submitting}
          onClick={() => void cancelSelection()}
          type="button"
          variant="secondary"
          data-icon="inline-start"
        >
          <RiCloseLine className="size-4" aria-hidden="true" />
          取消
        </Button>
      </div>
    </div>
  );
}

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  RiCheckboxCircleLine,
  RiCommandLine,
  RiHistoryLine,
  RiLayoutGridLine,
  RiPlayLine,
  RiRefreshLine,
  RiSettings3Line,
} from "@remixicon/react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { Field, FieldContent, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";

type MorsePageProps = {
  overlayMode?: boolean;
};

type RegionRect = {
  x: number;
  y: number;
  width: number;
  height: number;
};

type RegionTuple = [RegionRect | null, RegionRect | null, RegionRect | null];

type MorseSettings = {
  hotkey: string;
  regions: RegionTuple;
  binaryThreshold: number;
  autoInputDelay: number;
};

type MorseSettingsForm = {
  hotkey: string;
  regions: RegionTuple;
  binaryThreshold: string;
  autoInputDelay: string;
};

type MorseRegionDetail = {
  slot: number;
  thresholdMode: string;
  contourCount: number;
  morse: string | null;
  digit: string | null;
  error: string | null;
};

type MorseRunResult = {
  value: string | null;
  details: MorseRegionDetail[];
  triggeredBy: string;
  autoTyped: boolean;
  occurredAtMs: number;
  error: string | null;
};

type HistoryEntry = {
  id: number;
  result: string | null;
  success: boolean;
  triggeredBy: string;
  autoTyped: boolean;
  occurredAtMs: number;
  error: string | null;
};

type MorseBootstrap = {
  settings: MorseSettings;
  history: HistoryEntry[];
  latestRun: MorseRunResult | null;
  hotkeyError: string | null;
};

type RegionSelectionProgress = {
  currentSlot: number | null;
  regions: RegionTuple;
  completedSlots: number[];
};

type RegionSelectionOutcome = {
  kind: "selected" | "cancelled" | "closed";
  regions: RegionTuple;
};

type Point = {
  x: number;
  y: number;
};

const REGION_LABELS = ["位置 1", "位置 2", "位置 3"] as const;
const MIN_SELECTION_WIDTH = 10;
const MIN_SELECTION_HEIGHT = 5;
const EMPTY_REGIONS: RegionTuple = [null, null, null];
const HOTKEY_MODIFIER_KEYS = new Set(["Control", "Shift", "Alt", "Meta"]);
const AUTOSAVE_DELAY_MS = 400;

function getErrorMessage(error: unknown): string {
  if (typeof error === "string") {
    return error;
  }

  if (error instanceof Error) {
    return error.message;
  }

  return "发生未知错误。";
}

function settingsToForm(settings: MorseSettings): MorseSettingsForm {
  return {
    hotkey: settings.hotkey,
    regions: settings.regions,
    binaryThreshold: String(settings.binaryThreshold),
    autoInputDelay: String(settings.autoInputDelay),
  };
}

function parseSettingsForm(form: MorseSettingsForm): MorseSettings {
  const hotkey = form.hotkey.trim();
  if (!hotkey) {
    throw new Error("热键不能为空。");
  }

  const binaryThreshold = Number.parseInt(form.binaryThreshold, 10);
  if (!Number.isInteger(binaryThreshold) || binaryThreshold < 0 || binaryThreshold > 255) {
    throw new Error("二值化阈值必须是 0 到 255 之间的整数。");
  }

  const autoInputDelay = Number.parseInt(form.autoInputDelay, 10);
  if (!Number.isInteger(autoInputDelay) || autoInputDelay < 0) {
    throw new Error("输入延迟必须是大于等于 0 的整数毫秒值。");
  }

  return {
    hotkey,
    regions: form.regions,
    binaryThreshold,
    autoInputDelay,
  };
}

function formatTimestamp(timestamp: number | null | undefined): string {
  if (!timestamp) {
    return "--:--:--";
  }

  return new Intl.DateTimeFormat("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  }).format(timestamp);
}

function formatRegion(rect: RegionRect | null): string {
  if (!rect) {
    return "未设置";
  }

  return `X ${rect.x} · Y ${rect.y} · W ${rect.width} · H ${rect.height}`;
}

function getSelectionRect(start: Point, end: Point): RegionRect {
  const left = Math.min(start.x, end.x);
  const top = Math.min(start.y, end.y);
  const width = Math.abs(end.x - start.x);
  const height = Math.abs(end.y - start.y);

  return {
    x: Math.round(left),
    y: Math.round(top),
    width: Math.round(width),
    height: Math.round(height),
  };
}

function normalizeRunDetails(latestRun: MorseRunResult | null): MorseRegionDetail[] {
  return REGION_LABELS.map((_, slot) => {
    const detail = latestRun?.details.find((item) => item.slot === slot);

    return (
      detail ?? {
        slot,
        thresholdMode: "--",
        contourCount: 0,
        morse: null,
        digit: null,
        error: null,
      }
    );
  });
}

function parseOverlaySlots(): number[] {
  const params = new URLSearchParams(window.location.search);
  const slotsParam = params.get("slots");
  const singleSlotParam = params.get("slot");

  const rawValues = slotsParam
    ? slotsParam.split(",")
    : singleSlotParam
      ? [singleSlotParam]
      : [];

  const parsed = rawValues
    .map((value) => Number.parseInt(value, 10))
    .filter((value, index, values) => Number.isInteger(value) && value >= 0 && value < REGION_LABELS.length && values.indexOf(value) === index);

  return parsed.length > 0 ? parsed : [0, 1, 2];
}

function normalizeHotkeyPrimaryKey(key: string): string | null {
  if (HOTKEY_MODIFIER_KEYS.has(key)) {
    return null;
  }

  if (/^F\d{1,2}$/i.test(key)) {
    return key.toUpperCase();
  }

  if (/^[a-z]$/i.test(key)) {
    return key.toUpperCase();
  }

  if (/^[0-9]$/.test(key)) {
    return key;
  }

  const specialKeyMap: Record<string, string> = {
    " ": "Space",
    Enter: "Enter",
    Tab: "Tab",
    Escape: "Esc",
    ArrowUp: "Up",
    ArrowDown: "Down",
    ArrowLeft: "Left",
    ArrowRight: "Right",
    Home: "Home",
    End: "End",
    PageUp: "PageUp",
    PageDown: "PageDown",
    Insert: "Insert",
    Delete: "Delete",
    Backspace: "Backspace",
  };

  return specialKeyMap[key] ?? null;
}

function formatRecordedHotkey(event: React.KeyboardEvent<HTMLButtonElement>): string | null {
  const primaryKey = normalizeHotkeyPrimaryKey(event.key);
  if (!primaryKey) {
    return null;
  }

  const segments: string[] = [];
  if (event.ctrlKey) {
    segments.push("Ctrl");
  }
  if (event.altKey) {
    segments.push("Alt");
  }
  if (event.shiftKey) {
    segments.push("Shift");
  }
  if (event.metaKey) {
    segments.push("Super");
  }

  segments.push(primaryKey);
  return segments.join("+");
}

function RegionSelectionOverlay({ slots }: { slots: number[] }) {
  const [dragStart, setDragStart] = useState<Point | null>(null);
  const [dragCurrent, setDragCurrent] = useState<Point | null>(null);
  const [regions, setRegions] = useState<RegionTuple>(EMPTY_REGIONS);
  const [completedSlots, setCompletedSlots] = useState<number[]>([]);
  const [currentSlot, setCurrentSlot] = useState<number | null>(slots[0] ?? null);
  const [statusMessage, setStatusMessage] = useState("拖拽框选当前区域，Esc 或右键取消。") ;
  const [submitting, setSubmitting] = useState(false);

  const currentRect = useMemo(() => {
    if (!dragStart || !dragCurrent) {
      return null;
    }

    return getSelectionRect(dragStart, dragCurrent);
  }, [dragCurrent, dragStart]);

  const activeStep = currentSlot === null ? slots.length : completedSlots.length + 1;

  const cancelSelection = useCallback(async () => {
    if (currentSlot === null || submitting) {
      return;
    }

    setSubmitting(true);
    setStatusMessage("正在取消区域选择...");

    try {
      await invoke("morse_overlay_cancel_selection", { slot: currentSlot });
    } catch (error) {
      setStatusMessage(getErrorMessage(error));
      setSubmitting(false);
    }
  }, [currentSlot, submitting]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        void cancelSelection();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [cancelSelection]);

  const handleMouseDown = (event: React.MouseEvent<HTMLDivElement>) => {
    if (currentSlot === null || submitting || event.button !== 0) {
      return;
    }

    const point = { x: event.clientX, y: event.clientY };
    setDragStart(point);
    setDragCurrent(point);
    setStatusMessage(`正在框选 ${REGION_LABELS[currentSlot]}...`);
  };

  const handleMouseMove = (event: React.MouseEvent<HTMLDivElement>) => {
    if (!dragStart || submitting) {
      return;
    }

    setDragCurrent({ x: event.clientX, y: event.clientY });
  };

  const handleMouseUp = async (event: React.MouseEvent<HTMLDivElement>) => {
    if (!dragStart || currentSlot === null || submitting) {
      return;
    }

    const rect = getSelectionRect(dragStart, {
      x: event.clientX,
      y: event.clientY,
    });

    setDragStart(null);
    setDragCurrent(null);

    if (rect.width <= MIN_SELECTION_WIDTH || rect.height <= MIN_SELECTION_HEIGHT) {
      setStatusMessage("区域太小，请重新框选。");
      return;
    }

    setSubmitting(true);

    try {
      const progress = await invoke<RegionSelectionProgress>("morse_overlay_submit_selection", {
        slot: currentSlot,
        rect,
      });
      setRegions(progress.regions);
      setCompletedSlots(progress.completedSlots);
      setCurrentSlot(progress.currentSlot);

      if (progress.currentSlot === null) {
        setStatusMessage("3 个区域已保存，正在返回主界面...");
        return;
      }

      setStatusMessage(`${REGION_LABELS[currentSlot]} 已保存，请继续框选 ${REGION_LABELS[progress.currentSlot]}。`);
      setSubmitting(false);
    } catch (error) {
      setStatusMessage(getErrorMessage(error));
      setSubmitting(false);
    }
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
      {regions.map((region, index) => {
        if (!region) {
          return null;
        }

        const isCurrent = currentSlot === index;
        return (
          <div
            key={index}
            className={isCurrent ? "pointer-events-none absolute rounded-md border-2 border-primary/80 bg-primary/10" : "pointer-events-none absolute rounded-md border border-white/75 bg-white/8"}
            style={{
              left: region.x,
              top: region.y,
              width: region.width,
              height: region.height,
            }}
          />
        );
      })}

      {currentRect ? (
        <div
          className="pointer-events-none absolute rounded-md border-2 border-primary bg-primary/12"
          style={{
            left: currentRect.x,
            top: currentRect.y,
            width: currentRect.width,
            height: currentRect.height,
          }}
        />
      ) : null}

      <div className="pointer-events-none absolute left-6 top-6 max-w-md rounded-xl border border-border/80 bg-background/86 px-4 py-3 text-foreground shadow-lg backdrop-blur-sm">
        <div className="flex items-center gap-2">
          <Badge variant="outline">{`第 ${activeStep} / ${slots.length} 步`}</Badge>
          {currentSlot !== null ? <Badge variant="secondary">{REGION_LABELS[currentSlot]}</Badge> : null}
        </div>
        <h1 className="mt-3 text-lg font-semibold text-foreground">
          {currentSlot === null ? "区域已完成" : `选择 ${REGION_LABELS[currentSlot]}`}
        </h1>
        <p className="mt-2 text-sm text-muted-foreground">{statusMessage}</p>
        {currentRect ? (
          <p className="mt-3 font-mono text-xs text-muted-foreground">
            {`X ${currentRect.x} · Y ${currentRect.y} · W ${currentRect.width} · H ${currentRect.height}`}
          </p>
        ) : null}
      </div>

      <div className="absolute right-6 top-6 flex items-center gap-2">
        {completedSlots.map((slot) => (
          <Badge key={slot} variant="secondary">
            {REGION_LABELS[slot]}
          </Badge>
        ))}
        <Button disabled={submitting || currentSlot === null} onClick={() => void cancelSelection()} type="button" variant="secondary">
          取消
        </Button>
      </div>
    </div>
  );
}

export function MorsePage({ overlayMode = false }: MorsePageProps) {
  const overlaySlots = useMemo(() => (overlayMode ? parseOverlaySlots() : []), [overlayMode]);
  const [bootstrap, setBootstrap] = useState<MorseBootstrap | null>(null);
  const [form, setForm] = useState<MorseSettingsForm | null>(null);
  const formRef = useRef<MorseSettingsForm | null>(null);
  const hotkeyButtonRef = useRef<HTMLButtonElement | null>(null);
  const hotkeyDraftRef = useRef<string>("");
  const [loading, setLoading] = useState(!overlayMode);
  const [saving, setSaving] = useState(false);
  const [running, setRunning] = useState(false);
  const [selectingSlot, setSelectingSlot] = useState<number | null>(null);
  const [isRecordingHotkey, setIsRecordingHotkey] = useState(false);
  const [statusMessage, setStatusMessage] = useState("正在加载摩斯工具...");
  const [pageError, setPageError] = useState<string | null>(null);
  const saveTimeoutRef = useRef<number | null>(null);
  const autosaveVersionRef = useRef(0);

  useEffect(() => {
    formRef.current = form;
  }, [form]);

  useEffect(() => {
    if (isRecordingHotkey) {
      hotkeyButtonRef.current?.focus();
    }
  }, [isRecordingHotkey]);

  useEffect(() => {
    return () => {
      if (saveTimeoutRef.current !== null) {
        window.clearTimeout(saveTimeoutRef.current);
      }
    };
  }, []);

  useEffect(() => {
    if (overlayMode) {
      return;
    }

    let disposed = false;

    void invoke("morse_set_hotkey_recording", { recording: isRecordingHotkey }).catch((error) => {
      if (!disposed) {
        const message = getErrorMessage(error);
        setPageError(message);
        setStatusMessage(message);
      }
    });

    return () => {
      disposed = true;
    };
  }, [isRecordingHotkey, overlayMode]);

  useEffect(() => {
    if (!overlayMode) {
      return;
    }

    document.body.dataset.overlayMode = "true";
    return () => {
      delete document.body.dataset.overlayMode;
    };
  }, [overlayMode]);

  const syncBootstrap = useCallback(async (syncMode: "full" | "regions" | "none" = "none") => {
    const next = await invoke<MorseBootstrap>("morse_get_bootstrap");
    setBootstrap(next);
    setPageError(null);

    if (syncMode === "full" || formRef.current === null) {
      setForm(settingsToForm(next.settings));
      return next;
    }

    if (syncMode === "regions") {
      setForm((current) =>
        current
          ? {
              ...current,
              regions: next.settings.regions,
            }
          : settingsToForm(next.settings),
      );
    }

    return next;
  }, []);

  useEffect(() => {
    if (overlayMode) {
      return;
    }

    let disposed = false;

    const load = async () => {
      try {
        setLoading(true);
        await syncBootstrap("full");
        if (!disposed) {
          setStatusMessage("就绪，可开始框选区域或执行识别。");
        }
      } catch (error) {
        if (!disposed) {
          setPageError(getErrorMessage(error));
          setStatusMessage("加载失败，请确认桌面端 Tauri 进程已正常运行。");
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
  }, [overlayMode, syncBootstrap]);

  useEffect(() => {
    if (overlayMode) {
      return;
    }

    let isDisposed = false;
    let unlistenRunFinished: (() => void) | undefined;
    let unlistenSelectionProgress: (() => void) | undefined;
    let unlistenHotkeyError: (() => void) | undefined;

    void listen<MorseRunResult>("morse://run-finished", async (event) => {
      if (isDisposed) {
        return;
      }

      const result = event.payload;
      setBootstrap((current) => (current ? { ...current, latestRun: result } : current));
      setStatusMessage(result.error ? `识别失败：${result.error}` : `识别完成：${result.value ?? "无结果"}`);

      try {
        await syncBootstrap("none");
      } catch (error) {
        if (!isDisposed) {
          setPageError(getErrorMessage(error));
        }
      }
    }).then((dispose) => {
      unlistenRunFinished = dispose;
    });

    void listen<RegionSelectionProgress>("morse://selection-progress", (event) => {
      if (isDisposed) {
        return;
      }

      const progress = event.payload;
      setBootstrap((current) =>
        current
          ? {
              ...current,
              settings: {
                ...current.settings,
                regions: progress.regions,
              },
            }
          : current,
      );
      setForm((current) =>
        current
          ? {
              ...current,
              regions: progress.regions,
            }
          : current,
      );
    }).then((dispose) => {
      unlistenSelectionProgress = dispose;
    });

    void listen<string>("morse://hotkey-error", (event) => {
      if (isDisposed) {
        return;
      }
      setBootstrap((current) =>
        current ? { ...current, hotkeyError: event.payload } : current,
      );
    }).then((dispose) => {
      unlistenHotkeyError = dispose;
    });

    return () => {
      isDisposed = true;
      unlistenRunFinished?.();
      unlistenSelectionProgress?.();
      unlistenHotkeyError?.();
    };
  }, [overlayMode, syncBootstrap]);

  const latestRun = bootstrap?.latestRun ?? null;
  const history = bootstrap?.history ?? [];
  const savedSettings = bootstrap?.settings ?? null;
  const configuredCount = savedSettings?.regions.filter(Boolean).length ?? 0;
  const runDetails = normalizeRunDetails(latestRun);
  const canRun = configuredCount === REGION_LABELS.length;
  const isBusy = loading || saving || running || selectingSlot !== null;

  const isDirty = useMemo(() => {
    if (!bootstrap || !form) {
      return false;
    }

    return JSON.stringify(settingsToForm(bootstrap.settings)) !== JSON.stringify(form);
  }, [bootstrap, form]);

  if (overlayMode) {
    return <RegionSelectionOverlay slots={overlaySlots} />;
  }

  const updateForm = <K extends keyof MorseSettingsForm>(key: K, value: MorseSettingsForm[K]) => {
    setForm((current) => (current ? { ...current, [key]: value } : current));
  };

  const saveSettings = useCallback(
    async (settingsValue: MorseSettings, pendingVersion?: number) => {
      try {
        setSaving(true);
        setStatusMessage("正在保存设置...");
        const next = await invoke<MorseBootstrap>("morse_save_settings", { settingsValue });

        if (typeof pendingVersion === "number" && pendingVersion !== autosaveVersionRef.current) {
          return;
        }

        setBootstrap(next);
        setForm(settingsToForm(next.settings));
        setPageError(null);
        setStatusMessage("设置已保存。新的热键与识别参数已生效。");
      } catch (error) {
        const message = getErrorMessage(error);
        setPageError(message);
        setStatusMessage(message);
      } finally {
        setSaving(false);
      }
    },
    [],
  );

  useEffect(() => {
    if (overlayMode || loading || !bootstrap || !form || isRecordingHotkey || selectingSlot !== null) {
      return;
    }

    if (!isDirty) {
      return;
    }

    const nextVersion = autosaveVersionRef.current + 1;
    autosaveVersionRef.current = nextVersion;
    const formSnapshot = form;

    saveTimeoutRef.current = window.setTimeout(() => {
      try {
        const settingsValue = parseSettingsForm(formSnapshot);
        void saveSettings(settingsValue, nextVersion);
      } catch (error) {
        if (nextVersion !== autosaveVersionRef.current) {
          return;
        }

        const message = getErrorMessage(error);
        setPageError(message);
        setStatusMessage(`保存失败：${message}`);
      }
    }, AUTOSAVE_DELAY_MS);

    return () => {
      if (saveTimeoutRef.current !== null) {
        window.clearTimeout(saveTimeoutRef.current);
        saveTimeoutRef.current = null;
      }
    };
  }, [bootstrap, form, isDirty, isRecordingHotkey, loading, overlayMode, saveSettings, selectingSlot]);

  const performSelectionSession = async (slots: number[]) => {
    if (slots.length === 0) {
      return false;
    }

    setSelectingSlot(slots.length === 1 ? slots[0] : -1);
    setStatusMessage(
      slots.length === REGION_LABELS.length
        ? "请在悬浮层中依次完成 3 个区域框选。"
        : `请在悬浮层中框选 ${REGION_LABELS[slots[0]]}。`,
    );

    try {
      const outcome = await invoke<RegionSelectionOutcome>("morse_begin_region_selection", {
        slots,
      });
      await syncBootstrap("regions");

      if (outcome.kind === "selected") {
        setStatusMessage(
          slots.length === REGION_LABELS.length ? "3 个区域已全部更新。" : `${REGION_LABELS[slots[0]]} 已更新。`,
        );
        return true;
      }

      if (outcome.kind === "cancelled") {
        setStatusMessage("区域选择已取消。");
        return false;
      }

      setStatusMessage("区域选择窗口已关闭。");
      return false;
    } catch (error) {
      const message = getErrorMessage(error);
      setPageError(message);
      setStatusMessage(message);
      return false;
    } finally {
      setSelectingSlot(null);
    }
  };

  const handleRunRecognition = async (autoType: boolean) => {
    try {
      setRunning(true);
      setStatusMessage(autoType ? "正在识别并准备自动输入..." : "正在执行识别...");
      const result = await invoke<MorseRunResult>("morse_run_recognition", { autoType });
      setBootstrap((current) => (current ? { ...current, latestRun: result } : current));
      setPageError(result.error);
      setStatusMessage(result.error ? `识别失败：${result.error}` : `识别完成：${result.value ?? "无结果"}`);
    } catch (error) {
      const message = getErrorMessage(error);
      setPageError(message);
      setStatusMessage(message);
    } finally {
      setRunning(false);
    }
  };

  const beginHotkeyRecording = () => {
    if (!form) {
      return;
    }

    hotkeyDraftRef.current = form.hotkey;
    setIsRecordingHotkey(true);
    setStatusMessage("正在录制热键，按下组合键后会自动更新。");
  };

  const handleHotkeyRecorderKeyDown = (event: React.KeyboardEvent<HTMLButtonElement>) => {
    if (!form || !isRecordingHotkey) {
      return;
    }

    if (event.key === "Tab") {
      return;
    }

    event.preventDefault();
    event.stopPropagation();

    if (event.key === "Escape") {
      updateForm("hotkey", hotkeyDraftRef.current);
      setIsRecordingHotkey(false);
      setStatusMessage("已取消热键录制。");
      return;
    }

    const nextHotkey = formatRecordedHotkey(event);
    if (!nextHotkey) {
      setStatusMessage("请按下一个可识别的主键，支持字母、数字、功能键与常用导航键。");
      return;
    }

    updateForm("hotkey", nextHotkey);
    setIsRecordingHotkey(false);
    setPageError(null);
    setStatusMessage(`新的热键已录制：${nextHotkey}`);
  };

  const handleHotkeyRecorderBlur = () => {
    if (!isRecordingHotkey) {
      return;
    }

    updateForm("hotkey", hotkeyDraftRef.current);
    setIsRecordingHotkey(false);
    setStatusMessage("热键录制已结束。");
  };

  return (
    <div className="flex flex-1 flex-col gap-3">
      <div className="desktop-toolbar">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2">
            <h1 className="text-base font-semibold tracking-tight">摩斯密码解析</h1>
            <Badge variant={canRun ? "default" : "secondary"}>{canRun ? "可执行" : "等待区域配置"}</Badge>
            {saving ? <Badge variant="outline">保存中</Badge> : isDirty ? <Badge variant="outline">待保存</Badge> : <Badge variant="outline">已保存</Badge>}
          </div>
          <p className="mt-1 text-xs text-muted-foreground">{statusMessage}</p>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <Badge variant="outline">已配置 {configuredCount}/3</Badge>
          <Badge variant="outline">最近结果 {formatTimestamp(latestRun?.occurredAtMs)}</Badge>
        </div>
      </div>

      <Card size="sm">
        <CardHeader>
          <div className="flex items-center gap-2">
            <RiLayoutGridLine className="text-muted-foreground" />
            <CardTitle>采样区域</CardTitle>
          </div>
        </CardHeader>
        <CardContent className="grid gap-3 md:grid-cols-3">
          {REGION_LABELS.map((label, index) => {
            const region = form?.regions[index] ?? null;
            const isConfigured = Boolean(region);
            const isSelecting = selectingSlot === index;

            return (
              <div key={label} className="desktop-subpanel p-3">
                <div className="flex items-start justify-between gap-2">
                  <div className="min-w-0">
                    <p className="text-sm font-medium text-foreground">{label}</p>
                    <p className="mt-1 text-xs text-muted-foreground">{isConfigured ? formatRegion(region) : "尚未设置"}</p>
                  </div>
                  <Badge variant={isConfigured ? "default" : "outline"}>{isConfigured ? "已配置" : "待配置"}</Badge>
                </div>

                <div className="mt-3 flex aspect-[16/9] items-end rounded-lg border border-dashed border-border/70 bg-background px-3 py-3">
                  <div className="w-full rounded-md border border-border/70 bg-muted/30 px-3 py-2">
                    <p className="text-xs text-muted-foreground">{isConfigured ? "当前区域" : "等待框选"}</p>
                    <p className="mt-2 font-mono text-[0.75rem] text-foreground/80">{formatRegion(region)}</p>
                  </div>
                </div>

                <div className="mt-3 flex gap-2">
                  <Button
                    className="flex-1"
                    disabled={isBusy}
                    onClick={() => void performSelectionSession([index])}
                    type="button"
                    variant={isConfigured ? "outline" : "default"}
                  >
                    {isSelecting ? "正在框选..." : isConfigured ? "重新选择" : "选择区域"}
                  </Button>
                </div>
              </div>
            );
          })}
        </CardContent>
      </Card>

      <Card size="sm">
        <CardContent className="flex flex-wrap items-center gap-2 py-4">
          <Button disabled={isBusy} onClick={() => void performSelectionSession([0, 1, 2])} type="button">
            <RiRefreshLine data-icon="inline-start" />
            一次选择 3 个区域
          </Button>
          <Button disabled={isBusy || !canRun} onClick={() => void handleRunRecognition(true)} type="button">
            <RiPlayLine data-icon="inline-start" />
            开始解析
          </Button>
          <Button disabled={isBusy || !canRun} onClick={() => void handleRunRecognition(false)} type="button" variant="outline">
            <RiCommandLine data-icon="inline-start" />
            仅识别不输入
          </Button>
        </CardContent>
      </Card>

      <div className="grid gap-3 xl:grid-cols-[minmax(0,1.2fr)_320px]">
        <Card size="sm">
          <CardHeader>
            <div className="flex items-center gap-2">
              <RiCheckboxCircleLine className="text-muted-foreground" />
              <CardTitle>解析结果</CardTitle>
            </div>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="rounded-xl border border-border/70 bg-background px-4 py-4">
              <div className="flex flex-wrap items-center gap-2">
                <Badge variant={latestRun?.error ? "outline" : "default"}>
                  {latestRun?.error ? "失败" : latestRun?.value ? "成功" : "等待执行"}
                </Badge>
                {latestRun ? <Badge variant="outline">来源 {latestRun.triggeredBy}</Badge> : null}
                {latestRun?.autoTyped ? <Badge variant="outline">已自动输入</Badge> : null}
              </div>
              <p className="mt-4 font-mono text-3xl font-semibold tracking-[0.35em] text-foreground/90">
                {latestRun?.value ?? "---"}
              </p>
              <p className="mt-3 text-xs text-muted-foreground">{latestRun?.error ?? "执行识别后会在这里显示结果。"}</p>
            </div>

            <div className="grid gap-3 md:grid-cols-3">
              {runDetails.map((detail) => (
                <div key={detail.slot} className="desktop-subpanel p-3">
                  <div className="flex items-center justify-between gap-2">
                    <p className="text-xs font-medium text-foreground">{REGION_LABELS[detail.slot] ?? `位置 ${detail.slot + 1}`}</p>
                    <Badge variant={detail.error ? "outline" : detail.digit ? "default" : "secondary"}>
                      {detail.error ? "失败" : detail.digit ? detail.digit : "待机"}
                    </Badge>
                  </div>
                  <div className="mt-3 grid gap-2 text-xs text-muted-foreground">
                    <div className="desktop-subpanel bg-background/80 px-3 py-2">
                      <p className="text-xs text-muted-foreground">Morse</p>
                      <p className="mt-1 font-mono text-foreground/80">{detail.morse ?? "--"}</p>
                    </div>
                    <div className="desktop-subpanel bg-background/80 px-3 py-2">
                      <p className="text-xs text-muted-foreground">Threshold</p>
                      <p className="mt-1 text-foreground/80">{detail.thresholdMode}</p>
                    </div>
                    <div className="desktop-subpanel bg-background/80 px-3 py-2">
                      <p className="text-xs text-muted-foreground">Contours</p>
                      <p className="mt-1 text-foreground/80">{detail.contourCount}</p>
                    </div>
                  </div>
                  {detail.error ? <p className="mt-3 text-xs/relaxed text-destructive">{detail.error}</p> : null}
                </div>
              ))}
            </div>
          </CardContent>
        </Card>

        <Card size="sm">
          <CardHeader>
            <div className="flex items-center gap-2">
              <RiSettings3Line className="text-muted-foreground" />
              <CardTitle>设置</CardTitle>
            </div>
          </CardHeader>
          <CardContent>
            {form ? (
              <FieldGroup className="grid gap-3">
                <Field>
                  <FieldLabel htmlFor="hotkey-recorder">热键</FieldLabel>
                  <FieldContent>
                    <Button
                      ref={hotkeyButtonRef}
                      className="w-full justify-between font-mono"
                      id="hotkey-recorder"
                      onBlur={handleHotkeyRecorderBlur}
                      onClick={beginHotkeyRecording}
                      onKeyDown={handleHotkeyRecorderKeyDown}
                      type="button"
                      variant="outline"
                    >
                      <span>{isRecordingHotkey ? "正在录制，按下快捷键..." : form.hotkey || "点击录制热键"}</span>
                      <span className="text-xs text-muted-foreground">{isRecordingHotkey ? "Esc 取消" : "点击编辑"}</span>
                    </Button>
                    {bootstrap?.hotkeyError && (
                      <p className="text-xs text-destructive mt-1">{bootstrap.hotkeyError}</p>
                    )}
                  </FieldContent>
                </Field>

                <Field>
                  <FieldLabel htmlFor="binary-threshold">二值化阈值</FieldLabel>
                  <FieldContent>
                    <Input
                      id="binary-threshold"
                      inputMode="numeric"
                      max="255"
                      min="0"
                      onChange={(event) => updateForm("binaryThreshold", event.currentTarget.value)}
                      value={form.binaryThreshold}
                    />
                  </FieldContent>
                </Field>

                <Field>
                  <FieldLabel htmlFor="auto-input-delay">自动输入延迟（毫秒）</FieldLabel>
                  <FieldContent>
                    <Input
                      id="auto-input-delay"
                      inputMode="numeric"
                      min="0"
                      onChange={(event) => updateForm("autoInputDelay", event.currentTarget.value)}
                      value={form.autoInputDelay}
                    />
                  </FieldContent>
                </Field>
              </FieldGroup>
            ) : (
              <div className="text-xs text-muted-foreground">正在加载设置...</div>
            )}
          </CardContent>
        </Card>
      </div>

      <Card size="sm">
        <CardHeader>
          <div className="flex items-center gap-2">
            <RiHistoryLine className="text-muted-foreground" />
            <CardTitle>历史记录</CardTitle>
          </div>
        </CardHeader>
        <CardContent>
          <ScrollArea className="h-72">
            <div className="flex flex-col gap-3 pe-4">
              {history.length === 0 ? (
                <Empty className="border-border bg-muted/20">
                  <EmptyHeader>
                    <EmptyMedia variant="icon">
                      <RiHistoryLine />
                    </EmptyMedia>
                    <EmptyTitle>暂无记录</EmptyTitle>
                    <EmptyDescription>执行一次识别后会显示在这里。</EmptyDescription>
                  </EmptyHeader>
                </Empty>
              ) : (
                history.map((entry) => (
                  <div key={entry.id} className="desktop-subpanel p-3">
                    <div className="flex flex-wrap items-center justify-between gap-2">
                      <div className="flex flex-wrap items-center gap-2">
                        <p className="text-xs font-medium text-foreground">
                          {entry.result ? `识别结果 ${entry.result}` : "识别失败"}
                        </p>
                        <Badge variant={entry.success ? "default" : "outline"}>
                          {entry.success ? "成功" : "失败"}
                        </Badge>
                        <Badge variant="outline">{entry.triggeredBy}</Badge>
                        {entry.autoTyped ? <Badge variant="outline">已自动输入</Badge> : null}
                      </div>
                      <span className="text-xs text-muted-foreground">{formatTimestamp(entry.occurredAtMs)}</span>
                    </div>
                    <p className="mt-2 text-xs/relaxed text-muted-foreground">{entry.error ?? "识别流程已完成。"}</p>
                  </div>
                ))
              )}
            </div>
          </ScrollArea>
        </CardContent>
      </Card>

      <div className="rounded-lg border border-border/70 bg-card px-4 py-2 text-xs text-muted-foreground">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <span>{pageError ?? statusMessage}</span>
          <span>{loading ? "加载中" : isBusy ? "处理中" : "空闲"}</span>
        </div>
      </div>
    </div>
  );
}

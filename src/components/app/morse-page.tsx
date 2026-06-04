import { startTransition, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import { useNativeShell } from "@/hooks/use-native-shell";
import { useSyncedRef } from "@/hooks/use-synced-ref";
import { useTimeoutCleanup } from "@/hooks/use-timeout-cleanup";

import { Badge } from "@/components/ui/badge";
import { AppPage, PageHero, SaveStateBadge, SignalTile } from "@/components/app/app-ui";
import { RegionSelectionOverlay } from "@/components/app/morse-overlay";
import { HistoryPanel, ResultPanel, SelectionPanel, WorkbenchControlPanel } from "@/components/app/morse-panels";
import {
  AUTOSAVE_DELAY_MS,
  REGION_LABELS,
  type MorseBootstrap,
  type MorsePageProps,
  type MorseRunResult,
  type MorseSettings,
  type MorseSettingsForm,
  type RegionSelectionOutcome,
  type RegionSelectionProgress,
  type VerificationStatus,
} from "@/components/app/morse-types";
import {
  formatRecordedHotkey,
  formatTimestamp,
  getErrorMessage,
  normalizeRunDetails,
  parseOverlaySlots,
  parseSettingsForm,
  settingsToForm,
} from "@/components/app/morse-utils";

export function MorsePage({ overlayMode = false }: MorsePageProps) {
  const overlaySlots = useMemo(() => (overlayMode ? parseOverlaySlots() : []), [overlayMode]);
  const isNativeShell = useNativeShell();
  const [bootstrap, setBootstrap] = useState<MorseBootstrap | null>(null);
  const [form, setForm] = useState<MorseSettingsForm | null>(null);
  const formRef = useSyncedRef(form);
  const hotkeyButtonRef = useRef<HTMLButtonElement | null>(null);
  const hotkeyDraftRef = useRef<string>("");
  const [loading, setLoading] = useState(!overlayMode);
  const [saving, setSaving] = useState(false);
  const [running, setRunning] = useState(false);
  const [selectingSlot, setSelectingSlot] = useState<number | null>(null);
  const [isRecordingHotkey, setIsRecordingHotkey] = useState(false);
  const [_statusMessage, setStatusMessage] = useState("正在加载摩斯工具...");
  const [_pageError, setPageError] = useState<string | null>(null);
  const [verificationValue, setVerificationValue] = useState("");
  const [verificationStatus, setVerificationStatus] = useState<VerificationStatus>("idle");
  const [verificationMessage, setVerificationMessage] = useState("点击验证输入框即可执行一次仅识别流程，结果会直接回填到这里。");
  const saveTimeoutRef = useTimeoutCleanup();
  const autosaveVersionRef = useRef(0);

  useEffect(() => {
    if (isRecordingHotkey) {
      hotkeyButtonRef.current?.focus();
    }
  }, [isRecordingHotkey]);

  useEffect(() => {
    if (overlayMode || !isNativeShell) {
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
  }, [isNativeShell, isRecordingHotkey, overlayMode]);

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

    startTransition(() => {
      setBootstrap(next);
      setPageError(null);

      if (syncMode === "full" || formRef.current === null) {
        setForm(settingsToForm(next.settings));
        return;
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
    });

    return next;
  }, []);

  useEffect(() => {
    if (overlayMode) {
      return;
    }

    if (!isNativeShell) {
      setLoading(false);
      setPageError(null);
      setStatusMessage("浏览器预览模式：当前仅验证布局与滚动，原生命令请在桌面端运行。");
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
  }, [isNativeShell, overlayMode, syncBootstrap]);

  useEffect(() => {
    if (overlayMode || !isNativeShell) {
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
      startTransition(() => {
        setBootstrap((current) => (current ? { ...current, latestRun: result } : current));
        setStatusMessage(result.error ? `识别失败：${result.error}` : `识别完成：${result.value ?? "无结果"}`);
      });

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
      startTransition(() => {
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
      });
    }).then((dispose) => {
      unlistenSelectionProgress = dispose;
    });

    void listen<string>("morse://hotkey-error", (event) => {
      if (isDisposed) {
        return;
      }
      startTransition(() => {
        setBootstrap((current) => (current ? { ...current, hotkeyError: event.payload } : current));
      });
    }).then((dispose) => {
      unlistenHotkeyError = dispose;
    });

    return () => {
      isDisposed = true;
      unlistenRunFinished?.();
      unlistenSelectionProgress?.();
      unlistenHotkeyError?.();
    };
  }, [isNativeShell, overlayMode, syncBootstrap]);

  const latestRun = bootstrap?.latestRun ?? null;
  const history = bootstrap?.history ?? [];
  const savedSettings = bootstrap?.settings ?? null;
  const configuredCount = savedSettings?.regions.filter(Boolean).length ?? 0;
  const runDetails = normalizeRunDetails(latestRun);
  const latestResultValue = latestRun?.value ?? null;
  const latestResultTime = latestRun?.occurredAtMs ?? null;
  const hasLatestResult = Boolean(latestRun);
  const canRun = configuredCount === REGION_LABELS.length;
  const isBusy = loading || saving || running || selectingSlot !== null;
  const stepOneComplete = configuredCount === REGION_LABELS.length;
  const stepTwoActive = stepOneComplete && !hasLatestResult;
  const stepThreeActive = hasLatestResult;

  const isDirty = useMemo(() => {
    if (!bootstrap || !form) {
      return false;
    }

    return JSON.stringify(settingsToForm(bootstrap.settings)) !== JSON.stringify(form);
  }, [bootstrap, form]);

  const updateForm = useCallback(<K extends keyof MorseSettingsForm>(key: K, value: MorseSettingsForm[K]) => {
    setForm((current) => (current ? { ...current, [key]: value } : current));
  }, []);

  const saveSettings = useCallback(async (settingsValue: MorseSettings, pendingVersion?: number) => {
    try {
      setSaving(true);
      setStatusMessage("正在保存设置...");
      const next = await invoke<MorseBootstrap>("morse_save_settings", { settingsValue });

      if (typeof pendingVersion === "number" && pendingVersion !== autosaveVersionRef.current) {
        return;
      }

      startTransition(() => {
        setBootstrap(next);
        setForm(settingsToForm(next.settings));
        setPageError(null);
        setStatusMessage("设置已保存。新的热键与识别参数已生效。");
      });
    } catch (error) {
      const message = getErrorMessage(error);
      setPageError(message);
      setStatusMessage(message);
    } finally {
      setSaving(false);
    }
  }, []);

  useEffect(() => {
    if (overlayMode || !isNativeShell || loading || !bootstrap || !form || isRecordingHotkey || selectingSlot !== null) {
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
  }, [bootstrap, form, isDirty, isNativeShell, isRecordingHotkey, loading, overlayMode, saveSettings, selectingSlot]);

  const performSelectionSession = useCallback(async (slots: number[]) => {
    if (slots.length === 0) {
      return false;
    }

    if (!isNativeShell) {
      setStatusMessage("浏览器预览模式下不可执行区域框选，请在桌面端使用。");
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
        setStatusMessage(slots.length === REGION_LABELS.length ? "3 个区域已全部更新。" : `${REGION_LABELS[slots[0]]} 已更新。`);
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
  }, [isNativeShell, syncBootstrap]);

  const handleUpdateClickRegionDelay = useCallback((index: number, delayMs: string) => {
    setForm((current) => {
      if (!current) return current;
      const next = current.clickRegions.map((r, i) =>
        i === index ? { ...r, delayMs } : r,
      );
      return { ...current, clickRegions: next };
    });
  }, []);

  const handleAddClickRegion = useCallback(async () => {
    if (!isNativeShell) {
      setStatusMessage("浏览器预览模式下不可执行区域框选，请在桌面端使用。");
      return;
    }

    // 找到第一个空槽位
    const regions = form?.clickRegions ?? [];
    const emptyIndex = regions.findIndex((r) => !r.rect);
    if (emptyIndex === -1) {
      setStatusMessage("点击区域已满（最多 7 个）。");
      return;
    }

    setSelectingSlot(-2);
    setStatusMessage(`请在悬浮层中框选一个新的点击区域（槽位 ${emptyIndex + 1}）。`);

    try {
      const outcome = await invoke<RegionSelectionOutcome>("morse_begin_region_selection", {
        slots: [emptyIndex],
        target: "click",
      });
      await syncBootstrap("full");

      if (outcome.kind === "selected") {
        setStatusMessage(`点击区域 ${emptyIndex + 1} 已添加。`);
      } else if (outcome.kind === "cancelled") {
        setStatusMessage("点击区域选择已取消。");
      } else {
        setStatusMessage("点击区域选择窗口已关闭。");
      }
    } catch (error) {
      const message = getErrorMessage(error);
      setPageError(message);
      setStatusMessage(message);
    } finally {
      setSelectingSlot(null);
    }
  }, [form?.clickRegions, isNativeShell, syncBootstrap]);

  const handleRemoveClickRegion = useCallback((index: number) => {
    setForm((current) => {
      if (!current) return current;
      const next = current.clickRegions.map((r, i) =>
        i === index ? { rect: null, delayMs: "500" } : r,
      );
      return { ...current, clickRegions: next };
    });
  }, []);

  const handleVerificationRun = useCallback(async () => {
    if (!isNativeShell) {
      setVerificationStatus("error");
      setVerificationMessage("浏览器预览模式下不可执行测试验证，请在桌面端运行。");
      setStatusMessage("浏览器预览模式下不可执行识别，请在桌面端运行。");
      return;
    }

    setVerificationStatus("running");
    setVerificationMessage("正在执行一次仅识别测试验证...");

    try {
      setRunning(true);
      setStatusMessage("正在执行测试验证...");
      const result = await invoke<MorseRunResult>("morse_run_recognition", { autoType: false });
      startTransition(() => {
        setBootstrap((current) => (current ? { ...current, latestRun: result } : current));
        setPageError(result.error);
        setStatusMessage(result.error ? `识别失败：${result.error}` : `识别完成：${result.value ?? "无结果"}`);

        if (result.error) {
          setVerificationStatus("error");
          setVerificationMessage(result.error);
          return;
        }

        if (!result.value) {
          setVerificationValue("");
          setVerificationStatus("empty");
          setVerificationMessage("识别流程已执行，但本次没有得到可用结果。");
          return;
        }

        setVerificationValue(result.value);
        setVerificationStatus("success");
        setVerificationMessage("验证完成，结果已回填到输入框。再次聚焦会重新执行识别。");
      });
    } catch (error) {
      const message = getErrorMessage(error);
      setPageError(message);
      setStatusMessage(message);
      setVerificationStatus("error");
      setVerificationMessage(message);
    } finally {
      setRunning(false);
      try {
        await syncBootstrap("none");
      } catch (error) {
        setPageError(getErrorMessage(error));
      }
    }
  }, [isNativeShell, syncBootstrap]);

  const beginHotkeyRecording = useCallback(() => {
    if (!form) {
      return;
    }

    hotkeyDraftRef.current = form.hotkey;
    setIsRecordingHotkey(true);
    setStatusMessage("正在录制热键，按下组合键后会自动更新。");
  }, [form]);

  const handleHotkeyRecorderKeyDown = useCallback((event: React.KeyboardEvent<HTMLButtonElement>) => {
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
  }, [form, isRecordingHotkey, updateForm]);

  const handleHotkeyRecorderBlur = useCallback(() => {
    if (!isRecordingHotkey) {
      return;
    }

    updateForm("hotkey", hotkeyDraftRef.current);
    setIsRecordingHotkey(false);
    setStatusMessage("热键录制已结束。");
  }, [isRecordingHotkey, updateForm]);

  if (overlayMode) {
    return <RegionSelectionOverlay slots={overlaySlots} />;
  }

  const controlsDisabled = isBusy || !isNativeShell;

  return (
    <AppPage>
      <PageHero
        eyebrow="Morse Recognition"
        title="摩斯密码解析"
        description="把区域框选、识别参数、验证输入和历史结果串成一条清晰流程，适合战局中快速校准。"
        badges={
          <>
            <Badge variant={canRun ? "default" : "secondary"}>{canRun ? "可执行" : "等待区域配置"}</Badge>
            <SaveStateBadge dirty={isDirty} saving={saving} />
            <Badge variant={isBusy ? "outline" : "secondary"}>{isBusy ? "处理中" : "空闲"}</Badge>
          </>
        }
        stats={
          <>
            <SignalTile label="区域配置" value={`${configuredCount}/3`} detail="三段采样区域连续框选" />
            <SignalTile label="最近结果" value={latestResultValue ?? "---"} detail={latestRunErrorOrFallback(latestRun?.error)} />
            <SignalTile label="最近时间" value={formatTimestamp(latestResultTime)} detail="自动输入与热键触发同步记录" />
          </>
        }
      />

      <div className="mx-auto flex w-full max-w-7xl flex-col gap-5">
          <SelectionPanel
            configuredCount={configuredCount}
            form={form}
            isBusy={controlsDisabled}
            isPreviewMode={!isNativeShell}
            isPrimary={!stepOneComplete}
            selectingSlot={selectingSlot}
            onSelectAll={() => void performSelectionSession([0, 1, 2])}
            onSelectOne={(slot) => void performSelectionSession([slot])}
          />

          <WorkbenchControlPanel
            form={form}
            hotkeyButtonRef={hotkeyButtonRef}
            hotkeyError={bootstrap?.hotkeyError}
            isPrimary={stepTwoActive}
            isRecordingHotkey={isRecordingHotkey}
            isVerifying={verificationStatus === "running"}
            onAutoInputDelayChange={(value) => updateForm("autoInputDelay", value)}
            onBeginHotkeyRecording={beginHotkeyRecording}
            onBinaryThresholdChange={(value) => updateForm("binaryThreshold", value)}
            onHotkeyRecorderBlur={handleHotkeyRecorderBlur}
            onHotkeyRecorderKeyDown={handleHotkeyRecorderKeyDown}
            onVerificationChange={setVerificationValue}
            onVerificationFocus={() => void handleVerificationRun()}
            onVerificationRetry={() => void handleVerificationRun()}
            verificationMessage={verificationMessage}
            verificationStatus={verificationStatus}
            verificationValue={verificationValue}
            autoClickEnabled={form?.autoClickEnabled ?? false}
            clickRegions={form?.clickRegions ?? []}
            isBusy={isBusy}
            onAutoClickEnabledChange={(value) => updateForm("autoClickEnabled", value)}
            onUpdateClickRegionDelay={handleUpdateClickRegionDelay}
            onAddClickRegion={() => void handleAddClickRegion()}
            onRemoveClickRegion={handleRemoveClickRegion}
          />

          <ResultPanel
            hasResult={hasLatestResult}
            isPrimary={stepThreeActive}
            latestAutoTyped={Boolean(latestRun?.autoTyped)}
            latestRunError={latestRun?.error}
            latestRunValue={latestRun?.value}
            latestTriggeredBy={latestRun?.triggeredBy}
            runDetails={runDetails}
          />

          <div className="pt-2">
            <HistoryPanel history={history} isPreviewMode={!isNativeShell} />
          </div>
      </div>
    </AppPage>
  );
}

function latestRunErrorOrFallback(error: string | null | undefined): string {
  return error ? "最近一次识别失败" : "最新三位结果会在这里放大显示";
}

import { startTransition, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import { useNativeShell } from "@/hooks/use-native-shell";
import { useAutosave } from "@/hooks/use-autosave";
import { useBootstrapForm } from "@/hooks/use-bootstrap-form";
import { useHotkeyRecorder } from "@/hooks/use-hotkey-recorder";

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
  const hotkeyButtonRef = useRef<HTMLButtonElement | null>(null);
  const [running, setRunning] = useState(false);
  const [selectingSlot, setSelectingSlot] = useState<number | null>(null);
  const [verificationValue, setVerificationValue] = useState("");
  const [verificationStatus, setVerificationStatus] = useState<VerificationStatus>("idle");
  const [verificationMessage, setVerificationMessage] = useState("点击验证输入框即可执行一次仅识别流程，结果会直接回填到这里。");

  const bf = useBootstrapForm<MorseBootstrap, MorseSettings, MorseSettingsForm>({
    spec: {
      getBootstrapCommand: "morse_get_bootstrap",
      saveSettingsCommand: "morse_save_settings",
      settingsToForm,
      parseSettingsForm,
    },
    isNativeShell,
    skipInitialLoad: overlayMode,
    loadStatusMessage: "正在加载摩斯工具...",
    readyStatusMessage: "就绪，可开始框选区域或执行识别。",
    previewStatusMessage: "浏览器预览模式：当前仅验证布局与滚动，原生命令请在桌面端运行。",
    saveSuccessMessage: "设置已保存。新的热键与识别参数已生效。",
    saveInProgressMessage: "正在保存设置...",
    useStartTransition: true,
  });

  const { bootstrap, setBootstrap, form, setForm, isDirty, updateForm, saveSettings, syncBootstrap, loading, saving, pageError: _pageError, setPageError, statusMessage: _statusMessage, setStatusMessage, autosaveVersionRef: autosaveRef } = bf;

  const recorder = useHotkeyRecorder({
    formatKey: formatRecordedHotkey,
    onCommit: (key) => {
      updateForm("hotkey", key);
      setPageError(null);
    },
    onCancel: (draft) => updateForm("hotkey", draft),
    onStatusMessage: setStatusMessage,
    keyRecordedMessage: (key) => `新的热键已录制：${key}`,
    recordingCancelledMessage: "已取消热键录制。",
  });

  useAutosave<MorseSettingsForm>({
    form,
    isDirty,
    disabled: overlayMode || !isNativeShell || loading || !bootstrap || !form || recorder.isRecording || selectingSlot !== null,
    onSave: (formSnapshot, nextVersion) => {
      const settingsValue = parseSettingsForm(formSnapshot);
      return saveSettings(settingsValue, nextVersion);
    },
    onError: (message) => {
      setPageError(message);
      setStatusMessage(`保存失败：${message}`);
    },
    delay: AUTOSAVE_DELAY_MS,
    autosaveVersionRef: autosaveRef,
  });

  useEffect(() => {
    if (recorder.isRecording) {
      hotkeyButtonRef.current?.focus();
    }
  }, [recorder.isRecording]);

  useEffect(() => {
    if (overlayMode || !isNativeShell) {
      return;
    }

    let disposed = false;

    void invoke("morse_set_hotkey_recording", { recording: recorder.isRecording }).catch((error) => {
      if (!disposed) {
        const message = getErrorMessage(error);
        setPageError(message);
        setStatusMessage(message);
      }
    });

    return () => {
      disposed = true;
    };
  }, [isNativeShell, recorder.isRecording, overlayMode]);

  useEffect(() => {
    if (!overlayMode) {
      return;
    }

    document.body.dataset.overlayMode = "true";
    return () => {
      delete document.body.dataset.overlayMode;
    };
  }, [overlayMode]);

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
        await syncBootstrap({ syncMode: "none" });
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
        target: "sampling",
      });
      await syncBootstrap({ syncMode: "regions" });

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
      await syncBootstrap({ syncMode: "full" });

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
        await syncBootstrap({ syncMode: "none" });
      } catch (error) {
        setPageError(getErrorMessage(error));
      }
    }
  }, [isNativeShell, syncBootstrap]);

  if (overlayMode) {
    return <RegionSelectionOverlay slots={overlaySlots} />;
  }

  const selectionControlsDisabled = loading || running || selectingSlot !== null || !isNativeShell;

  return (
    <AppPage className="auto-rows-max gap-4">
      <PageHero
        eyebrow="MX-01 / DECODER"
        title="摩斯信号破译台"
        description="把采样窗位、阈值校准、单次验证与识别回溯串成一条硬线路，供战局内快速复核三码信号。"
        badges={
          <>
            <Badge variant={canRun ? "default" : "secondary"}>{canRun ? "三区就绪" : "等待窗位标定"}</Badge>
            <SaveStateBadge dirty={isDirty} saving={saving} />
            <Badge variant={isBusy ? "outline" : "secondary"}>{isBusy ? "链路占用" : "链路待命"}</Badge>
          </>
        }
        stats={
          <>
            <SignalTile label="采样阵列" value={`${configuredCount}/3`} detail="三段信号窗位完成标定" />
            <SignalTile label="最新报码" value={latestResultValue ?? "---"} detail={latestRunErrorOrFallback(latestRun?.error)} />
            <SignalTile label="最近触发" value={formatTimestamp(latestResultTime)} detail="自动输入与热键触发统一归档" />
          </>
        }
      />

      {/* 结构分隔线 */}
      <div className="col-span-12 h-0.5 bg-[var(--ink)]" />

      <div className="col-span-12 grid min-h-0 gap-4 xl:col-span-4">
        <SelectionPanel
          configuredCount={configuredCount}
          form={form}
          isBusy={selectionControlsDisabled}
          isPreviewMode={!isNativeShell}
          isPrimary={!stepOneComplete}
          selectingSlot={selectingSlot}
          onSelectAll={() => void performSelectionSession([0, 1, 2])}
          onSelectOne={(slot) => void performSelectionSession([slot])}
        />
      </div>

      <div className="col-span-12 grid min-h-0 gap-4 xl:col-span-8">
        <WorkbenchControlPanel
          form={form}
          hotkeyButtonRef={hotkeyButtonRef}
          hotkeyError={bootstrap?.hotkeyError}
          isPrimary={stepTwoActive}
          isRecordingHotkey={recorder.isRecording}
          isVerifying={verificationStatus === "running"}
          onAutoInputDelayChange={(value) => updateForm("autoInputDelay", value)}
          onBeginHotkeyRecording={() => form && recorder.beginRecording(form.hotkey)}
          onBinaryThresholdChange={(value) => updateForm("binaryThreshold", value)}
          onHotkeyRecorderBlur={recorder.handleBlur}
          onHotkeyRecorderKeyDown={recorder.handleKeyDown}
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
          onAfterClickHotkeyChange={(value) => updateForm("afterClickHotkey", value)}
          onUpdateClickRegionDelay={handleUpdateClickRegionDelay}
          onAddClickRegion={() => void handleAddClickRegion()}
          onRemoveClickRegion={handleRemoveClickRegion}
        />
      </div>

      <div className="col-span-12 grid min-h-0 gap-4 border-2 border-[var(--ink)] p-3 xl:col-span-7">
        <ResultPanel
          hasResult={hasLatestResult}
          isPrimary={stepThreeActive}
          latestAutoTyped={Boolean(latestRun?.autoTyped)}
          latestRunError={latestRun?.error}
          latestRunValue={latestRun?.value}
          latestTriggeredBy={latestRun?.triggeredBy}
          runDetails={runDetails}
        />
      </div>

      <div className="col-span-12 grid min-h-0 gap-4 border-2 border-[var(--ink)] p-3 xl:col-span-5">
        <HistoryPanel history={history} isPreviewMode={!isNativeShell} />
      </div>
    </AppPage>
  );
}

function latestRunErrorOrFallback(error: string | null | undefined): string {
  return error ? "最近一次识别失败，需回查三码链路" : "最新三码会在报码窗中放大显示";
}

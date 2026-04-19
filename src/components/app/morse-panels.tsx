import {
  RiCheckboxCircleLine,
  RiHistoryLine,
  RiLayoutGridLine,
  RiRefreshLine,
  RiSettings3Line,
  RiSparklingLine,
} from "@remixicon/react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Field, FieldContent, FieldDescription, FieldError, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import type {
  HistoryEntry,
  MorseRegionDetail,
  MorseSettingsForm,
  VerificationStatus,
} from "@/components/app/morse-types";
import { REGION_LABELS } from "@/components/app/morse-types";
import { formatRegion, formatTimestamp } from "@/components/app/morse-utils";

type SelectionPanelProps = {
  configuredCount: number;
  isBusy: boolean;
  selectingSlot: number | null;
  form: MorseSettingsForm | null;
  onSelectAll: () => void;
  onSelectOne: (slot: number) => void;
};

export function SelectionPanel({ configuredCount, form, isBusy, selectingSlot, onSelectAll, onSelectOne }: SelectionPanelProps) {
  return (
    <Card size="sm" className="desktop-panel-card border border-border/70 shadow-sm">
      <CardHeader className="desktop-panel-header border-b border-border/60">
        <div className="flex items-center gap-2">
          <RiLayoutGridLine className="text-muted-foreground" />
          <div>
            <CardTitle>采样区域</CardTitle>
            <CardDescription>优先使用一次完成 3 个区域的连续框选；单区域仅用于纠偏或重选。</CardDescription>
          </div>
        </div>
      </CardHeader>
      <CardContent className="space-y-4 xl:space-y-5">
        <div className="desktop-priority-strip flex flex-wrap items-center justify-between gap-3 rounded-2xl border border-border/70 px-4 py-4">
          <div>
            <p className="text-sm font-medium text-foreground">推荐路径</p>
            <p className="mt-1 text-xs text-muted-foreground">先完成 3 个区域配置，再执行识别。当前已配置 {configuredCount}/3。</p>
          </div>
          <Button disabled={isBusy} onClick={onSelectAll} type="button" size="lg">
            <RiRefreshLine data-icon="inline-start" />
            一次选择 3 个区域
          </Button>
        </div>

        <div className="grid gap-3 xl:grid-cols-3">
          {REGION_LABELS.map((label, index) => {
            const region = form?.regions[index] ?? null;
            const isConfigured = Boolean(region);
            const isSelecting = selectingSlot === index;

            return (
              <div key={label} className="desktop-subpanel desktop-region-card rounded-2xl border border-border/70 bg-card/85 p-3.5 shadow-sm">
                <div className="flex items-start justify-between gap-2">
                  <div className="min-w-0">
                    <p className="text-sm font-medium text-foreground">{label}</p>
                    <p className="mt-1 text-xs text-muted-foreground">
                      {isSelecting ? "正在当前轮次中等待框选。" : isConfigured ? "坐标已保存，可直接执行识别。" : "尚未配置，需要先完成框选。"}
                    </p>
                  </div>
                  <Badge variant={isSelecting ? "secondary" : isConfigured ? "default" : "outline"}>
                    {isSelecting ? "正在框选" : isConfigured ? "已配置" : "未配置"}
                  </Badge>
                </div>

                <div className="mt-3 rounded-xl border border-dashed border-border/80 bg-muted/30 px-3 py-3">
                  <p className="desktop-caption">区域摘要</p>
                  <p className="desktop-mono mt-2 overflow-hidden text-ellipsis whitespace-nowrap">{formatRegion(region)}</p>
                </div>

                <div className="mt-3 flex gap-2">
                  <Button
                    className="flex-1"
                    disabled={isBusy}
                    onClick={() => onSelectOne(index)}
                    type="button"
                    variant={isConfigured ? "outline" : "default"}
                  >
                    {isSelecting ? "等待中..." : isConfigured ? "重选本区域" : "选择区域"}
                  </Button>
                </div>
              </div>
            );
          })}
        </div>
      </CardContent>
    </Card>
  );
}

type WorkbenchControlPanelProps = {
  form: MorseSettingsForm | null;
  hotkeyError: string | null | undefined;
  hotkeyButtonRef: React.RefObject<HTMLButtonElement | null>;
  isRecordingHotkey: boolean;
  isVerifying: boolean;
  verificationMessage: string;
  verificationStatus: VerificationStatus;
  verificationValue: string;
  onAutoInputDelayChange: (value: string) => void;
  onBeginHotkeyRecording: () => void;
  onBinaryThresholdChange: (value: string) => void;
  onHotkeyRecorderBlur: () => void;
  onHotkeyRecorderKeyDown: (event: React.KeyboardEvent<HTMLButtonElement>) => void;
  onVerificationChange: (value: string) => void;
  onVerificationFocus: () => void;
  onVerificationRetry: () => void;
};

export function WorkbenchControlPanel({
  form,
  hotkeyButtonRef,
  hotkeyError,
  isRecordingHotkey,
  isVerifying,
  onAutoInputDelayChange,
  onBeginHotkeyRecording,
  onBinaryThresholdChange,
  onHotkeyRecorderBlur,
  onHotkeyRecorderKeyDown,
  onVerificationChange,
  onVerificationFocus,
  onVerificationRetry,
  verificationMessage,
  verificationStatus,
  verificationValue,
}: WorkbenchControlPanelProps) {
  return (
    <Card size="sm" className="desktop-panel-card desktop-console-card border border-border/70 shadow-sm">
      <CardHeader className="desktop-panel-header desktop-console-header border-b border-border/60">
        <div className="flex min-w-0 items-start gap-3">
          <div className="flex size-10 shrink-0 items-center justify-center rounded-2xl border border-border/70 bg-background/90 text-primary shadow-sm">
            <RiSparklingLine />
          </div>
          <div className="min-w-0">
            <p className="desktop-caption">Workbench Console</p>
            <CardTitle className="mt-2 text-base font-semibold">设置与测试验证</CardTitle>
            <CardDescription className="mt-1 max-w-3xl">
              左侧维护热键与阈值配置，右侧直接做测试验证。点击验证输入框会执行一次仅识别流程，并把结果回填到输入框中。
            </CardDescription>
          </div>
        </div>
      </CardHeader>

      <CardContent className="desktop-console-content px-4 py-4 xl:px-5 xl:py-5">
        <div className="desktop-console-grid grid gap-4 xl:grid-cols-[minmax(0,1.2fr)_minmax(20rem,0.88fr)]">
          <section className="desktop-console-pane desktop-console-settings rounded-[1.5rem] border border-border/70 bg-card/88 px-4 py-4 shadow-sm">
            <div className="mb-4 flex items-center gap-2">
              <RiSettings3Line className="text-muted-foreground" />
              <div>
                <h3 className="text-sm font-semibold text-foreground">参数设置</h3>
                <p className="mt-1 text-xs text-muted-foreground">变更自动保存；热键继续保持录制式交互。</p>
              </div>
            </div>

            {form ? (
              <FieldGroup className="desktop-settings-fields flex flex-1 flex-col gap-4">
                <Field className="desktop-settings-field">
                  <FieldLabel htmlFor="hotkey-recorder">热键</FieldLabel>
                  <FieldContent className="desktop-settings-field-content">
                    <Button
                      ref={hotkeyButtonRef}
                      className="h-auto w-full justify-between gap-4 py-2 font-mono"
                      id="hotkey-recorder"
                      onBlur={onHotkeyRecorderBlur}
                      onClick={onBeginHotkeyRecording}
                      onKeyDown={onHotkeyRecorderKeyDown}
                      type="button"
                      variant="outline"
                    >
                      <span>{isRecordingHotkey ? "正在录制，按下快捷键..." : form.hotkey || "点击录制热键"}</span>
                      <span className="text-[0.6875rem] text-muted-foreground">{isRecordingHotkey ? "Esc 取消" : "点击录制"}</span>
                    </Button>
                    <FieldDescription>
                      {isRecordingHotkey ? "录制中：支持字母、数字、功能键和常见导航键。" : "点击按钮进入录制状态，录制完成后会自动保存。"}
                    </FieldDescription>
                    <FieldError>{hotkeyError}</FieldError>
                  </FieldContent>
                </Field>

                <Field className="desktop-settings-field">
                  <FieldLabel htmlFor="binary-threshold">二值化阈值</FieldLabel>
                  <FieldContent className="desktop-settings-field-content">
                    <Input
                      id="binary-threshold"
                      inputMode="numeric"
                      max="255"
                      min="0"
                      onChange={(event) => onBinaryThresholdChange(event.currentTarget.value)}
                      value={form.binaryThreshold}
                    />
                    <FieldDescription>控制图像转二值图时的阈值，推荐保持在 0 到 255 的可调范围内微调。</FieldDescription>
                  </FieldContent>
                </Field>

                <Field className="desktop-settings-field">
                  <FieldLabel htmlFor="auto-input-delay">自动输入延迟（毫秒）</FieldLabel>
                  <FieldContent className="desktop-settings-field-content">
                    <Input
                      id="auto-input-delay"
                      inputMode="numeric"
                      min="0"
                      onChange={(event) => onAutoInputDelayChange(event.currentTarget.value)}
                      value={form.autoInputDelay}
                    />
                    <FieldDescription>热键流程下识别成功后自动输入前的等待时间，适合为切回目标窗口预留缓冲。</FieldDescription>
                  </FieldContent>
                </Field>
              </FieldGroup>
            ) : (
              <div className="text-xs text-muted-foreground">正在加载设置...</div>
            )}
          </section>

          <section className="desktop-console-pane desktop-console-verification rounded-[1.5rem] border border-border/70 bg-card/88 px-4 py-4 shadow-sm">
            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0">
                <div className="flex items-center gap-2">
                  <RiSparklingLine className="text-primary" />
                  <h3 className="text-sm font-semibold text-foreground">测试验证</h3>
                </div>
                <p className="mt-2 text-xs/relaxed text-muted-foreground">
                  点击下面的验证输入框会执行一次仅识别流程，结果只写回当前工作台，不会自动输入到外部窗口。
                </p>
              </div>

              <Badge variant={verificationStatus === "error" ? "outline" : verificationStatus === "success" ? "default" : "secondary"}>
                {verificationStatus === "running"
                  ? "验证中"
                  : verificationStatus === "success"
                    ? "已回填"
                    : verificationStatus === "empty"
                      ? "无结果"
                      : verificationStatus === "error"
                        ? "失败"
                        : "待验证"}
              </Badge>
            </div>

            <div className="desktop-verification-box mt-4 rounded-[1.35rem] border border-border/70 bg-background/92 p-4 shadow-sm">
              <label className="desktop-caption" htmlFor="verification-input">
                Verification Input
              </label>
              <Input
                className="desktop-verification-input mt-3 h-12 rounded-xl px-4 font-mono text-base tracking-[0.22em] md:text-sm"
                id="verification-input"
                onChange={(event) => onVerificationChange(event.currentTarget.value)}
                onFocus={onVerificationFocus}
                placeholder="点击此处执行测试验证"
                value={verificationValue}
              />
              <p className="mt-3 text-xs text-muted-foreground">{verificationMessage}</p>
            </div>

            <div className="mt-4 flex flex-wrap items-center justify-between gap-3">
              <div className="desktop-subpanel rounded-xl border border-border/70 bg-background/85 px-3 py-3">
                <p className="desktop-caption">验证模式</p>
                <p className="mt-1 text-sm text-foreground">{isVerifying ? "正在执行仅识别流程..." : "聚焦输入框即可重新验证"}</p>
              </div>
              <Button disabled={isVerifying} onClick={onVerificationRetry} type="button" variant="outline">
                <RiRefreshLine data-icon="inline-start" />
                重新验证
              </Button>
            </div>
          </section>
        </div>
      </CardContent>
    </Card>
  );
}

type ResultPanelProps = {
  latestRunValue: string | null | undefined;
  latestRunError: string | null | undefined;
  latestTriggeredBy: string | null | undefined;
  latestAutoTyped: boolean;
  runDetails: MorseRegionDetail[];
};

export function ResultPanel({ latestAutoTyped, latestRunError, latestRunValue, latestTriggeredBy, runDetails }: ResultPanelProps) {
  return (
    <Card size="sm" className="desktop-panel-card desktop-result-card border border-border/70 shadow-sm">
      <CardHeader className="desktop-panel-header desktop-result-header border-b border-border/60">
        <div className="flex items-center gap-2">
          <RiCheckboxCircleLine className="text-muted-foreground" />
          <div className="desktop-panel-heading">
            <CardTitle>解析结果</CardTitle>
            <CardDescription>解析结果独占一整行展示，主结果优先显示，区域级细节用于判断识别质量与排查失败原因。</CardDescription>
          </div>
        </div>
      </CardHeader>
      <CardContent className="desktop-result-content flex flex-col gap-4 px-4 pt-4 pb-4 xl:px-5 xl:pt-5 xl:pb-5">
        <div className="desktop-result-hero rounded-[1.75rem] border border-border/70 bg-background px-5 py-5 shadow-sm xl:px-6 xl:py-6">
          <div className="flex flex-wrap items-center gap-2">
            <Badge variant={latestRunError ? "outline" : latestRunValue ? "default" : "secondary"}>
              {latestRunError ? "失败" : latestRunValue ? "成功" : "等待执行"}
            </Badge>
            {latestTriggeredBy ? <Badge variant="outline">来源 {latestTriggeredBy}</Badge> : null}
            {latestAutoTyped ? <Badge variant="outline">已自动输入</Badge> : null}
          </div>
          <p className="mt-5 font-mono text-5xl font-semibold tracking-[0.36em] text-foreground/92 xl:text-6xl">
            {latestRunValue ?? "---"}
          </p>
          <p className="mt-3 text-xs text-muted-foreground">{latestRunError ?? "执行识别后会在这里显示最新三位结果。"}</p>
        </div>

        <div className="desktop-result-details grid gap-3 xl:grid-cols-3">
          {runDetails.map((detail) => (
            <div key={detail.slot} className="desktop-subpanel desktop-detail-card rounded-2xl border border-border/70 bg-card/85 p-3.5 shadow-sm">
              <div className="flex items-center justify-between gap-2">
                <p className="text-xs font-medium text-foreground">{REGION_LABELS[detail.slot] ?? `位置 ${detail.slot + 1}`}</p>
                <Badge variant={detail.error ? "outline" : detail.digit ? "default" : "secondary"}>
                  {detail.error ? "失败" : detail.digit ? detail.digit : "待机"}
                </Badge>
              </div>
              <div className="mt-3 grid gap-2 text-xs text-muted-foreground">
                <div className="desktop-subpanel bg-background/90 px-3 py-2">
                  <p className="desktop-caption">Morse</p>
                  <p className="mt-1 font-mono text-foreground/80">{detail.morse ?? "--"}</p>
                </div>
                <div className="desktop-subpanel bg-background/90 px-3 py-2">
                  <p className="desktop-caption">Threshold</p>
                  <p className="mt-1 text-foreground/80">{detail.thresholdMode}</p>
                </div>
                <div className="desktop-subpanel bg-background/90 px-3 py-2">
                  <p className="desktop-caption">Contours</p>
                  <p className="mt-1 text-foreground/80">{detail.contourCount}</p>
                </div>
              </div>
              {detail.error ? <p className="mt-3 text-xs/relaxed text-destructive">{detail.error}</p> : null}
            </div>
          ))}
        </div>
      </CardContent>
    </Card>
  );
}

type HistoryPanelProps = {
  history: HistoryEntry[];
};

export function HistoryPanel({ history }: HistoryPanelProps) {
  return (
    <Card size="sm" className="desktop-panel-card border border-border/70 shadow-sm">
      <CardHeader className="desktop-panel-header border-b border-border/60">
        <div className="flex items-center gap-2">
          <RiHistoryLine className="text-muted-foreground" />
          <div>
            <CardTitle>历史记录</CardTitle>
            <CardDescription>保留最近的识别结果与失败记录，用于复查识别输出与触发来源。</CardDescription>
          </div>
        </div>
      </CardHeader>
      <CardContent>
        <ScrollArea className="desktop-scroll-area h-72">
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
                <div key={entry.id} className="desktop-subpanel desktop-history-entry rounded-2xl border border-border/70 bg-card/85 p-3.5 shadow-sm">
                  <div className="flex flex-wrap items-center justify-between gap-2">
                    <div className="flex flex-wrap items-center gap-2">
                      <p className="text-xs font-medium text-foreground">{entry.result ? `识别结果 ${entry.result}` : "识别失败"}</p>
                      <Badge variant={entry.success ? "default" : "outline"}>{entry.success ? "成功" : "失败"}</Badge>
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
  );
}

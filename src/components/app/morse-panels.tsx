import {
  RiCheckboxCircleLine,
  RiEyeLine,
  RiHistoryLine,
  RiLayoutGridLine,
  RiRefreshLine,
  RiSettings3Line,
  RiSparklingLine,
} from "@remixicon/react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Field, FieldContent, FieldError, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { CardBody, ControlTile, SectionHeader, TacticalCard } from "@/components/app/app-ui";

import type {
  HistoryEntry,
  MorseRegionDetail,
  MorseSettingsForm,
  RegionRect,
  VerificationStatus,
} from "@/components/app/morse-types";
import { REGION_LABELS } from "@/components/app/morse-types";
import { clickRegionRows, formatRegion, formatTimestamp } from "@/components/app/morse-utils";

type SelectionPanelProps = {
  configuredCount: number;
  isBusy: boolean;
  isPrimary?: boolean;
  selectingSlot: number | null;
  form: MorseSettingsForm | null;
  isPreviewMode?: boolean;
  onSelectAll: () => void;
  onSelectOne: (slot: number) => void;
};

export function SelectionPanel({ configuredCount, form, isBusy, isPrimary = false, selectingSlot, isPreviewMode, onSelectAll, onSelectOne }: SelectionPanelProps) {
  return (
    <TacticalCard active={isPrimary}>
      <SectionHeader
        eyebrow="Step 01"
        icon={<RiLayoutGridLine />}
        title="配置采样区域"
        description="先完成 3 个区域配置，再执行识别。"
        badge={<Badge variant={configuredCount === 3 ? "default" : "outline"}>{configuredCount}/3</Badge>}
      />
      <CardBody className="flex flex-col gap-4 xl:gap-5">
        {isPreviewMode ? (
          <div className="rounded-lg border border-dashed border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-tile),color-mix(in_oklch,var(--card)_34%,transparent))] px-4 py-8 text-center">
            <RiEyeLine className="mx-auto mb-2 text-muted-foreground" />
            <p className="text-sm font-medium text-muted-foreground">预览模式</p>
            <p className="mt-1 text-xs text-muted-foreground">启动桌面程序以配置采样区域</p>
          </div>
        ) : (
          <>
            <ControlTile className="flex flex-wrap items-center justify-between gap-3">
              <div className="flex flex-wrap items-center gap-3">
                <div className="flex items-center gap-2">
                  <div className="size-2.5 rounded-full bg-primary" style={{ opacity: configuredCount >= 1 ? 1 : 0.25 }} />
                  <div className="size-2.5 rounded-full bg-primary" style={{ opacity: configuredCount >= 2 ? 1 : 0.25 }} />
                  <div className="size-2.5 rounded-full bg-primary" style={{ opacity: configuredCount >= 3 ? 1 : 0.25 }} />
                </div>
                <div>
                  <p className="text-sm font-medium text-foreground">已配置 {configuredCount}/3</p>
                  <p className="mt-1 text-xs text-muted-foreground">推荐先一次完成 3 个区域，再进入设置与验证。</p>
                </div>
              </div>
              <Button disabled={isBusy} onClick={onSelectAll} type="button" size="lg">
                <RiRefreshLine data-icon="inline-start" />
                一次选择 3 个区域
              </Button>
            </ControlTile>

            <div className="grid gap-3 xl:grid-cols-3">
              {REGION_LABELS.map((label, index) => {
                const region = form?.regions[index] ?? null;
                const isConfigured = Boolean(region);
                const isSelecting = selectingSlot === index;

                return (
                  <ControlTile key={label} className="flex flex-col gap-3">
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

                    <div className="rounded-lg border border-dashed border-[var(--surface-border)] bg-[linear-gradient(145deg,color-mix(in_oklch,var(--card)_48%,transparent),var(--surface-tile))] px-3 py-3">
                      <p className="text-xs font-medium tracking-[0.18em] text-muted-foreground uppercase">区域摘要</p>
                      <p className="mt-2 overflow-hidden font-mono text-[0.6875rem] text-foreground/80 text-ellipsis whitespace-nowrap">{formatRegion(region)}</p>
                    </div>

                    <div className="flex gap-2">
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
                  </ControlTile>
                );
              })}
            </div>
          </>
        )}
      </CardBody>
    </TacticalCard>
  );
}
type WorkbenchControlPanelProps = {
  form: MorseSettingsForm | null;
  hotkeyError: string | null | undefined;
  hotkeyButtonRef: React.RefObject<HTMLButtonElement | null>;
  isBusy: boolean;
  isRecordingHotkey: boolean;
  isPrimary?: boolean;
  isVerifying: boolean;
  verificationMessage: string;
  verificationStatus: VerificationStatus;
  verificationValue: string;
  autoClickEnabled: boolean;
  clickRegions: { rect: RegionRect | null; delayMs: string }[];
  onAutoClickEnabledChange: (value: boolean) => void;
  onAutoInputDelayChange: (value: string) => void;
  onAfterClickHotkeyChange: (value: string) => void;
  onBeginHotkeyRecording: () => void;
  onBinaryThresholdChange: (value: string) => void;
  onHotkeyRecorderBlur: () => void;
  onHotkeyRecorderKeyDown: (event: React.KeyboardEvent<HTMLButtonElement>) => void;
  onUpdateClickRegionDelay: (index: number, delayMs: string) => void;
  onAddClickRegion: () => void;
  onRemoveClickRegion: (index: number) => void;
  onVerificationChange: (value: string) => void;
  onVerificationFocus: () => void;
  onVerificationRetry: () => void;
};

export function WorkbenchControlPanel({
  form,
  hotkeyButtonRef,
  hotkeyError,
  isBusy,
  isRecordingHotkey,
  isPrimary = false,
  isVerifying,
  autoClickEnabled,
  clickRegions,
  onAutoClickEnabledChange,
  onAutoInputDelayChange,
  onAfterClickHotkeyChange,
  onBeginHotkeyRecording,
  onBinaryThresholdChange,
  onHotkeyRecorderBlur,
  onHotkeyRecorderKeyDown,
  onUpdateClickRegionDelay,
  onAddClickRegion,
  onRemoveClickRegion,
  onVerificationChange,
  onVerificationFocus,
  onVerificationRetry,
  verificationMessage,
  verificationStatus,
  verificationValue,
}: WorkbenchControlPanelProps) {
  return (
    <TacticalCard active={isPrimary}>
      <SectionHeader
        eyebrow="Step 02"
        icon={<RiSettings3Line />}
        title="调整设置并验证"
        description="区域准备完成后，在这里微调参数并验证识别效果。"
        badge={<Badge variant={verificationStatus === "success" ? "default" : "outline"}>{verificationStatus === "running" ? "验证中" : "校准"}</Badge>}
      />

      <CardBody className="xl:min-h-88">
        <div className="grid gap-4 xl:grid-cols-[minmax(0,1.2fr)_minmax(20rem,0.88fr)]">
          <section className="min-h-0 rounded-xl border border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-tile),color-mix(in_oklch,var(--card)_48%,transparent))] px-4 py-4">

            <div className="mb-4 flex items-center gap-2">

              <RiSettings3Line className="text-muted-foreground" />
              <div>
                <h3 className="text-sm font-semibold text-foreground">参数设置</h3>
                <p className="mt-1 text-xs text-muted-foreground">调整识别参数，变更会自动保存。</p>
              </div>
            </div>

            {form ? (
              <FieldGroup className="flex flex-1 flex-col gap-4 xl:min-h-full">
                <Field className="xl:min-h-0">
                  <FieldLabel htmlFor="hotkey-recorder">热键</FieldLabel>
                  <FieldContent className="xl:flex xl:flex-col xl:gap-2.2 xl:min-h-0">
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
                      <span className="text-[0.6875rem] text-muted-foreground">{isRecordingHotkey ? "失焦取消" : "点击录制"}</span>
                     </Button>
                     <FieldError>{hotkeyError}</FieldError>
                  </FieldContent>
                </Field>

                <Field className="xl:min-h-0">
                  <FieldLabel htmlFor="binary-threshold">二值化阈值</FieldLabel>
                  <FieldContent className="xl:flex xl:flex-col xl:gap-2.2 xl:min-h-0">
                    <Input
                      id="binary-threshold"
                      inputMode="numeric"
                      max="255"
                      min="0"
                      onChange={(event) => onBinaryThresholdChange(event.currentTarget.value)}
                      value={form.binaryThreshold}
                     />
                   </FieldContent>
                </Field>

                <Field className="xl:min-h-0">
                  <FieldLabel htmlFor="auto-input-delay">自动输入延迟（毫秒）</FieldLabel>
                  <FieldContent className="xl:flex xl:flex-col xl:gap-2.2 xl:min-h-0">
                    <Input
                      id="auto-input-delay"
                      inputMode="numeric"
                      min="0"
                      onChange={(event) => onAutoInputDelayChange(event.currentTarget.value)}
                      value={form.autoInputDelay}
                     />
                   </FieldContent>
                </Field>

                <ControlTile className="flex items-center gap-3">
                  <Switch
                    checked={autoClickEnabled}
                    disabled={isBusy}
                    onCheckedChange={onAutoClickEnabledChange}
                  />
                  <div className="min-w-0">
                    <p className="text-sm font-medium text-foreground">识别后自动点击</p>
                    <p className="mt-1 text-xs text-muted-foreground">识别成功后按延迟依次点击已配置区域。</p>
                  </div>
                </ControlTile>
                {autoClickEnabled && (
                  <>
                    <Field className="xl:min-h-0">
                      <FieldLabel htmlFor="after-click-hotkey">点击完成后按键</FieldLabel>
                      <FieldContent className="xl:flex xl:flex-col xl:gap-2.2 xl:min-h-0">
                        <Input
                          id="after-click-hotkey"
                          placeholder="留空不执行，例如 F4 或 Ctrl+F4"
                          value={form.afterClickHotkey}
                          onChange={(event) => onAfterClickHotkeyChange(event.currentTarget.value)}
                        />
                      </FieldContent>
                    </Field>
                    <Field className="xl:min-h-0">
                      <FieldLabel>点击区域（最多 7 个）</FieldLabel>
                      <FieldContent className="xl:flex xl:flex-col xl:gap-2.2 xl:min-h-0">
                        <div className="flex flex-col gap-2">
                          {clickRegionRows(clickRegions).map((cr) => (
                            <ControlTile key={cr.slotIndex} className="flex items-center gap-3">
                              <Badge variant={cr.rect ? "default" : "outline"} className="shrink-0">
                                {cr.slotIndex + 1}
                              </Badge>
                              <span className="flex-1 font-mono text-xs text-muted-foreground">
                                {formatRegion(cr.rect)}
                              </span>
                              <Input
                                className="w-20"
                                inputMode="numeric"
                                min="0"
                                value={cr.delayMs}
                                onChange={(e) => onUpdateClickRegionDelay(cr.slotIndex, e.currentTarget.value)}
                              />
                              <span className="text-xs text-muted-foreground">ms</span>
                              <Button
                                className="h-7 w-7 shrink-0 p-0"
                                disabled={isBusy}
                                onClick={() => onRemoveClickRegion(cr.slotIndex)}
                                type="button"
                                variant="ghost"
                              >
                                ×
                              </Button>
                            </ControlTile>
                          ))}
                          {clickRegions.filter((r) => r.rect).length < 7 && (
                            <Button
                              disabled={isBusy}
                              onClick={onAddClickRegion}
                              type="button"
                              variant="outline"
                            >
                              <RiLayoutGridLine data-icon="inline-start" />
                              添加点击区域
                            </Button>
                          )}
                        </div>
                      </FieldContent>
                    </Field>
                  </>
                )}
              </FieldGroup>
            ) : (
              <div className="text-xs text-muted-foreground">正在加载设置...</div>
            )}
          </section>
          <section className="min-h-0 rounded-xl border border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-tile),color-mix(in_oklch,var(--card)_48%,transparent))] px-4 py-4">

            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0">
                <div className="flex items-center gap-2">
                  <RiSparklingLine className="text-primary" />
                  <h3 className="text-sm font-semibold text-foreground">测试验证</h3>
                </div>
                <p className="mt-1 text-xs text-muted-foreground">聚焦输入框或点击按钮，执行一次仅识别验证。</p>

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

            <div className="mt-4 rounded-lg border border-[var(--surface-border)] bg-[linear-gradient(145deg,color-mix(in_oklch,var(--card)_56%,transparent),var(--surface-tile))] p-4">
              <Input
                className="h-12 rounded-lg px-4 font-mono text-base tracking-[0.22em]"
                id="verification-input"
                onChange={(event) => onVerificationChange(event.currentTarget.value)}
                onFocus={onVerificationFocus}
                placeholder="点击此处执行测试验证"
                value={verificationValue}
              />
              <p className="mt-3 text-xs text-muted-foreground">{verificationMessage}</p>
            </div>

            <div className="mt-4 flex flex-wrap items-center justify-between gap-3">
              <div className="rounded-lg border border-[var(--surface-border)] bg-[linear-gradient(145deg,color-mix(in_oklch,var(--card)_50%,transparent),var(--surface-tile))] px-3 py-3">
                <p className="text-xs font-medium tracking-[0.18em] text-muted-foreground uppercase">验证模式</p>
                <p className="mt-1 text-sm text-foreground">{isVerifying ? "正在执行仅识别流程..." : "聚焦输入框即可重新验证"}</p>
              </div>
              <Button disabled={isVerifying} onClick={onVerificationRetry} type="button" variant="outline">
                <RiRefreshLine data-icon="inline-start" />
                重新验证
              </Button>
            </div>
          </section>
        </div>
      </CardBody>
    </TacticalCard>
  );
}

type ResultPanelProps = {
  hasResult?: boolean;
  isPrimary?: boolean;
  latestRunValue: string | null | undefined;
  latestRunError: string | null | undefined;
  latestTriggeredBy: string | null | undefined;
  latestAutoTyped: boolean;
  runDetails: MorseRegionDetail[];
};

export function ResultPanel({ hasResult = false, isPrimary = false, latestAutoTyped, latestRunError, latestRunValue, latestTriggeredBy, runDetails }: ResultPanelProps) {
  return (
    <TacticalCard active={isPrimary}>
      <SectionHeader
        eyebrow="Step 03"
        icon={<RiCheckboxCircleLine />}
        title="查看结果"
        description="完成前两步后，这里会显示最新识别结果。"
        badge={<Badge variant={latestRunError ? "outline" : latestRunValue ? "default" : "secondary"}>{latestRunError ? "失败" : latestRunValue ? "成功" : "待机"}</Badge>}
      />
      <CardBody className="flex min-h-0 flex-col gap-4">
        {!hasResult ? (
          <div className="rounded-xl border border-dashed border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-tile),color-mix(in_oklch,var(--card)_34%,transparent))] px-4 py-8 text-center">
            <RiCheckboxCircleLine className="mx-auto mb-2 text-muted-foreground" />
            <p className="text-sm font-medium text-muted-foreground">等待执行</p>
            <p className="mt-1 text-xs text-muted-foreground">完成前两步后，结果会显示在这里。</p>
          </div>
        ) : (
          <>
            <div className="rounded-xl border border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-card-strong),color-mix(in_oklch,var(--surface-tile)_55%,transparent))] px-5 py-5">
              <div className="flex flex-wrap items-center gap-2">
                <Badge variant={latestRunError ? "outline" : latestRunValue ? "default" : "secondary"}>
                  {latestRunError ? "失败" : latestRunValue ? "成功" : "等待执行"}
                </Badge>
                {latestTriggeredBy ? <Badge variant="outline">来源 {latestTriggeredBy}</Badge> : null}
                {latestAutoTyped ? <Badge variant="outline">已自动输入</Badge> : null}
              </div>
              <p className="mt-5 break-all font-mono text-4xl font-semibold tracking-[0.24em] text-foreground/92 sm:text-5xl sm:tracking-[0.36em]">
                {latestRunValue ?? "---"}
              </p>
              <p className="mt-3 text-xs text-muted-foreground">{latestRunError ?? "执行识别后会在这里显示最新三位结果。"}</p>
            </div>

            <div className="grid gap-3 xl:grid-cols-3">
              {runDetails.map((detail) => (
                <div key={detail.slot} className="rounded-lg border border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-tile),color-mix(in_oklch,var(--card)_40%,transparent))] p-4">
                  <div className="flex items-center justify-between gap-2">
                    <p className="text-xs font-medium text-foreground">{REGION_LABELS[detail.slot] ?? `位置 ${detail.slot + 1}`}</p>
                    <Badge variant={detail.error ? "outline" : detail.digit ? "default" : "secondary"}>
                      {detail.error ? "失败" : detail.digit ? detail.digit : "待机"}
                    </Badge>
                  </div>
                  <div className="mt-3 grid gap-2 text-xs text-muted-foreground">
                    <div className="rounded-lg border border-[var(--surface-border)] bg-[linear-gradient(145deg,color-mix(in_oklch,var(--card)_52%,transparent),var(--surface-tile))] px-3 py-2">
                      <p className="font-mono text-foreground/80">{detail.morse ?? "--"}</p>
                    </div>
                    <div className="rounded-lg border border-[var(--surface-border)] bg-[linear-gradient(145deg,color-mix(in_oklch,var(--card)_52%,transparent),var(--surface-tile))] px-3 py-2">
                      <p className="text-foreground/80">{detail.thresholdMode}</p>
                    </div>
                    <div className="rounded-lg border border-[var(--surface-border)] bg-[linear-gradient(145deg,color-mix(in_oklch,var(--card)_52%,transparent),var(--surface-tile))] px-3 py-2">
                      <p className="text-foreground/80">{detail.contourCount}</p>
                    </div>
                  </div>
                  {detail.error ? <p className="mt-3 text-xs text-destructive">识别失败</p> : null}
                </div>
              ))}
            </div>
          </>
        )}
      </CardBody>
    </TacticalCard>
  );
}

type HistoryPanelProps = {
  history: HistoryEntry[];
  isPreviewMode?: boolean;
};

export function HistoryPanel({ history, isPreviewMode }: HistoryPanelProps) {
  return (
    <TacticalCard>
      <SectionHeader
        eyebrow="Archive"
        icon={<RiHistoryLine />}
        title="历史记录"
        description="最近的识别记录。"
      />
      <CardBody>
        {isPreviewMode ? (
          <div className="rounded-xl border border-dashed border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-tile),color-mix(in_oklch,var(--card)_34%,transparent))] px-4 py-8 text-center">
            <RiEyeLine className="mx-auto mb-2 text-muted-foreground" />
            <p className="text-sm font-medium text-muted-foreground">预览模式</p>
            <p className="mt-1 text-xs text-muted-foreground">启动桌面程序以查看历史记录</p>
          </div>
        ) : (
          <>
            <ScrollArea className="h-72">
              <div className="flex flex-col gap-3 pe-4">
                {history.length === 0 ? (
                  <Empty className="border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-tile),color-mix(in_oklch,var(--card)_34%,transparent))]">
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
                    <div key={entry.id} className="rounded-lg border border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-card-strong),color-mix(in_oklch,var(--surface-tile)_45%,transparent))] p-4">
                      <div className="flex flex-wrap items-center justify-between gap-2">
                        <div className="flex flex-wrap items-center gap-2">
                          <p className="text-xs font-medium text-foreground">{entry.result ? `识别结果 ${entry.result}` : "识别失败"}</p>
                          <Badge variant={entry.success ? "default" : "outline"}>{entry.success ? "成功" : "失败"}</Badge>
                          <Badge variant="outline">{entry.triggeredBy}</Badge>
                          {entry.autoTyped ? <Badge variant="outline">已自动输入</Badge> : null}
                        </div>
                        <span className="text-xs text-muted-foreground">{formatTimestamp(entry.occurredAtMs)}</span>
                      </div>
                      <p className="mt-2 text-xs/relaxed text-muted-foreground">{entry.error ? "识别失败" : "识别流程已完成。"}</p>
                    </div>
                  ))
                )}
              </div>
            </ScrollArea>
          </>
        )}
      </CardBody>
    </TacticalCard>
  );
}

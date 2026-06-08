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
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { Field, FieldContent, FieldError, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { CardBody, ControlTile, InlineControl, SectionHeader, TacticalCard } from "@/components/app/app-ui";

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
        eyebrow="单元 01 / 采样阵列"
        icon={<RiLayoutGridLine />}
        title="框选采样窗位"
        description="先锁定三段信号窗位，再放行整条识别链路。"
        badge={<Badge variant={configuredCount === 3 ? "default" : "outline"}>{`窗位 ${configuredCount}/3`}</Badge>}
      />
      <CardBody className="flex flex-col gap-4 xl:gap-5">
        {isPreviewMode ? (
          <InlineControl className="border-2 border-dashed border-[var(--ink)] bg-[var(--bone)] px-4 py-8 text-center">
            <RiEyeLine className="mx-auto mb-2 text-muted-foreground" />
            <p className="text-sm font-medium text-muted-foreground">预览模式</p>
            <p className="mt-1 text-xs text-muted-foreground">启动桌面程序后才能写入采样窗位</p>
          </InlineControl>
        ) : (
          <>
            <ControlTile className="flex flex-wrap items-center justify-between gap-3">
              <div className="flex flex-wrap items-center gap-3">
                <div className="grid grid-cols-3 gap-1">
                  <div className="size-3 border border-[var(--ink)] bg-[var(--alert-red)]" style={{ opacity: configuredCount >= 1 ? 1 : 0.18 }} />
                  <div className="size-3 border border-[var(--ink)] bg-[var(--alert-red)]" style={{ opacity: configuredCount >= 2 ? 1 : 0.18 }} />
                  <div className="size-3 border border-[var(--ink)] bg-[var(--alert-red)]" style={{ opacity: configuredCount >= 3 ? 1 : 0.18 }} />
                </div>
                <div>
                  <p className="font-mono text-[0.68rem] font-black tracking-[0.2em] text-[var(--steel)] uppercase">采样主控</p>
                  <p className="mt-1 text-sm font-medium text-foreground">已锁定 {configuredCount}/3 个窗位</p>
                  <p className="mt-1 text-xs text-muted-foreground">推荐在同一轮完成三段框选，避免后续校准链路漂移。</p>
                </div>
              </div>
              <Button className="min-w-40 rounded-none" disabled={isBusy} onClick={onSelectAll} type="button" size="lg">
                <RiRefreshLine data-icon="inline-start" />
                一次框选三段窗位
              </Button>
            </ControlTile>

            <div className="grid gap-3 xl:grid-cols-3">
              {REGION_LABELS.map((label, index) => {
                const region = form?.regions[index] ?? null;
                const isConfigured = Boolean(region);
                const isSelecting = selectingSlot === index;

                return (
                  <ControlTile key={label} className="flex min-h-full flex-col gap-3">
                    <div className="flex items-start justify-between gap-3 border-b border-[var(--ink)] pb-3">
                      <div className="min-w-0">
                        <p className="font-mono text-[0.68rem] font-black tracking-[0.18em] text-[var(--steel)] uppercase">{`窗位 0${index + 1}`}</p>
                        <p className="mt-1 text-sm font-medium text-foreground">{label}</p>
                        <p className="mt-1 text-xs text-muted-foreground">
                          {isSelecting ? "当前轮次正在等待框选。" : isConfigured ? "坐标已写入，可直接放行识别。" : "尚未写入坐标，需先完成框选。"}
                        </p>
                      </div>
                      <Badge variant={isSelecting ? "secondary" : isConfigured ? "default" : "outline"}>
                        {isSelecting ? "框选中" : isConfigured ? "已锁定" : "待锁定"}
                      </Badge>
                    </div>

                    <InlineControl className="border-2 border-dashed border-[var(--ink)] bg-[var(--paper)] px-3 py-3">
                      <p className="font-mono text-[0.68rem] font-black tracking-[0.18em] text-[var(--steel)] uppercase">坐标纪要</p>
                      <p className="mt-2 overflow-hidden font-mono text-[0.6875rem] text-foreground/80 text-ellipsis whitespace-nowrap">{formatRegion(region)}</p>
                    </InlineControl>

                    <Button
                      className="mt-auto w-full rounded-none"
                      disabled={isBusy}
                      onClick={() => onSelectOne(index)}
                      type="button"
                      variant={isConfigured ? "outline" : "default"}
                    >
                      {isSelecting ? "等待当前框选完成" : isConfigured ? "重选本窗位" : "写入本窗位"}
                    </Button>
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
        eyebrow="单元 02 / 参数机架"
        icon={<RiSettings3Line />}
        title="校准识别链路"
        description="窗位锁定后，在这里写入阈值、热键与回填验证。"
        badge={<Badge variant={verificationStatus === "success" ? "default" : "outline"}>{verificationStatus === "running" ? "验证中" : "校准台"}</Badge>}
      />

      <CardBody className="xl:min-h-88">
        <div className="grid gap-4 xl:grid-cols-[minmax(0,1.2fr)_minmax(20rem,0.88fr)]">
          <section className="min-h-0 border-2 border-[var(--ink)] bg-[var(--bone)]">
            <div className="border-b-2 border-[var(--ink)] bg-[var(--ink)] px-3 py-2 text-[var(--paper)]">
              <div className="flex items-start gap-2">
                <RiSettings3Line className="mt-0.5 text-[var(--alert-red)]" />
                <div>
                  <h3 className="font-mono text-[0.68rem] font-black tracking-[0.22em] uppercase">字段机架</h3>
                  <p className="mt-1 font-mono text-[0.62rem] font-bold tracking-[0.08em] text-[var(--bone)] uppercase">热键、阈值与自动点击链路写入后即刻自动保存</p>
                </div>
              </div>
            </div>

            <div className="p-4">
              {form ? (
                <FieldGroup className="flex flex-1 flex-col gap-4 xl:min-h-full">
                  <Field className="xl:min-h-0">
                    <FieldLabel htmlFor="hotkey-recorder">热键</FieldLabel>
                    <FieldContent className="xl:flex xl:min-h-0 xl:flex-col xl:gap-2.2">
                      <Button
                        ref={hotkeyButtonRef}
                        className="h-auto w-full justify-between gap-4 rounded-none border-2 border-[var(--ink)] bg-[var(--paper)] px-3 py-3 font-mono text-[0.78rem] font-semibold text-[var(--ink)]"
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
                    <FieldContent className="xl:flex xl:min-h-0 xl:flex-col xl:gap-2.2">
                      <Input
                        className="rounded-none border-2 border-[var(--ink)] bg-[var(--paper)] font-mono"
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
                    <FieldContent className="xl:flex xl:min-h-0 xl:flex-col xl:gap-2.2">
                      <Input
                        className="rounded-none border-2 border-[var(--ink)] bg-[var(--paper)] font-mono"
                        id="auto-input-delay"
                        inputMode="numeric"
                        min="0"
                        onChange={(event) => onAutoInputDelayChange(event.currentTarget.value)}
                        value={form.autoInputDelay}
                      />
                    </FieldContent>
                  </Field>

                  <ControlTile className="flex items-center gap-3 bg-[var(--paper)]">
                    <Switch checked={autoClickEnabled} disabled={isBusy} onCheckedChange={onAutoClickEnabledChange} />
                    <div className="min-w-0">
                      <p className="font-mono text-[0.68rem] font-black tracking-[0.18em] text-[var(--steel)] uppercase">自动点击链路</p>
                      <p className="mt-1 text-sm font-medium text-foreground">识别成功后按设定顺序执行点击</p>
                      <p className="mt-1 text-xs text-muted-foreground">每个点击区域都保留单独延迟，供战局内细调节奏。</p>
                    </div>
                  </ControlTile>

                  {autoClickEnabled && (
                    <Collapsible defaultOpen={false} className="border-2 border-[var(--ink)] bg-[var(--paper)]">
                      <CollapsibleTrigger asChild>
                        <Button className="h-auto w-full justify-between rounded-none px-3 py-3 font-mono text-[0.72rem] font-black tracking-[0.18em]" type="button" variant="ghost">
                          点击区域配置
                          <Badge variant="outline">{clickRegions.filter((r) => r.rect).length}/7</Badge>
                        </Button>
                      </CollapsibleTrigger>
                      <CollapsibleContent className="border-t-2 border-[var(--ink)] px-3 py-3">
                        <FieldGroup className="gap-3">
                          <Field className="xl:min-h-0">
                            <FieldLabel htmlFor="after-click-hotkey">点击完成后按键</FieldLabel>
                            <FieldContent className="xl:flex xl:min-h-0 xl:flex-col xl:gap-2.2">
                              <Input
                                className="rounded-none border-2 border-[var(--ink)] bg-[var(--paper)] font-mono"
                                id="after-click-hotkey"
                                placeholder="留空不执行，例如 F4 或 Ctrl+F4"
                                value={form.afterClickHotkey}
                                onChange={(event) => onAfterClickHotkeyChange(event.currentTarget.value)}
                              />
                            </FieldContent>
                          </Field>
                          <Field className="xl:min-h-0">
                            <FieldLabel>点击区域（最多 7 个）</FieldLabel>
                            <FieldContent className="xl:flex xl:min-h-0 xl:flex-col xl:gap-2.2">
                              <div className="flex flex-col gap-2">
                                {clickRegionRows(clickRegions).map((cr) => (
                                  <InlineControl key={cr.slotIndex} className="flex items-center gap-3 border-2 border-[var(--ink)] bg-[var(--bone)]">
                                    <Badge variant={cr.rect ? "default" : "outline"} className="shrink-0">
                                      {cr.slotIndex + 1}
                                    </Badge>
                                    <span className="flex-1 font-mono text-xs text-muted-foreground">{formatRegion(cr.rect)}</span>
                                    <Input
                                      className="w-20 rounded-none border-2 border-[var(--ink)] bg-[var(--paper)] font-mono"
                                      inputMode="numeric"
                                      min="0"
                                      value={cr.delayMs}
                                      onChange={(event) => onUpdateClickRegionDelay(cr.slotIndex, event.currentTarget.value)}
                                    />
                                    <span className="text-xs text-muted-foreground">ms</span>
                                    <Button
                                      className="h-8 w-8 shrink-0 rounded-none px-0"
                                      disabled={isBusy}
                                      onClick={() => onRemoveClickRegion(cr.slotIndex)}
                                      type="button"
                                      variant="ghost"
                                    >
                                      ×
                                    </Button>
                                  </InlineControl>
                                ))}
                                {clickRegions.filter((r) => r.rect).length < 7 && (
                                  <Button className="rounded-none" disabled={isBusy} onClick={onAddClickRegion} type="button" variant="outline">
                                    <RiLayoutGridLine data-icon="inline-start" />
                                    添加点击区域
                                  </Button>
                                )}
                              </div>
                            </FieldContent>
                          </Field>
                        </FieldGroup>
                      </CollapsibleContent>
                    </Collapsible>
                  )}
                </FieldGroup>
              ) : (
                <div className="text-xs text-muted-foreground">正在加载设置...</div>
              )}
            </div>
          </section>

          <section className="min-h-0 border-2 border-[var(--ink)] bg-[var(--paper)]">
            <div className="border-b-2 border-[var(--ink)] bg-[var(--ink)] px-3 py-2 text-[var(--paper)]">
              <div className="flex items-start gap-2">
                <RiSparklingLine className="mt-0.5 text-[var(--alert-red)]" />
                <div>
                  <h3 className="font-mono text-[0.68rem] font-black tracking-[0.22em] uppercase">即时验证</h3>
                  <p className="mt-1 font-mono text-[0.62rem] font-bold tracking-[0.08em] text-[var(--bone)] uppercase">聚焦输入框或手动触发，执行一次仅识别回路</p>
                </div>
              </div>
            </div>

            <div className="p-4">
              <div className="flex items-start justify-between gap-3">
                <div className="min-w-0">
                  <p className="text-sm font-semibold text-foreground">验证回写</p>
                  <p className="mt-1 text-xs text-muted-foreground">用于快速核对三码输出是否符合当前阈值与窗位。</p>
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

              <InlineControl className="mt-4 border-2 border-[var(--ink)] bg-[var(--bone)] p-4">
                <p className="font-mono text-[0.68rem] font-black tracking-[0.18em] text-[var(--steel)] uppercase">即时验证输入</p>
                <Input
                  className="mt-3 h-12 rounded-none border-2 border-[var(--ink)] bg-[var(--paper)] px-4 font-mono text-base tracking-[0.22em]"
                  id="verification-input"
                  onChange={(event) => onVerificationChange(event.currentTarget.value)}
                  onFocus={onVerificationFocus}
                  placeholder="聚焦此处立即执行测试验证"
                  value={verificationValue}
                />
                <p className="mt-3 text-xs text-muted-foreground">{verificationMessage}</p>
              </InlineControl>

              <div className="mt-4 flex flex-wrap items-center justify-between gap-3">
                <InlineControl className="border-2 border-[var(--ink)] bg-[var(--bone)] px-3 py-3">
                  <p className="font-mono text-[0.68rem] font-black tracking-[0.18em] text-[var(--steel)] uppercase">验证状态</p>
                  <p className="mt-1 text-sm text-foreground">{isVerifying ? "正在执行仅识别流程..." : "聚焦输入框或按按钮即可重新验证"}</p>
                </InlineControl>
                <Button className="rounded-none" disabled={isVerifying} onClick={onVerificationRetry} type="button" variant="outline">
                  <RiRefreshLine data-icon="inline-start" />
                  重新验证
                </Button>
              </div>
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
        eyebrow="单元 03 / 报码输出"
        icon={<RiCheckboxCircleLine />}
        title="审阅三码结果"
        description="校准链路完成后，在这里放大查看最新报码与逐段细节。"
        badge={<Badge variant={latestRunError ? "outline" : latestRunValue ? "default" : "secondary"}>{latestRunError ? "报码失败" : latestRunValue ? "报码完成" : "待报码"}</Badge>}
      />
      <CardBody className="flex min-h-0 flex-col gap-4">
        {!hasResult ? (
          <InlineControl className="border-2 border-dashed border-[var(--ink)] bg-[var(--bone)] px-4 py-8 text-center">
            <RiCheckboxCircleLine className="mx-auto mb-2 text-muted-foreground" />
            <p className="text-sm font-medium text-muted-foreground">等待报码</p>
            <p className="mt-1 text-xs text-muted-foreground">完成前两单元后，三码结果会写入这里。</p>
          </InlineControl>
        ) : (
          <>
            <InlineControl className="border-2 border-[var(--ink)] bg-[var(--data-well)] px-5 py-5 text-[var(--paper)]">
              <div className="flex flex-wrap items-center gap-2">
                <Badge variant={latestRunError ? "outline" : latestRunValue ? "default" : "secondary"}>
                  {latestRunError ? "失败" : latestRunValue ? "成功" : "等待执行"}
                </Badge>
                {latestTriggeredBy ? <Badge variant="outline">来源 {latestTriggeredBy}</Badge> : null}
                {latestAutoTyped ? <Badge variant="outline">已自动输入</Badge> : null}
              </div>
              <p className="mt-4 font-mono text-[0.68rem] font-black tracking-[0.18em] text-[var(--bone)] uppercase">最新三码输出</p>
              <p className="mt-4 break-all font-mono text-4xl font-semibold tracking-[0.24em] text-[var(--paper)] sm:text-5xl sm:tracking-[0.36em]">
                {latestRunValue ?? "---"}
              </p>
              <p className="mt-3 text-xs text-[var(--bone)]">{latestRunError ?? "执行识别后会在这里显示最新三码输出。"}</p>
            </InlineControl>

            <div className="grid gap-2 md:grid-cols-3">
              {runDetails.map((detail) => (
                <InlineControl key={detail.slot} className="border-2 border-[var(--ink)] bg-[var(--bone)] p-3">
                  <div className="flex items-start justify-between gap-2">
                    <div className="min-w-0">
                      <p className="font-mono text-[0.68rem] font-black tracking-[0.18em] text-[var(--steel)] uppercase">{REGION_LABELS[detail.slot] ?? `位置 ${detail.slot + 1}`}</p>
                      <p className="mt-1 font-mono text-sm font-semibold text-foreground">{detail.morse ?? "--"}</p>
                    </div>
                    <Badge variant={detail.error ? "outline" : detail.digit ? "default" : "secondary"}>{detail.error ? "失败" : detail.digit ? detail.digit : "待机"}</Badge>
                  </div>
                  <div className="mt-3 flex flex-wrap gap-2 text-xs text-muted-foreground">
                    <span className="font-mono text-foreground/80">报码 {detail.digit ?? "--"}</span>
                    <span>{detail.thresholdMode}</span>
                    <span>轮廓 {detail.contourCount}</span>
                  </div>
                </InlineControl>
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
        eyebrow="单元 04 / 运行档案"
        icon={<RiHistoryLine />}
        title="回看识别历史"
        description="保留最近识别回执、触发来源与异常记录。"
      />
      <CardBody>
        {isPreviewMode ? (
          <InlineControl className="border-2 border-dashed border-[var(--ink)] bg-[var(--bone)] px-4 py-8 text-center">
            <RiEyeLine className="mx-auto mb-2 text-muted-foreground" />
            <p className="text-sm font-medium text-muted-foreground">预览模式</p>
            <p className="mt-1 text-xs text-muted-foreground">启动桌面程序后才能读取识别档案</p>
          </InlineControl>
        ) : (
          <ScrollArea className="h-72">
            <div className="flex flex-col gap-3 pe-4">
              {history.length === 0 ? (
                <InlineControl className="border-2 border-dashed border-[var(--ink)] bg-[var(--bone)] px-4 py-8 text-center">
                  <RiHistoryLine className="mx-auto mb-2 text-muted-foreground" />
                  <p className="text-sm font-medium text-muted-foreground">暂无档案</p>
                  <p className="mt-1 text-xs text-muted-foreground">执行一次识别后会在这里生成运行回执。</p>
                </InlineControl>
              ) : (
                history.map((entry) => (
                  <InlineControl key={entry.id} className="border-2 border-[var(--ink)] bg-[var(--bone)] p-4">
                    <div className="flex flex-wrap items-center justify-between gap-2 border-b border-[var(--ink)] pb-3">
                      <div className="flex flex-wrap items-center gap-2">
                        <p className="font-mono text-[0.68rem] font-black tracking-[0.18em] text-[var(--steel)] uppercase">{entry.result ? `报码 ${entry.result}` : "识别失败"}</p>
                        <Badge variant={entry.success ? "default" : "outline"}>{entry.success ? "成功" : "失败"}</Badge>
                        <Badge variant="outline">{entry.triggeredBy}</Badge>
                        {entry.autoTyped ? <Badge variant="outline">已自动输入</Badge> : null}
                      </div>
                      <span className="text-xs text-muted-foreground">{formatTimestamp(entry.occurredAtMs)}</span>
                    </div>
                    <p className="mt-3 text-xs/relaxed text-muted-foreground">{entry.error ? "本轮识别失败，建议回查窗位与阈值。" : "识别链路执行完成，结果已写入历史档案。"}</p>
                  </InlineControl>
                ))
              )}
            </div>
          </ScrollArea>
        )}
      </CardBody>
    </TacticalCard>
  );
}

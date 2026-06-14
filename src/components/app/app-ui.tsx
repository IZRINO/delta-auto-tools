import type { ComponentProps, ReactNode } from "react";
import {
  RiAddLine,
  RiArrowDownSLine,
  RiDeleteBinLine,
  RiErrorWarningLine,
  RiInformationLine,
  RiMapPinLine,
} from "@remixicon/react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Field, FieldContent, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Slider } from "@/components/ui/slider";
import { Switch } from "@/components/ui/switch";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";

/* ────────── App Page Grid ────────── */

type AppPageProps = {
  children: ReactNode;
  className?: string;
};

export function AppPage({ children, className }: AppPageProps) {
  return (
    <div className={cn("grid min-h-0 flex-1 grid-cols-12 gap-2 font-sans", className)}>
      {children}
    </div>
  );
}

/* ────────── Macro Header ────────── */

type MacroHeaderProps = {
  code: string;
  title: string;
  subtitle: string;
  verticalLabel?: string;
  badges?: ReactNode;
  actions?: ReactNode;
  className?: string;
};

export function MacroHeader({ actions, badges, className, code, subtitle, title, verticalLabel }: MacroHeaderProps) {
  return (
    <section
      className={cn(
        "relative col-span-12 overflow-hidden border-2 border-[var(--chalk)] bg-[var(--carbon)] text-[var(--chalk)]",
        className,
      )}
    >
      <div className="relative grid gap-px bg-[var(--chalk)] lg:grid-cols-[auto_minmax(0,1fr)_auto]">
        <div className="hidden shrink-0 border-r-2 border-[var(--chalk)] bg-[var(--chalk)] lg:flex lg:items-center lg:justify-center lg:px-4 lg:py-2">
          <span className="font-heading text-[clamp(2rem,3vw,4rem)] font-black leading-[0.82] tracking-[-0.04em] text-[var(--carbon)] uppercase [writing-mode:vertical-rl]">
            {verticalLabel ?? title.replace(/^\s*(\S+).*/, "$1")}
          </span>
        </div>
        <div className="min-w-0 bg-[var(--carbon)] px-3 py-3 sm:px-4">
          <div className="flex flex-wrap items-center gap-2">
            <p className="font-mono text-[0.6rem] font-black tracking-[0.22em] text-[var(--amber)] uppercase">
              [ {code} ]
            </p>
            {badges}
          </div>
          <div className="mt-2 flex min-w-0 flex-wrap items-end gap-x-4 gap-y-2">
            <h1 className="max-w-4xl text-balance font-heading text-[clamp(2rem,4vw,4.5rem)] font-black leading-[0.85] tracking-[-0.06em] text-[var(--chalk)] uppercase">
              {title}
            </h1>
            <p className="max-w-[64ch] border-l-4 border-[var(--amber)] pl-3 font-mono text-[0.68rem] font-bold leading-relaxed tracking-[0.06em] text-[var(--zinc)] uppercase">
              {subtitle}
            </p>
          </div>
        </div>
        {actions ? (
          <aside className="grid min-w-0 bg-[var(--carbon)] lg:min-w-72 lg:max-w-[26rem]">
            <div className="flex flex-wrap items-center justify-end gap-2 border-b-2 border-[var(--chalk)] px-3 py-2">{actions}</div>
          </aside>
        ) : null}
      </div>
    </section>
  );
}

/* ────────── Page Hero (legacy alias, maps to MacroHeader) ────────── */

type PageHeroProps = {
  eyebrow: string;
  title: string;
  description: string;
  badges?: ReactNode;
  actions?: ReactNode;
  stats?: ReactNode;
  className?: string;
};

export function PageHero({ actions, badges, className, description, eyebrow, stats, title }: PageHeroProps) {
  return (
    <section
      className={cn(
        "relative col-span-12 overflow-hidden border-2 border-[var(--chalk)] bg-[var(--carbon)] text-[var(--chalk)]",
        className,
      )}
    >
      <div className="relative grid gap-px bg-[var(--chalk)] lg:grid-cols-[auto_minmax(0,1fr)_auto]">
        <div className="hidden shrink-0 border-r-2 border-[var(--chalk)] bg-[var(--chalk)] lg:flex lg:items-center lg:justify-center lg:px-4 lg:py-2">
          <span className="font-heading text-[clamp(2rem,3vw,4rem)] font-black leading-[0.82] tracking-[-0.04em] text-[var(--carbon)] uppercase [writing-mode:vertical-rl]">
            {eyebrow.replace(/^\s*(\S+).*/, "$1")}
          </span>
        </div>
        <div className="min-w-0 bg-[var(--carbon)] px-3 py-3 sm:px-4">
          <div className="flex flex-wrap items-center gap-2">
            <p className="font-mono text-[0.6rem] font-black tracking-[0.22em] text-[var(--amber)] uppercase">
              [ {eyebrow} ]
            </p>
            {badges}
          </div>
          <div className="mt-2 flex min-w-0 flex-wrap items-end gap-x-4 gap-y-2">
            <h1 className="max-w-4xl text-balance font-heading text-[clamp(2rem,4vw,4.5rem)] font-black leading-[0.85] tracking-[-0.06em] text-[var(--chalk)] uppercase">
              {title}
            </h1>
            <p className="max-w-[64ch] border-l-4 border-[var(--amber)] pl-3 font-mono text-[0.68rem] font-bold leading-relaxed tracking-[0.06em] text-[var(--zinc)] uppercase">
              {description}
            </p>
          </div>
        </div>
        {(actions || stats) ? (
          <aside className="grid min-w-0 bg-[var(--carbon)] lg:min-w-72 lg:max-w-[26rem]">
            {actions ? <div className="flex flex-wrap items-center justify-end gap-2 border-b-2 border-[var(--chalk)] px-3 py-2">{actions}</div> : null}
            {stats ? <div className="grid gap-px bg-[var(--chalk)] sm:grid-cols-3 lg:grid-cols-1">{stats}</div> : null}
          </aside>
        ) : null}
      </div>
    </section>
  );
}

/* ────────── Status Matrix ────────── */

type StatusMatrixItem = {
  id: string;
  state: "idle" | "active" | "valid" | "warning" | "error";
  label?: string;
};

type StatusMatrixProps = {
  items: StatusMatrixItem[];
  className?: string;
};

export function StatusMatrix({ items, className }: StatusMatrixProps) {
  const stateColor: Record<string, string> = {
    idle: "bg-[var(--zinc)]",
    active: "bg-[var(--amber)]",
    valid: "bg-[var(--valid-green)]",
    warning: "bg-[var(--warning-amber)]",
    error: "bg-[var(--alert-red)]",
  };
  return (
    <div className={cn("flex flex-wrap gap-px border border-[var(--seam)] bg-[var(--seam)]", className)}>
      {items.map((item) => (
        <div
          key={item.id}
          data-state={item.state}
          className="relative flex items-center gap-2 bg-[var(--carbon)] px-2 py-1"
          title={item.label}
        >
          <span className={cn("size-1.5", stateColor[item.state])} />
          <span className="whitespace-nowrap font-mono text-[0.58rem] font-bold tracking-[0.12em] text-[var(--chalk)] uppercase">
            {item.label ?? item.id}
          </span>
        </div>
      ))}
    </div>
  );
}

/* ────────── Signal Tile (legacy) ────────── */

type SignalTileProps = {
  label: string;
  value: ReactNode;
  detail?: ReactNode;
  icon?: ReactNode;
  className?: string;
};

export function SignalTile({ className, detail, icon, label, value }: SignalTileProps) {
  return (
    <div className={cn("flex min-h-12 items-center justify-between gap-3 bg-[var(--carbon)] px-3 py-2 text-[var(--chalk)]", className)}>
      <div className="min-w-0">
        <p className="truncate font-mono text-[0.56rem] font-black tracking-[0.18em] text-[var(--zinc)] uppercase">{label}</p>
        {detail ? <div className="mt-0.5 truncate font-mono text-[0.58rem] font-bold tracking-[0.06em] text-[var(--zinc)] uppercase">{detail}</div> : null}
      </div>
      <div className="flex shrink-0 items-center gap-2 font-mono text-lg font-black leading-none tracking-[-0.04em] text-[var(--chalk)] tabular-nums">
        {icon ? <div className="text-[var(--amber)]">{icon}</div> : null}
        {value}
      </div>
    </div>
  );
}

/* ────────── Tactical Card (legacy) ────────── */

type TacticalCardProps = ComponentProps<typeof Card> & {
  active?: boolean;
};

export function TacticalCard({ active, children, className, size = "sm", ...props }: TacticalCardProps) {
  return (
    <Card
      size={size}
      className={cn(
        "border-2 border-[var(--chalk)] bg-[var(--carbon)] text-[var(--chalk)] shadow-none transition-[background-color,outline-color]",
        active && "bg-[var(--slate)] outline-4 outline-[var(--amber)]",
        className,
      )}
      {...props}
    >
      {children}
    </Card>
  );
}

/* ────────── Section Header (legacy) ────────── */

type SectionHeaderProps = {
  title: ReactNode;
  description?: ReactNode;
  eyebrow?: string;
  icon?: ReactNode;
  badge?: ReactNode;
  actions?: ReactNode;
  className?: string;
};

export function SectionHeader({ actions, badge, className, description, eyebrow, icon, title }: SectionHeaderProps) {
  return (
    <CardHeader className={cn("border-b-2 border-[var(--chalk)] bg-[var(--chalk)] text-[var(--carbon)]", className)}>
      <div className="flex min-w-0 items-start justify-between gap-4">
        <div className="flex min-w-0 items-start gap-3">
          {icon ? (
            <div className="flex size-9 shrink-0 items-center justify-center border border-[var(--carbon)] bg-[var(--amber)] text-[var(--carbon)]">
              {icon}
            </div>
          ) : null}
          <div className="min-w-0">
            {eyebrow ? <p className="font-mono text-[0.6rem] font-black tracking-[0.24em] text-[var(--slate)] uppercase">[ {eyebrow} ]</p> : null}
            <CardTitle className="mt-0.5 truncate text-[var(--carbon)] uppercase">{title}</CardTitle>
            {description ? <CardDescription className="mt-1 truncate font-mono text-[0.68rem] font-bold tracking-[0.08em] text-[var(--slate)] uppercase">{description}</CardDescription> : null}
          </div>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          {actions}
          {badge ? <div className="shrink-0">{badge}</div> : null}
        </div>
      </div>
    </CardHeader>
  );
}

/* ────────── Control Tile / Inline Control (legacy) ────────── */

type ControlTileProps = {
  children: ReactNode;
  className?: string;
};

export function ControlTile({ children, className }: ControlTileProps) {
  return <div className={cn("border-2 border-[var(--chalk)] bg-[var(--slate)] p-3", className)}>{children}</div>;
}

export function InlineControl({ children, className }: ControlTileProps) {
  return <div className={cn("border border-[var(--chalk)] bg-[var(--carbon)] p-3", className)}>{children}</div>;
}

export function SaveStateBadge({ dirty, saving }: { dirty: boolean; saving: boolean }) {
  if (saving) return <Badge variant="outline">保存中</Badge>;
  if (dirty) return <Badge variant="outline">待保存</Badge>;
  return <Badge variant="secondary">已保存</Badge>;
}

export function CardBody({ children, className }: { children: ReactNode; className?: string }) {
  return <CardContent className={cn("pt-4", className)}>{children}</CardContent>;
}

/* ────────── New Industrial Components ────────── */

/* Channel Tabs */

type ChannelTab = {
  id: string;
  label: string;
  active?: boolean;
};

type ChannelTabsProps = {
  tabs: ChannelTab[];
  onTabChange: (id: string) => void;
  className?: string;
};

export function ChannelTabs({ tabs, onTabChange, className }: ChannelTabsProps) {
  return (
    <div className={cn("flex border-b-2 border-[var(--chalk)] bg-[var(--carbon)]", className)}>
      {tabs.map((tab) => (
        <button
          key={tab.id}
          className={cn(
            "px-4 py-2 font-mono text-xs font-black tracking-[0.12em] uppercase transition-colors focus:outline-none focus-visible:outline-2 focus-visible:outline-offset-[-2px] focus-visible:outline-[var(--amber)]",
            tab.active
              ? "border-b-2 border-[var(--amber)] bg-[var(--chalk)] text-[var(--carbon)]"
              : "border-b-2 border-transparent text-[var(--zinc)] hover:bg-[var(--slate)] hover:text-[var(--chalk)]",
          )}
          onClick={() => onTabChange(tab.id)}
          type="button"
        >
          {tab.label}
        </button>
      ))}
    </div>
  );
}

/* Config Row */

type ConfigRowProps = {
  label: string;
  value: ReactNode;
  unit?: string;
  state?: "idle" | "active" | "valid" | "warning" | "error";
  className?: string;
};

export function ConfigRow({ label, value, unit, state = "idle", className }: ConfigRowProps) {
  const stateColor: Record<string, string> = {
    idle: "bg-[var(--zinc)]",
    active: "bg-[var(--amber)]",
    valid: "bg-[var(--valid-green)]",
    warning: "bg-[var(--warning-amber)]",
    error: "bg-[var(--alert-red)]",
  };
  return (
    <div
      data-state={state}
      className={cn(
        "grid grid-cols-[max-content_1fr_max-content_max-content] items-center gap-x-3 border-b border-[var(--seam)] px-3 py-2 text-[var(--chalk)]",
        className,
      )}
    >
      <span className="font-mono text-[0.62rem] font-black tracking-[0.18em] text-[var(--zinc)] uppercase">{label}</span>
      <span className="min-w-0 truncate text-right font-mono text-sm font-bold tracking-[-0.02em] tabular-nums">{value}</span>
      {unit ? <span className="font-mono text-[0.6rem] font-bold tracking-[0.1em] text-[var(--zinc)] uppercase">{unit}</span> : <span />}
      <span className={cn("size-2", stateColor[state])} />
    </div>
  );
}

/* Help Hint (circle !) */

type HelpHintProps = {
  content: ReactNode;
  className?: string;
};

export function HelpHint({ content, className }: HelpHintProps) {
  return (
    <Tooltip delayDuration={200}>
      <TooltipTrigger asChild>
        <button
          className={cn(
            "inline-flex size-4 items-center justify-center rounded-full border border-[var(--zinc)] text-[var(--zinc)] hover:border-[var(--amber)] hover:text-[var(--amber)] focus:outline-none focus-visible:outline-2 focus-visible:outline-[var(--amber)]",
            className,
          )}
          type="button"
        >
          <RiInformationLine className="size-3" />
        </button>
      </TooltipTrigger>
      <TooltipContent className="max-w-xs border border-[var(--chalk)] bg-[var(--carbon)] px-3 py-2 font-mono text-xs text-[var(--chalk)]">
        {content}
      </TooltipContent>
    </Tooltip>
  );
}

/* Error Hint (square !) */

type ErrorHintProps = {
  content: ReactNode;
  className?: string;
};

export function ErrorHint({ content, className }: ErrorHintProps) {
  return (
    <Tooltip delayDuration={0}>
      <TooltipTrigger asChild>
        <button
          className={cn(
            "inline-flex size-4 items-center justify-center border border-[var(--alert-red)] text-[var(--alert-red)] hover:bg-[var(--alert-red)] hover:text-[var(--carbon)] focus:outline-none focus-visible:outline-2 focus-visible:outline-[var(--alert-red)]",
            className,
          )}
          type="button"
        >
          <RiErrorWarningLine className="size-3" />
        </button>
      </TooltipTrigger>
      <TooltipContent className="max-w-xs border border-[var(--alert-red)] bg-[var(--carbon)] px-3 py-2 font-mono text-xs text-[var(--alert-red)]">
        {content}
      </TooltipContent>
    </Tooltip>
  );
}

/* Data Well */

type DataWellProps = {
  children: ReactNode;
  className?: string;
  maxHeight?: string;
};

export function DataWell({ children, className, maxHeight = "max-h-64" }: DataWellProps) {
  return (
    <div className={cn("overflow-auto border border-[var(--chalk)] bg-[var(--slate)] p-3 font-mono text-xs font-bold leading-relaxed text-[var(--chalk)]", maxHeight, className)}>
      {children}
    </div>
  );
}

/* Field Unit */

type FieldUnitProps = {
  children: ReactNode;
  className?: string;
  header?: ReactNode;
  footer?: ReactNode;
};

export function FieldUnit({ children, className, header, footer }: FieldUnitProps) {
  return (
    <div className={cn("border-2 border-[var(--chalk)] bg-[var(--carbon)]", className)}>
      {header ? <div className="border-b-2 border-[var(--chalk)] bg-[var(--chalk)] px-3 py-2 font-mono text-[0.6rem] font-black tracking-[0.24em] text-[var(--carbon)] uppercase">{header}</div> : null}
      <div className="p-3">{children}</div>
      {footer ? <div className="border-t border-[var(--seam)] px-3 py-2">{footer}</div> : null}
    </div>
  );
}

/* Empty State */

type EmptyStateProps = {
  icon?: ReactNode;
  title: string;
  description: ReactNode;
  action?: ReactNode;
  className?: string;
};

export function EmptyState({ action, className, description, icon, title }: EmptyStateProps) {
  return (
    <div className={cn("flex flex-col items-center justify-center border-2 border-dashed border-[var(--chalk)] bg-[var(--slate)] px-6 py-12 text-center", className)}>
      {icon ? <div className="mb-4 flex size-12 items-center justify-center border-2 border-[var(--chalk)] bg-[var(--carbon)] text-[var(--amber)]">{icon}</div> : null}
      <h3 className="font-heading text-lg font-black uppercase tracking-[-0.02em] text-[var(--chalk)]">{title}</h3>
      <p className="mt-2 max-w-[48ch] font-mono text-xs font-bold tracking-[0.06em] text-[var(--zinc)] uppercase">{description}</p>
      {action ? <div className="mt-6">{action}</div> : null}
    </div>
  );
}

/* Macro Number */

type MacroNumberProps = {
  value: ReactNode;
  label?: string;
  unit?: string;
  className?: string;
};

export function MacroNumber({ className, label, unit, value }: MacroNumberProps) {
  return (
    <div className={cn("flex flex-col items-start gap-1", className)}>
      {label ? (
        <span className="font-mono text-[0.6rem] font-black tracking-[0.22em] text-[var(--zinc)] uppercase">{label}</span>
      ) : null}
      <div className="flex items-baseline gap-2">
        <span className="font-heading text-[clamp(3rem,10vw,9rem)] font-black leading-[0.82] tracking-[-0.06em] text-[var(--chalk)] uppercase">
          {value}
        </span>
        {unit ? (
          <span className="font-mono text-[0.75rem] font-bold tracking-[0.1em] text-[var(--zinc)] uppercase">{unit}</span>
        ) : null}
      </div>
    </div>
  );
}

/* Drag Button */

type DragButtonProps = {
  controlsDisabled: boolean;
  onDragStart: () => void;
};

export function DragButton({ controlsDisabled, onDragStart }: DragButtonProps) {
  return (
    <Button aria-label="拖动排序" className="cursor-grab active:cursor-grabbing" disabled={controlsDisabled} onPointerDown={(event) => { event.preventDefault(); onDragStart(); }} size="icon-sm" type="button" variant="ghost">
      <span className="text-xs font-bold">↕</span>
    </Button>
  );
}

/* Hotkey Field */

type HotkeyFieldProps = {
  controlsDisabled: boolean;
  hotkey: string;
  id: string;
  isRecording: boolean;
  onBeginHotkeyRecording: () => void;
  onHotkeyKeyDown: (event: React.KeyboardEvent<HTMLButtonElement>) => void;
  onHotkeyRecorderBlur: () => void;
};

export function HotkeyField({ controlsDisabled, hotkey, id, isRecording, onBeginHotkeyRecording, onHotkeyKeyDown, onHotkeyRecorderBlur }: HotkeyFieldProps) {
  return (
    <Field>
      <FieldLabel htmlFor={id}>快捷键</FieldLabel>
      <FieldContent>
        <Button className="h-9 w-full justify-between gap-4 px-3 font-mono" disabled={controlsDisabled} id={id} onBlur={onHotkeyRecorderBlur} onClick={onBeginHotkeyRecording} onKeyDown={onHotkeyKeyDown} type="button" variant="outline">
          <span>{isRecording ? "录制中..." : hotkey || "未设置"}</span>
          <span className="text-[0.6875rem] text-muted-foreground">{isRecording ? "失焦取消" : "点击录制"}</span>
        </Button>
      </FieldContent>
    </Field>
  );
}

/* Display Settings Inline (legacy) */

type DisplaySettingsInlineProps = {
  canDelete: boolean;
  controlsDisabled: boolean;
  display: { fontOpacity?: string; rect?: { width?: number } } | undefined;
  group: { enabled: boolean; name: string };
  statusMessage: string;
  targetLabel: string;
  onGroupDelete: () => void;
  onGroupUpdate: (value: Partial<{ enabled: boolean; name: string }>) => void;
  onPositionSelection: () => void;
  onUpdate: (value: Partial<{ fontOpacity?: string }>) => void;
  onUpdateRect: (value: Partial<{ width?: number }>) => void;
};

export function DisplaySettingsInline({
  canDelete,
  controlsDisabled,
  display,
  group,
  statusMessage,
  targetLabel,
  onGroupDelete,
  onGroupUpdate,
  onPositionSelection,
  onUpdate,
  onUpdateRect,
}: DisplaySettingsInlineProps) {
  return (
    <ControlTile className="flex flex-col gap-3 bg-[var(--carbon)]">
      <div className="flex flex-wrap items-center gap-3">
        <Switch checked={group.enabled} disabled={controlsDisabled} onCheckedChange={(checked) => onGroupUpdate({ enabled: checked })} />
        <p className="font-mono text-xs font-medium tracking-[0.12em] text-[var(--chalk)] uppercase">
          {targetLabel}分组 · {group.name}
        </p>
        <Input
          className="w-28 font-mono text-sm"
          disabled={controlsDisabled}
          value={group.name}
          onChange={(event) => onGroupUpdate({ name: event.currentTarget.value })}
          aria-label="分组名称"
        />
        <Button disabled={!canDelete} onClick={onGroupDelete} type="button" variant="ghost" className="shrink-0" size="icon-sm">
          <RiDeleteBinLine />
        </Button>
        <Button className="shrink-0" disabled={controlsDisabled} onClick={onPositionSelection} type="button" variant="outline" size="sm">
          <RiMapPinLine data-icon="inline-start" />
          位置
        </Button>
      </div>

      <Collapsible defaultOpen={false}>
        <InlineControl className="p-0">
          <CollapsibleTrigger asChild>
            <Button className="w-full justify-between px-2 py-1.5 font-mono text-xs font-medium tracking-[0.12em] uppercase" type="button" variant="ghost">
              显示参数
              <RiArrowDownSLine className="size-3.5" />
            </Button>
          </CollapsibleTrigger>
          <CollapsibleContent className="border-t border-[var(--chalk)] px-2 py-2">
            <div className="flex flex-wrap items-center gap-4">
              <Field className="min-w-0 flex-1">
                <FieldLabel className="font-mono text-xs">字体透明度</FieldLabel>
                <FieldContent>
                  <div className="flex items-center gap-3">
                    <Slider disabled={controlsDisabled || !display} min={0.1} max={1} step={0.05} value={[Number.parseFloat(display?.fontOpacity ?? "0.9")]} onValueChange={([value]) => onUpdate({ fontOpacity: value.toFixed(2) })} />
                    <span className="w-10 text-right font-mono text-xs text-muted-foreground">{display?.fontOpacity ?? "--"}</span>
                  </div>
                </FieldContent>
              </Field>
              <Field className="w-36 shrink-0">
                <FieldLabel className="font-mono text-xs">窗口宽度</FieldLabel>
                <FieldContent>
                  <Input disabled={controlsDisabled || !display} inputMode="numeric" min="320" className="h-7 font-mono text-xs" value={display?.rect?.width ?? 320} onChange={(event) => onUpdateRect({ width: Number.parseInt(event.currentTarget.value, 10) || 320 })} />
                </FieldContent>
              </Field>
            </div>
          </CollapsibleContent>
        </InlineControl>
      </Collapsible>

      <p className="font-mono text-xs font-medium tracking-[0.08em] text-muted-foreground uppercase">{statusMessage}</p>
    </ControlTile>
  );
}

/* Page Preview Banner (legacy) */

export function PagePreviewBanner({ className }: { className?: string }) {
  return (
    <Alert
      variant="destructive"
      className={cn("border-2 border-[var(--warning-amber)] bg-[var(--carbon)] text-[var(--chalk)]", className)}
    >
      <AlertTitle className="font-mono text-xs font-black tracking-[0.14em] text-[var(--warning-amber)] uppercase">
        [ 浏览器预览模式 ]
      </AlertTitle>
      <AlertDescription className="font-mono text-xs font-bold tracking-[0.06em] text-[var(--zinc)] uppercase">
        当前处于浏览器预览模式，所有 Tauri 原生命令已被禁用。如需使用完整功能，请在桌面端运行。
      </AlertDescription>
    </Alert>
  );
}

/* Tactical Empty State (legacy alias) */

type TacticalEmptyStateProps = {
  icon?: ReactNode;
  title: string;
  description: ReactNode;
  className?: string;
  children?: ReactNode;
};

export function TacticalEmptyState({ children, className, description, icon, title }: TacticalEmptyStateProps) {
  return (
    <TacticalCard className={cn("min-h-48", className)}>
      <CardBody className="flex h-full items-center justify-center">
        <Empty className="min-h-40 border-2 border-dashed border-[var(--chalk)] bg-[var(--slate)] px-4 py-8 text-center">
          {icon ? <EmptyMedia variant="icon">{icon}</EmptyMedia> : null}
          <EmptyHeader>
            <EmptyTitle>{title}</EmptyTitle>
            <EmptyDescription>{description}</EmptyDescription>
          </EmptyHeader>
          {children}
        </Empty>
      </CardBody>
    </TacticalCard>
  );
}

/* Add Card Button (legacy) */

type AddCardButtonProps = {
  disabled?: boolean;
  title: string;
  description: ReactNode;
  className?: string;
  onClick: () => void;
};

export function AddCardButton({ className, description, disabled, onClick, title }: AddCardButtonProps) {
  return (
    <button
      className={cn(
        "group flex min-h-32 flex-col items-center justify-center border-2 border-dashed border-[var(--chalk)] bg-[var(--slate)] p-4 text-center transition-colors hover:bg-[var(--carbon)] focus:outline-none focus-visible:outline-2 focus-visible:outline-offset-[-2px] focus-visible:outline-[var(--amber)] active:bg-[var(--chalk)] active:text-[var(--carbon)] disabled:cursor-not-allowed disabled:opacity-50",
        className,
      )}
      disabled={disabled}
      onClick={onClick}
      type="button"
    >
      <span className="mb-4 flex size-11 items-center justify-center border-2 border-[var(--chalk)] bg-[var(--amber)] text-[var(--carbon)]">
        <RiAddLine />
      </span>
      <span className="text-sm font-black uppercase text-[var(--chalk)] group-active:text-[var(--carbon)]">{title}</span>
      <span className="mt-1 max-w-56 font-mono text-[0.68rem] font-bold leading-relaxed tracking-[0.08em] text-[var(--zinc)] uppercase group-active:text-[var(--slate)]">{description}</span>
    </button>
  );
}

/* Json Pre Block (legacy) */

type JsonPreBlockProps = {
  data: unknown;
  className?: string;
  maxHeightClassName?: string;
};

export function JsonPreBlock({ className, data, maxHeightClassName = "max-h-64" }: JsonPreBlockProps) {
  return (
    <pre
      className={cn(
        maxHeightClassName,
        "overflow-auto border-2 border-[var(--chalk)] bg-[var(--void)] p-3 font-mono text-xs font-bold leading-relaxed text-[var(--carbon)]",
        className,
      )}
    >
      {JSON.stringify(data, null, 2)}
    </pre>
  );
}

/* Inline Notice (legacy) */

type InlineNoticeProps = {
  title?: string;
  children: ReactNode;
  className?: string;
};

export function InlineNotice({ children, className, title }: InlineNoticeProps) {
  return (
    <Alert variant="destructive" className={cn("border-2 border-[var(--amber)] bg-[var(--carbon)] text-[var(--chalk)]", className)}>
      {title ? <AlertTitle>{title}</AlertTitle> : null}
      <AlertDescription>{children}</AlertDescription>
    </Alert>
  );
}

/* Card Toolbar (legacy) */

type CardToolbarProps = {
  children: ReactNode;
  className?: string;
};

export function CardToolbar({ children, className }: CardToolbarProps) {
  return <div className={cn("flex flex-wrap items-center gap-2 border-2 border-[var(--chalk)] bg-[var(--slate)] p-2", className)}>{children}</div>;
}

/* Surface Toggle Group (legacy) */

type SurfaceToggleGroupProps = {
  children: ReactNode;
  className?: string;
};

export function SurfaceToggleGroup({ children, className }: SurfaceToggleGroupProps) {
  return <div className={cn("border-2 border-[var(--chalk)] bg-[var(--chalk)] p-px", className)}>{children}</div>;
}

import type {ComponentProps, ReactNode} from "react";
import {
    RiAddLine,
    RiArrowDownSLine,
    RiDeleteBinLine,
    RiInformationLine,
    RiMapPinLine,
    RiStarFill,
    RiStarLine,
} from "@remixicon/react";

import {Alert, AlertDescription, AlertTitle} from "@/components/ui/alert";
import {Badge} from "@/components/ui/badge";
import {Button} from "@/components/ui/button";
import {Card, CardContent, CardDescription, CardHeader} from "@/components/ui/card";
import {Collapsible, CollapsibleContent, CollapsibleTrigger} from "@/components/ui/collapsible";
import {Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle} from "@/components/ui/empty";
import {Field, FieldContent, FieldLabel} from "@/components/ui/field";
import {Input} from "@/components/ui/input";
import {Slider} from "@/components/ui/slider";
import {Switch} from "@/components/ui/switch";
import {Tooltip, TooltipContent, TooltipTrigger} from "@/components/ui/tooltip";
import {BlackmarkPage} from "@/components/app/blackmark-page";
import {useTheme} from "@/hooks/use-theme";
import {cn} from "@/lib/utils";

/* ────────── App Page Grid ────────── */

type AppPageProps = {
    children: ReactNode;
    className?: string;
};

export function AppPage({children, className}: AppPageProps) {
    return (
        <div className={cn("grid min-h-0 flex-1 grid-cols-12 gap-3 font-sans", className)}>
            {children}
        </div>
    );
}

/* ────────── Macro Header ────────── */

type MacroHeaderProps = {
    title: string;
    actions?: ReactNode;
    className?: string;
};

export function MacroHeader({actions, className, title}: MacroHeaderProps) {
    return (
        <header className={cn("col-span-12 flex min-h-10 flex-wrap items-center justify-between gap-3", className)}>
            <h1 className="truncate text-xl font-semibold leading-tight" title={title}>
                {title}
            </h1>
            {actions ? <div className="flex min-w-0 flex-wrap items-center justify-end gap-2">{actions}</div> : null}
        </header>
    );
}

/* 新工具页骨架：ToolPageFrame → MasterSwitchCard? → SyncGroupSection? → SyncCardList */

type ToolPageFrameProps = {
    title: string;
    actions?: ReactNode;
    copy?: string;
    error?: ReactNode;
    specs?: ReactNode;
    children: ReactNode;
    className?: string;
};

const BLACKMARK_COPY: Record<string, string> = {
    计时器: "局内倒计时。显示窗叠在游戏画面上，不跟主窗口换皮。",
    计数器: "局内计数。显示窗叠在游戏画面上，不跟主窗口换皮。",
    连发器: "按住连发。显示窗叠在游戏画面上，不跟主窗口换皮。",
    识别触发: "区域匹配、识色或热键触发效果。",
    息屏: "只挡画面。识别截图、键鼠和 Alt+Tab 照常。",
    摩斯: "截屏解码摩斯并自动输入。",
    收藏夹: "从计时器、计数器、连发器星标收进来的卡片。",
};

export function ToolPageFrame({actions, children, className, copy, error, specs, title}: ToolPageFrameProps) {
    const {uiWorld} = useTheme();
    if (uiWorld === "blackmark") {
        return (
            <BlackmarkPage
                actions={actions}
                copy={copy ?? BLACKMARK_COPY[title]}
                specs={specs}
                title={title === "收藏夹" ? "收藏" : title}
            >
                {error ? <div className="px-8">{error}</div> : null}
                <div className={cn("grid grid-cols-12 gap-3 px-8 pb-16", className)}>
                    {children}
                </div>
            </BlackmarkPage>
        );
    }
    return (
        <AppPage className={cn("auto-rows-max", className)}>
            <MacroHeader actions={actions} title={title}/>
            {error ? <div className="col-span-12">{error}</div> : null}
            {children}
        </AppPage>
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

export function StatusMatrix({items, className}: StatusMatrixProps) {
    const stateColor: Record<string, string> = {
        idle: "badge-ghost",
        active: "badge-primary",
        valid: "badge-success",
        warning: "badge-warning",
        error: "badge-error",
    };
    return (
        <div className={cn("flex flex-wrap gap-2", className)} aria-label="只读状态">
            {items.map((item) => (
                <span
                    key={item.id}
                    data-state={item.state}
                    className={cn("badge badge-outline badge-sm gap-2 font-mono text-xs", stateColor[item.state])}
                    title={item.label}
                >
                    {item.label ?? item.id}
                </span>
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

export function SignalTile({className, detail, icon, label, value}: SignalTileProps) {
    const detailTitle = typeof detail === "string" ? detail : undefined;

    return (
        <div className={cn("stat min-h-12 min-w-0 bg-base-100 px-3 py-2 text-base-content", className)}>
            {icon ? <div className="stat-figure text-base-content">{icon}</div> : null}
            <div className="stat-title truncate text-xs" title={label}>{label}</div>
            <div className="stat-value flex items-center gap-2 text-lg">
                {value}
            </div>
            {detail ? <div className="stat-desc truncate" title={detailTitle}>{detail}</div> : null}
        </div>
    );
}

/* ────────── Tactical Card (legacy) ────────── */

type TacticalCardProps = ComponentProps<typeof Card> & {
    active?: boolean;
};

export function runStateClass(status?: string | null) {
    if (status === "running" || status === "firing") {
        return "outline outline-1 -outline-offset-1 outline-primary";
    }
    if (status === "finished") {
        return "outline outline-1 -outline-offset-1 outline-success";
    }
    if (status === "pendingCompensation") {
        return "outline outline-1 -outline-offset-1 outline-warning";
    }
    return "";
}

export function TacticalCard({active, children, className, size = "sm", ...props}: TacticalCardProps) {
    return (
        <Card
            size={size}
            className={cn(
                "bg-base-200 text-base-content shadow-none transition-[background-color,outline-color] duration-200 ease-[cubic-bezier(0.16,1,0.3,1)] motion-reduce:duration-150",
                active && "ring-2 ring-primary",
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

export function SectionHeader({actions, badge, className, description, title}: SectionHeaderProps) {
    const descriptionTitle = typeof description === "string" ? description : undefined;

    return (
        <CardHeader className={cn("border-b border-base-300 pb-3", className)}>
            <div className="flex min-w-0 items-start justify-between gap-4">
                <div className="min-w-0 flex-1">
                    {typeof title === "string"
                        ? <h2 className="truncate text-base font-semibold">{title}</h2>
                        : <div className="truncate text-base font-semibold">{title}</div>}
                    {description ? (
                        <CardDescription className="mt-1 truncate" title={descriptionTitle}>
                            {description}
                        </CardDescription>
                    ) : null}
                </div>
                <div className="flex shrink-0 flex-wrap items-center justify-end gap-2">
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

export function ControlTile({children, className}: ControlTileProps) {
    return <div className={cn("min-w-0 overflow-hidden rounded-box border border-base-300 bg-base-100 p-4", className)}>{children}</div>;
}

export function InlineControl({children, className}: ControlTileProps) {
    return <div className={cn("rounded-field border border-base-300 bg-base-100 p-3", className)}>{children}</div>;
}

export function SaveStateBadge({dirty, saving}: { dirty: boolean; saving: boolean }) {
    if (saving) return <Badge variant="outline">保存中</Badge>;
    if (dirty) return <Badge variant="outline">待保存</Badge>;
    return <Badge variant="secondary">已保存</Badge>;
}

export function CardBody({children, className}: { children: ReactNode; className?: string }) {
    return <CardContent className={cn("pt-4", className)}>{children}</CardContent>;
}

type MasterSwitchCardProps = {
    checked: boolean;
    disabled?: boolean;
    ariaLabel: string;
    label: ReactNode;
    onCheckedChange: (checked: boolean) => void;
    className?: string;
};

export function MasterSwitchCard({
                                     ariaLabel,
                                     checked,
                                     className,
                                     disabled,
                                     label,
                                     onCheckedChange,
                                 }: MasterSwitchCardProps) {
    return (
        <TacticalCard className={cn("col-span-12", className)}>
            <SectionHeader title="总开关"/>
            <CardBody>
                <ControlTile className="flex items-center gap-3">
                    <Switch aria-label={ariaLabel} checked={checked} disabled={disabled}
                            onCheckedChange={onCheckedChange}/>
                    {typeof label === "string" ? <p className="text-sm font-medium">{label}</p> : label}
                </ControlTile>
            </CardBody>
        </TacticalCard>
    );
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

export function ChannelTabs({tabs, onTabChange, className}: ChannelTabsProps) {
    return (
        <div role="tablist" className={cn("tabs tabs-border", className)}>
            {tabs.map((tab) => (
                <button
                    key={tab.id}
                    className={cn(
                        "tab",
                        tab.active && "tab-active",
                    )}
                    onClick={() => onTabChange(tab.id)}
                    role="tab"
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

export function ConfigRow({label, value, unit, state = "idle", className}: ConfigRowProps) {
    const stateColor: Record<string, string> = {
        idle: "status-neutral",
        active: "status-primary",
        valid: "status-success",
        warning: "status-warning",
        error: "status-error",
    };
    return (
        <div
            data-state={state}
            className={cn(
                "grid grid-cols-[max-content_1fr_max-content_max-content] items-center gap-x-3 border-b border-base-300 px-3 py-2 text-sm text-base-content",
                className,
            )}
        >
            <span className="text-base-content/60">{label}</span>
            <span className="min-w-0 truncate text-right font-medium tabular-nums">{value}</span>
            {unit ? <span className="text-xs text-base-content/60">{unit}</span> :
                <span/>}
            <span className={cn("status status-sm", stateColor[state])}/>
        </div>
    );
}

/* Help Hint (circle !) */

type HelpHintProps = {
    content: ReactNode;
    className?: string;
};

export function HelpHint({content, className}: HelpHintProps) {
    return (
        <Tooltip delayDuration={200}>
            <TooltipTrigger asChild>
                <button
                    className={cn(
                        "btn btn-circle btn-ghost btn-xs text-base-content/60 hover:text-base-content",
                        className,
                    )}
                    type="button"
                    aria-label="说明"
                >
                    <RiInformationLine className="size-3"/>
                </button>
            </TooltipTrigger>
            <TooltipContent className="max-w-xs">
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

export function DataWell({children, className, maxHeight = "max-h-64"}: DataWellProps) {
    return (
        <div
            className={cn("overflow-auto rounded-box border border-base-300 bg-base-100 p-3 font-mono text-xs leading-relaxed text-base-content", maxHeight, className)}>
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

export function FieldUnit({children, className, header, footer}: FieldUnitProps) {
    return (
        <div className={cn("card card-border bg-base-200 shadow-none", className)}>
            {header ? <div className="border-b border-base-300 px-4 py-3 text-sm font-semibold">{header}</div> : null}
            <div className="p-3">{children}</div>
            {footer ? <div className="border-t border-base-300 px-4 py-3">{footer}</div> : null}
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

export function EmptyState({action, className, description, icon, title}: EmptyStateProps) {
    return (
        <div
            className={cn("flex flex-col items-center justify-center rounded-box border border-dashed border-base-300 bg-base-200 px-6 py-12 text-center", className)}>
            {icon ? <div className="mb-4 flex size-12 items-center justify-center rounded-field bg-base-100 text-base-content">{icon}</div> : null}
            <h3 className="text-lg font-semibold text-base-content">{title}</h3>
            <p className="mt-2 max-w-[48ch] text-sm text-base-content/60">{description}</p>
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

export function MacroNumber({className, label, unit, value}: MacroNumberProps) {
    return (
        <div className={cn("flex flex-col items-start gap-1", className)}>
            {label ? (
                <span className="text-xs text-base-content/60">{label}</span>
            ) : null}
            <div className="flex items-baseline gap-2">
        <span className="text-5xl font-semibold leading-none text-base-content">
          {value}
        </span>
                {unit ? (
                    <span className="text-sm text-base-content/60">{unit}</span>
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

export function DragButton({controlsDisabled, onDragStart}: DragButtonProps) {
    return (
        <Button aria-label="拖动排序" className="cursor-grab active:cursor-grabbing" disabled={controlsDisabled}
                onPointerDown={(event) => {
                    event.preventDefault();
                    onDragStart();
                }} size="icon-sm" type="button" variant="ghost">
            <span className="text-xs font-bold">↕</span>
        </Button>
    );
}

type FavoriteButtonProps = {
    isFavorite: boolean;
    disabled?: boolean;
    onClick: () => void;
};

export function FavoriteButton({disabled, isFavorite, onClick}: FavoriteButtonProps) {
    return (
        <Button
            aria-label={isFavorite ? "取消收藏" : "加入收藏"}
            aria-pressed={isFavorite}
            className={cn(isFavorite ? "text-base-content" : "text-base-content/60")}
            data-icon="inline-start"
            disabled={disabled}
            onClick={onClick}
            size="icon-sm"
            type="button"
            variant="outline"
        >
            {isFavorite ? <RiStarFill aria-hidden="true"/> : <RiStarLine aria-hidden="true"/>}
        </Button>
    );
}

type CardNameInputProps = {
    value: string;
    fallback: string;
    disabled?: boolean;
    ariaLabel: string;
    onChange: (value: string) => void;
};

export function CardNameInput({ariaLabel, disabled, fallback, onChange, value}: CardNameInputProps) {
    return (
        <Input
            aria-label={ariaLabel}
            className="h-auto w-full border-0 bg-transparent p-0 font-heading text-lg font-medium text-base-content placeholder:text-base-content/40 focus-visible:ring-0 focus-visible:ring-offset-0"
            disabled={disabled}
            onChange={(event) => onChange(event.currentTarget.value)}
            placeholder="输入卡片名称"
            value={value || fallback}
        />
    );
}

/* Hotkey Field */

type HotkeyFieldProps = {
    controlsDisabled: boolean;
    hotkey: string;
    id: string;
    isRecording: boolean;
    helperText?: string;
    onBeginHotkeyRecording: () => void;
    onHotkeyKeyDown: (event: React.KeyboardEvent<HTMLButtonElement>) => void;
    onHotkeyRecorderBlur: () => void;
};

export function HotkeyField({
                                controlsDisabled,
                                hotkey,
                                id,
                                isRecording,
                                helperText,
                                onBeginHotkeyRecording,
                                onHotkeyKeyDown,
                                onHotkeyRecorderBlur
                            }: HotkeyFieldProps) {
    return (
        <Field>
            <FieldLabel htmlFor={id}>快捷键</FieldLabel>
            <FieldContent>
                <Button className="h-9 w-full justify-between gap-4 px-3 font-mono" disabled={controlsDisabled} id={id}
                        onBlur={onHotkeyRecorderBlur} onClick={onBeginHotkeyRecording} onKeyDown={onHotkeyKeyDown}
                        title={helperText} type="button" variant="outline">
                    <span>{isRecording ? "录制中..." : hotkey || "未设置"}</span>
                    <span
                        className="text-caption text-muted-foreground">{isRecording ? "失焦取消" : "点击录制"}</span>
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
        <ControlTile className="flex flex-col gap-3 bg-base-100">
            <div className="flex flex-wrap items-center gap-3">
                <Switch checked={group.enabled} disabled={controlsDisabled}
                        aria-label={`${targetLabel}分组启用`}
                        onCheckedChange={(checked) => onGroupUpdate({enabled: checked})}/>
                <p className="font-mono text-xs font-medium text-base-content">
                    {targetLabel}分组 · {group.name}
                </p>
                <Input
                    aria-label="分组名称"
                    className="w-28 font-mono text-sm"
                    disabled={controlsDisabled}
                    onChange={(event) => onGroupUpdate({name: event.currentTarget.value})}
                    value={group.name}
                />
                <Button aria-label="删除分组" className="shrink-0" disabled={!canDelete} onClick={onGroupDelete}
                        size="icon-sm" type="button" variant="ghost">
                    <RiDeleteBinLine/>
                </Button>
                <Button className="shrink-0" disabled={controlsDisabled} onClick={onPositionSelection} size="sm"
                        type="button" variant="outline">
                    <RiMapPinLine data-icon="inline-start"/>
                    位置
                </Button>
            </div>

            <Collapsible defaultOpen={false}>
                <InlineControl className="p-0">
                    <CollapsibleTrigger asChild>
                        <Button
                            className="w-full justify-between px-2 py-1.5 font-mono text-xs font-medium"
                            type="button"
                            variant="ghost"
                        >
                            显示参数
                            <RiArrowDownSLine className="size-3.5"/>
                        </Button>
                    </CollapsibleTrigger>
                    <CollapsibleContent className="border-t border-base-content px-2 py-2">
                        <div className="flex flex-wrap items-center gap-4">
                            <Field className="min-w-0 flex-1">
                                <FieldLabel className="font-mono text-xs">字体透明度</FieldLabel>
                                <FieldContent>
                                    <div className="flex items-center gap-3">
                                        <Slider
                                            aria-label="字体透明度"
                                            disabled={controlsDisabled || !display}
                                            max={1}
                                            min={0.1}
                                            onValueChange={([value]) => onUpdate({fontOpacity: value.toFixed(2)})}
                                            step={0.05}
                                            value={[Number.parseFloat(display?.fontOpacity ?? "0.9")]}
                                        />
                                        <span className="w-10 text-right font-mono text-xs text-base-content/60">
                                            {display?.fontOpacity ?? "--"}
                                        </span>
                                    </div>
                                </FieldContent>
                            </Field>
                            <Field className="w-36 shrink-0">
                                <FieldLabel className="font-mono text-xs">窗口宽度</FieldLabel>
                                <FieldContent>
                                    <Input
                                        className="h-7 font-mono text-xs"
                                        disabled={controlsDisabled || !display}
                                        inputMode="numeric"
                                        min="320"
                                        onChange={(event) => onUpdateRect({width: Number.parseInt(event.currentTarget.value, 10) || 320})}
                                        value={display?.rect?.width ?? 320}
                                    />
                                </FieldContent>
                            </Field>
                        </div>
                    </CollapsibleContent>
                </InlineControl>
            </Collapsible>

            <p className="font-mono text-xs font-medium text-base-content/60">{statusMessage}</p>
        </ControlTile>
    );
}

/* Page Preview Banner (legacy) */

export function PagePreviewBanner({className}: { className?: string }) {
    return (
        <Alert
            variant="destructive"
            className={cn("alert-warning", className)}
        >
            <AlertTitle>浏览器预览模式</AlertTitle>
            <AlertDescription>
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

export function TacticalEmptyState({children, className, description, icon, title}: TacticalEmptyStateProps) {
    return (
        <Empty className={cn("min-h-48 rounded-box border border-dashed border-base-300 bg-base-200 px-4 py-8 text-center", className)}>
            {icon ? <EmptyMedia variant="icon">{icon}</EmptyMedia> : null}
            <EmptyHeader>
                <EmptyTitle>{title}</EmptyTitle>
                <EmptyDescription>{description}</EmptyDescription>
            </EmptyHeader>
            {children}
        </Empty>
    );
}

/* Add Card Button (legacy) */

type AddCardButtonProps = {
    disabled?: boolean;
    title: string;
    description?: ReactNode;
    className?: string;
    onClick: () => void;
};

export function AddCardButton({className, description, disabled, onClick, title}: AddCardButtonProps) {
    return (
        <button
            className={cn(
                "group flex min-h-24 flex-col items-center justify-center rounded-box border border-dashed border-base-300 bg-base-200 p-4 text-center transition-colors hover:bg-base-100 focus:outline-none focus-visible:outline-2 focus-visible:outline-primary disabled:cursor-not-allowed disabled:opacity-50",
                className,
            )}
            disabled={disabled}
            onClick={onClick}
            type="button"
        >
            <RiAddLine className="size-6 text-base-content/60"/>
            <span className="mt-2 text-sm font-semibold text-base-content">{title}</span>
            {description ? (
                <span className="mt-1 max-w-56 text-sm leading-relaxed text-base-content/60">{description}</span>
            ) : null}
        </button>
    );
}

/* Json Pre Block (legacy) */

type JsonPreBlockProps = {
    data: unknown;
    className?: string;
    maxHeightClassName?: string;
};

export function JsonPreBlock({className, data, maxHeightClassName = "max-h-64"}: JsonPreBlockProps) {
    return (
        <pre
            className={cn(
                maxHeightClassName,
                "mockup-code overflow-auto bg-neutral p-3 font-mono text-xs leading-relaxed text-neutral-content",
                className,
            )}
        >
      {JSON.stringify(data, null, 2)}
    </pre>
    );
}

/* Soft Alert */

type SoftAlertTone = "error" | "warning" | "info" | "success";

type SoftAlertProps = {
    children: ReactNode;
    className?: string;
    tone?: SoftAlertTone;
};

const SOFT_ALERT_TONE: Record<Exclude<SoftAlertTone, "error">, string> = {
    info: "alert-info",
    success: "alert-success",
    warning: "alert-warning",
};

export function SoftAlert({children, className, tone = "error"}: SoftAlertProps) {
    return (
        <Alert
            className={cn(tone !== "error" && SOFT_ALERT_TONE[tone], className)}
            variant={tone === "error" ? "destructive" : "default"}
        >
            {children}
        </Alert>
    );
}

/* Overlay Readout Shell */

type OverlayReadoutShellProps = {
    children: ReactNode;
    className?: string;
    opacity?: number;
};

export function OverlayReadoutShell({children, className, opacity}: OverlayReadoutShellProps) {
    return (
        <div
            className="flex h-screen w-screen items-start justify-start overflow-hidden bg-transparent p-2 font-mono text-white"
            style={opacity === undefined ? undefined : {opacity}}
        >
            <div
                className={cn(
                    "h-full w-full overflow-hidden rounded-md border border-white/20 bg-black/20 px-3 py-2 backdrop-blur-[1px]",
                    className,
                )}
            >
                {children}
            </div>
        </div>
    );
}

/* Inline Notice (legacy) */

type InlineNoticeProps = {
    title?: string;
    children: ReactNode;
    className?: string;
    tone?: SoftAlertTone;
};

export function InlineNotice({children, className, title, tone = "error"}: InlineNoticeProps) {
    return (
        <SoftAlert className={className} tone={tone}>
            {title ? <AlertTitle>{title}</AlertTitle> : null}
            <AlertDescription>{children}</AlertDescription>
        </SoftAlert>
    );
}

/* Card Toolbar (legacy) */

type CardToolbarProps = {
    children: ReactNode;
    className?: string;
};

export function CardToolbar({children, className}: CardToolbarProps) {
    return <div className={cn("flex flex-wrap items-center gap-2 rounded-box border border-base-300 bg-base-200 p-2", className)}>{children}</div>;
}

/* Surface Toggle Group (legacy) */

type SurfaceToggleGroupProps = {
    children: ReactNode;
    className?: string;
};

export function SurfaceToggleGroup({children, className}: SurfaceToggleGroupProps) {
    return <div className={cn("join rounded-field bg-base-200", className)}>{children}</div>;
}

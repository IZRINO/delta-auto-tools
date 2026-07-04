import type {ComponentProps, ReactNode} from "react";
import {
    RiAddLine,
    RiInformationLine,
} from "@remixicon/react";

import {Alert, AlertDescription, AlertTitle} from "@/components/ui/alert";
import {Badge} from "@/components/ui/badge";
import {Button} from "@/components/ui/button";
import {Card, CardContent, CardDescription, CardHeader, CardTitle} from "@/components/ui/card";
import {Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle} from "@/components/ui/empty";
import {Field, FieldContent, FieldLabel} from "@/components/ui/field";
import {Tooltip, TooltipContent, TooltipTrigger} from "@/components/ui/tooltip";
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
    code: string;
    title: string;
    subtitle: string;
    verticalLabel?: string;
    badges?: ReactNode;
    actions?: ReactNode;
    className?: string;
};

export function MacroHeader({actions, badges, className, code, subtitle, title, verticalLabel}: MacroHeaderProps) {
    return (
        <section
            className={cn(
                "card card-border col-span-12 bg-base-200 text-base-content shadow-none",
                className,
            )}
        >
            <div className="card-body gap-4">
                <div className="flex flex-wrap items-center gap-2">
                    <span className="badge badge-primary badge-sm">{code}</span>
                    {verticalLabel ? <span className="badge badge-ghost badge-sm">{verticalLabel}</span> : null}
                    {badges}
                </div>
                <div className="flex min-w-0 flex-col gap-3 lg:flex-row lg:items-end lg:justify-between">
                    <div className="min-w-0">
                        <h1 className="text-4xl font-semibold leading-tight text-base-content">
                            {title}
                        </h1>
                        <p className="mt-2 max-w-[64ch] text-sm leading-relaxed text-base-content/70">
                            {subtitle}
                        </p>
                    </div>
                    {actions ? <div className="card-actions shrink-0 justify-end">{actions}</div> : null}
                </div>
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

export function StatusMatrix({items, className}: StatusMatrixProps) {
    const stateColor: Record<string, string> = {
        idle: "badge-ghost",
        active: "badge-primary",
        valid: "badge-success",
        warning: "badge-warning",
        error: "badge-error",
    };
    return (
        <div className={cn("flex flex-wrap gap-2", className)}>
            {items.map((item) => (
                <span
                    key={item.id}
                    data-state={item.state}
                    className={cn("badge badge-sm gap-2", stateColor[item.state])}
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
    return (
        <div className={cn("stat min-h-12 bg-base-100 px-3 py-2 text-base-content", className)}>
            {icon ? <div className="stat-figure text-primary">{icon}</div> : null}
            <div className="stat-title truncate text-xs">{label}</div>
            <div className="stat-value flex items-center gap-2 text-lg">
                {value}
            </div>
            {detail ? <div className="stat-desc truncate">{detail}</div> : null}
        </div>
    );
}

/* ────────── Tactical Card (legacy) ────────── */

type TacticalCardProps = ComponentProps<typeof Card> & {
    active?: boolean;
};

export function TacticalCard({active, children, className, size = "sm", ...props}: TacticalCardProps) {
    return (
        <Card
            size={size}
            className={cn(
                "bg-base-200 text-base-content shadow-none transition-[background-color,outline-color]",
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

export function SectionHeader({actions, badge, className, description, eyebrow, icon, title}: SectionHeaderProps) {
    return (
        <CardHeader
            className={cn("border-b border-base-300 pb-3", className)}>
            <div className="flex min-w-0 items-start justify-between gap-4">
                <div className="flex min-w-0 items-start gap-3">
                    {icon ? (
                        <div className="flex size-9 shrink-0 items-center justify-center rounded-field bg-primary text-primary-content">
                            {icon}
                        </div>
                    ) : null}
                    <div className="min-w-0">
                        {eyebrow ?
                            <p className="mb-1 text-xs text-base-content/60">{eyebrow}</p> : null}
                        <CardTitle className="mt-0.5 truncate">{title}</CardTitle>
                        {description ? <CardDescription
                            className="mt-1 truncate">{description}</CardDescription> : null}
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

export function ControlTile({children, className}: ControlTileProps) {
    return <div className={cn("rounded-box border border-base-300 bg-base-100 p-4", className)}>{children}</div>;
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
                        "btn btn-circle btn-ghost btn-xs text-base-content/60 hover:text-primary",
                        className,
                    )}
                    type="button"
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
            {icon ? <div className="mb-4 flex size-12 items-center justify-center rounded-field bg-base-100 text-primary">{icon}</div> : null}
            <h3 className="text-lg font-semibold text-base-content">{title}</h3>
            <p className="mt-2 max-w-[48ch] text-sm text-base-content/60">{description}</p>
            {action ? <div className="mt-6">{action}</div> : null}
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

export function HotkeyField({
                                controlsDisabled,
                                hotkey,
                                id,
                                isRecording,
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
                        type="button" variant="outline">
                    <span>{isRecording ? "录制中..." : hotkey || "未设置"}</span>
                    <span
                        className="text-[0.6875rem] text-muted-foreground">{isRecording ? "失焦取消" : "点击录制"}</span>
                </Button>
            </FieldContent>
        </Field>
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
        <TacticalCard className={cn("min-h-48", className)}>
            <CardBody className="flex h-full items-center justify-center">
                <Empty className="min-h-40 rounded-box border border-dashed border-base-300 bg-base-200 px-4 py-8 text-center">
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

export function AddCardButton({className, description, disabled, onClick, title}: AddCardButtonProps) {
    return (
        <button
            className={cn(
                "group flex min-h-32 flex-col items-center justify-center rounded-box border border-dashed border-base-300 bg-base-200 p-4 text-center transition-colors hover:bg-base-100 focus:outline-none focus-visible:outline-2 focus-visible:outline-primary disabled:cursor-not-allowed disabled:opacity-50",
                className,
            )}
            disabled={disabled}
            onClick={onClick}
            type="button"
        >
      <span className="mb-4 flex size-11 items-center justify-center rounded-field bg-primary text-primary-content">
        <RiAddLine/>
      </span>
            <span className="text-sm font-semibold text-base-content">{title}</span>
            <span className="mt-1 max-w-56 text-sm leading-relaxed text-base-content/60">{description}</span>
        </button>
    );
}

/* Inline Notice (legacy) */

type InlineNoticeProps = {
    title?: string;
    children: ReactNode;
    className?: string;
};

export function InlineNotice({children, className, title}: InlineNoticeProps) {
    return (
        <Alert variant="destructive"
               className={className}>
            {title ? <AlertTitle>{title}</AlertTitle> : null}
            <AlertDescription>{children}</AlertDescription>
        </Alert>
    );
}

/* Surface Toggle Group (legacy) */

type SurfaceToggleGroupProps = {
    children: ReactNode;
    className?: string;
};

export function SurfaceToggleGroup({children, className}: SurfaceToggleGroupProps) {
    return <div className={cn("join rounded-field bg-base-200", className)}>{children}</div>;
}
